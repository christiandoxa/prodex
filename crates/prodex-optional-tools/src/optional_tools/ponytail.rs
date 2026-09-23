use super::{
    OptionalToolId, ResolvedTool, TOOL_PROBE_TIMEOUT, ToolDiscoverySource, ToolHealth,
    find_path_command, invalid_tool, manifest_tree_sha256_supported, optional_tool_descriptor,
};
use crate::discovery::managed_optimizer_roots;
use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const PONYTAIL_SOURCE: &str = "https://github.com/DietrichGebert/ponytail";
const TOOL_MANIFEST: &str = "prodex-tool.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolInstallManifest {
    schema_version: u32,
    id: String,
    version: String,
    source: String,
    commit: String,
    tree_sha256: String,
}

#[derive(Debug, Deserialize)]
struct CodexPluginManifest {
    name: String,
    version: String,
}

pub(super) fn tool_status() -> ToolHealth {
    tool_status_with_node(find_path_command("node"))
}

fn tool_status_with_node(node: Option<PathBuf>) -> ToolHealth {
    let id = OptionalToolId::Ponytail;
    let Some(node) = node else {
        return ToolHealth::missing(id, "Node.js was not found on PATH");
    };
    match crate::process::probe_command(&node, &["--version"], TOOL_PROBE_TIMEOUT) {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            return invalid_tool(
                id,
                anyhow::anyhow!("Node.js health check exited with {}", output.status),
            );
        }
        Err(error) => return invalid_tool(id, error),
    }
    match candidate() {
        Ok(Some((root, candidate))) => validate_install(&root, &candidate)
            .map(ToolHealth::installed)
            .unwrap_or_else(|error| invalid_tool(id, error)),
        Ok(None) => ToolHealth::missing(
            id,
            format!(
                "Ponytail {}+ was not found under a managed optional-tool root; latest stable reference is {}",
                crate::PONYTAIL_MINIMUM_SUPPORTED_VERSION,
                crate::PONYTAIL_LATEST_STABLE_REFERENCE
            ),
        ),
        Err(error) => invalid_tool(id, error),
    }
}

fn candidate() -> Result<Option<(PathBuf, PathBuf)>> {
    let minimum = Version::parse(crate::PONYTAIL_MINIMUM_SUPPORTED_VERSION)
        .context("invalid Ponytail minimum supported version")?;
    for root in managed_optimizer_roots() {
        let metadata = match fs::symlink_metadata(&root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("failed to inspect {}", root.display()));
            }
        };
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "optional-tool root {} must be a real directory",
            root.display()
        );
        let tool_root = root.join("ponytail");
        let entries = match fs::read_dir(&tool_root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", tool_root.display()));
            }
        };
        let mut newest: Option<(Version, PathBuf)> = None;
        for entry in entries {
            let entry = entry
                .with_context(|| format!("failed to read entry in {}", tool_root.display()))?;
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(version) = Version::parse(&name) else {
                continue;
            };
            if !version.pre.is_empty() {
                continue;
            }
            if newest
                .as_ref()
                .is_none_or(|(current, _)| version > *current)
            {
                newest = Some((version, entry.path()));
            }
        }
        let Some((version, candidate)) = newest else {
            continue;
        };
        anyhow::ensure!(
            version >= minimum,
            "installed Ponytail {version} is too old; Prodex requires {} or newer. Update Ponytail to the latest stable release (release-qualified reference: {})",
            crate::PONYTAIL_MINIMUM_SUPPORTED_VERSION,
            crate::PONYTAIL_LATEST_STABLE_REFERENCE
        );
        return Ok(Some((root, candidate)));
    }
    Ok(None)
}

fn valid_git_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_install(allowed_root: &Path, candidate: &Path) -> Result<ResolvedTool> {
    let candidate = validated_managed_directory(allowed_root, candidate)?;
    let manifest_path = candidate.join(TOOL_MANIFEST);
    let manifest: ToolInstallManifest =
        serde_json::from_slice(&crate::tree::read_bounded_file(&manifest_path, 64 * 1024)?)
            .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    anyhow::ensure!(
        manifest.schema_version == 1,
        "unsupported Ponytail manifest schema"
    );
    anyhow::ensure!(
        manifest.id == "ponytail",
        "Ponytail manifest id must be ponytail"
    );
    let version = Version::parse(&manifest.version)
        .with_context(|| format!("invalid Ponytail version {}", manifest.version))?;
    let minimum = Version::parse(crate::PONYTAIL_MINIMUM_SUPPORTED_VERSION)
        .context("invalid Ponytail minimum supported version")?;
    anyhow::ensure!(
        version.pre.is_empty() && version >= minimum,
        "Ponytail {} is incompatible; Prodex requires {} or newer. Update Ponytail to the latest stable release (release-qualified reference: {})",
        manifest.version,
        crate::PONYTAIL_MINIMUM_SUPPORTED_VERSION,
        crate::PONYTAIL_LATEST_STABLE_REFERENCE
    );
    anyhow::ensure!(
        candidate.file_name().and_then(|name| name.to_str()) == Some(manifest.version.as_str()),
        "Ponytail version directory does not match manifest version {}",
        manifest.version
    );
    anyhow::ensure!(
        manifest.source == PONYTAIL_SOURCE,
        "unexpected Ponytail source"
    );
    anyhow::ensure!(
        valid_git_sha(&manifest.commit),
        "Ponytail manifest commit must be a 40-character Git SHA"
    );
    anyhow::ensure!(
        valid_sha256(&manifest.tree_sha256),
        "Ponytail manifest tree digest must be SHA-256"
    );

    let plugin_path = candidate.join(".codex-plugin/plugin.json");
    let plugin: CodexPluginManifest =
        serde_json::from_slice(&crate::tree::read_bounded_file(&plugin_path, 256 * 1024)?)
            .with_context(|| format!("failed to parse {}", plugin_path.display()))?;
    anyhow::ensure!(
        plugin.name == "ponytail",
        "Codex plugin name must be ponytail"
    );
    anyhow::ensure!(
        plugin.version == manifest.version,
        "Codex plugin version does not match Ponytail manifest version"
    );
    anyhow::ensure!(
        candidate.join("hooks/claude-codex-hooks.json").is_file()
            && candidate.join("skills").is_dir(),
        "Ponytail installation is incomplete"
    );
    let digest = crate::tree::tree_sha256(&candidate, b"prodex-ponytail-tree-v1\0")?;
    if manifest.version == crate::PONYTAIL_LATEST_STABLE_REFERENCE {
        anyhow::ensure!(
            manifest.commit == crate::PONYTAIL_LATEST_STABLE_COMMIT,
            "Ponytail latest-stable commit does not match release-qualified metadata"
        );
        anyhow::ensure!(
            manifest_tree_sha256_supported(
                &manifest.tree_sha256,
                crate::PONYTAIL_LATEST_STABLE_TREE_SHA256,
                crate::PONYTAIL_LEGACY_MANIFEST_TREE_SHA256,
            ),
            "Ponytail latest-stable manifest tree digest does not match release-qualified metadata"
        );
        anyhow::ensure!(
            digest == crate::PONYTAIL_LATEST_STABLE_TREE_SHA256,
            "Ponytail tree digest mismatch: expected {}, got {digest}",
            crate::PONYTAIL_LATEST_STABLE_TREE_SHA256
        );
    } else {
        anyhow::ensure!(
            digest == manifest.tree_sha256,
            "Ponytail tree digest mismatch: manifest {}, got {digest}",
            manifest.tree_sha256
        );
    }

    Ok(ResolvedTool {
        descriptor: optional_tool_descriptor(OptionalToolId::Ponytail),
        source: ToolDiscoverySource::ManagedRoot,
        path: Some(candidate),
        version: Some(plugin.version),
        digest: Some(format!("sha256:{digest}")),
    })
}

fn validated_managed_directory(allowed_root: &Path, candidate: &Path) -> Result<PathBuf> {
    let root_metadata = fs::symlink_metadata(allowed_root)
        .with_context(|| format!("failed to inspect {}", allowed_root.display()))?;
    anyhow::ensure!(
        root_metadata.is_dir() && !root_metadata.file_type().is_symlink(),
        "optional-tool root {} must be a real directory",
        allowed_root.display()
    );
    let candidate_metadata = fs::symlink_metadata(candidate)
        .with_context(|| format!("failed to inspect {}", candidate.display()))?;
    anyhow::ensure!(
        candidate_metadata.is_dir() && !candidate_metadata.file_type().is_symlink(),
        "optional-tool installation {} must be a real directory",
        candidate.display()
    );
    let allowed_root = allowed_root.canonicalize()?;
    let candidate = candidate.canonicalize()?;
    anyhow::ensure!(
        candidate.starts_with(&allowed_root) && candidate != allowed_root,
        "optional-tool installation escapes its managed root"
    );
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optional_tools::ToolHealthStatus;

    #[test]
    fn is_missing_when_node_is_not_on_path() {
        let health = tool_status_with_node(None);
        assert_eq!(health.status, ToolHealthStatus::Missing);
        assert!(health.detail.contains("Node.js was not found"));
    }

    #[test]
    fn validation_accepts_future_stable_self_consistent_install() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let allowed_root = std::env::temp_dir().join(format!(
            "prodex-ponytail-future-{}-{stamp}",
            std::process::id()
        ));
        let candidate = allowed_root.join("ponytail/4.11.0");
        fs::create_dir_all(candidate.join(".codex-plugin")).unwrap();
        fs::create_dir_all(candidate.join("hooks")).unwrap();
        fs::create_dir_all(candidate.join("skills")).unwrap();
        fs::write(
            candidate.join(".codex-plugin/plugin.json"),
            r#"{"name":"ponytail","version":"4.11.0"}"#,
        )
        .unwrap();
        fs::write(candidate.join("hooks/claude-codex-hooks.json"), "{}\n").unwrap();
        fs::write(candidate.join("skills/README.md"), "# future\n").unwrap();
        let digest = crate::tree::tree_sha256(&candidate, b"prodex-ponytail-tree-v1\0").unwrap();
        fs::write(
            candidate.join(TOOL_MANIFEST),
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "id": "ponytail",
                "version": "4.11.0",
                "source": PONYTAIL_SOURCE,
                "commit": "1111111111111111111111111111111111111111",
                "tree_sha256": digest,
            }))
            .unwrap(),
        )
        .unwrap();

        let tool = validate_install(&allowed_root, &candidate).unwrap();
        assert_eq!(tool.version.as_deref(), Some("4.11.0"));
        assert_eq!(tool.digest, Some(format!("sha256:{digest}")));
        fs::remove_dir_all(allowed_root).unwrap();
    }
}

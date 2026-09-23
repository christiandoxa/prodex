use crate::discovery::managed_optimizer_roots;
use crate::localization::ensure_agents_reference;
use crate::optional_tools::{
    OptionalToolId, ResolvedTool, ToolDiscoverySource, ToolHealth, ToolHealthStatus,
    optional_tool_descriptor,
};
use crate::tree::{read_bounded_file, tree_sha256};
use crate::{
    CAVEMAN_LATEST_STABLE_COMMIT, CAVEMAN_LATEST_STABLE_REFERENCE,
    CAVEMAN_LATEST_STABLE_TREE_SHA256, CAVEMAN_LEGACY_MANIFEST_TREE_SHA256,
    CAVEMAN_MINIMUM_SUPPORTED_VERSION,
};
use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const CAVEMAN_SOURCE: &str = "https://github.com/JuliusBrussee/caveman";
const TOOL_MANIFEST: &str = "prodex-tool.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CavemanInstallManifest {
    schema_version: u32,
    id: String,
    version: String,
    source: String,
    commit: String,
    tree_sha256: String,
}

#[derive(Debug, Deserialize)]
struct ClaudePluginManifest {
    name: String,
    hooks: serde_json::Value,
}

pub fn resolve_caveman() -> Result<ResolvedTool> {
    let Some((allowed_root, candidate)) = caveman_candidate()? else {
        bail!(
            "Caveman is not installed; install version {CAVEMAN_MINIMUM_SUPPORTED_VERSION} or newer under a managed optional-tool root (latest stable reference: {CAVEMAN_LATEST_STABLE_REFERENCE})"
        );
    };
    validate_caveman_install(&allowed_root, &candidate)
}

pub fn resolve_caveman_claude_plugin_dir() -> Result<PathBuf> {
    resolve_caveman()?
        .path
        .context("validated Caveman installation has no plugin path")
}

pub fn activate_caveman_for_codex(codex_home: &Path, tool: &ResolvedTool) -> Result<()> {
    ensure!(
        tool.descriptor.id == OptionalToolId::Caveman,
        "refusing to activate a non-Caveman optional tool as Caveman"
    );
    let validated = resolve_caveman()?;
    ensure!(
        validated.path == tool.path
            && validated.version == tool.version
            && validated.digest == tool.digest,
        "Caveman installation changed after resolution"
    );
    let root = validated
        .path
        .as_deref()
        .context("validated Caveman installation has no plugin path")?;
    let agents = root.join("AGENTS.md");
    ensure!(agents.is_file(), "{} is missing", agents.display());
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    ensure_agents_reference(codex_home, &agents)
}

pub(crate) fn caveman_tool_status() -> ToolHealth {
    let candidate = match caveman_candidate() {
        Ok(candidate) => candidate,
        Err(error) => return invalid_health(None, error),
    };
    let Some((allowed_root, candidate)) = candidate else {
        return ToolHealth::missing(
            OptionalToolId::Caveman,
            format!(
                "Caveman {CAVEMAN_MINIMUM_SUPPORTED_VERSION}+ was not found under a managed optional-tool root; latest stable reference is {CAVEMAN_LATEST_STABLE_REFERENCE}"
            ),
        );
    };
    match validate_caveman_install(&allowed_root, &candidate) {
        Ok(tool) => ToolHealth::installed(tool),
        Err(error) => invalid_health(Some(candidate), error),
    }
}

fn invalid_health(path: Option<PathBuf>, error: anyhow::Error) -> ToolHealth {
    ToolHealth {
        id: OptionalToolId::Caveman,
        status: ToolHealthStatus::Invalid,
        source: Some(ToolDiscoverySource::ManagedRoot),
        path,
        version: None,
        digest: None,
        can_activate: false,
        detail: error.to_string(),
    }
}

fn caveman_candidate() -> Result<Option<(PathBuf, PathBuf)>> {
    let minimum = Version::parse(CAVEMAN_MINIMUM_SUPPORTED_VERSION)
        .context("invalid Caveman minimum supported version")?;
    for root in managed_optimizer_roots() {
        let metadata = match fs::symlink_metadata(&root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("failed to inspect {}", root.display()));
            }
        };
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "optional-tool root {} must be a real directory",
            root.display()
        );
        let tool_root = root.join("caveman");
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
        ensure!(
            version >= minimum,
            "installed Caveman {version} is too old; Prodex requires {CAVEMAN_MINIMUM_SUPPORTED_VERSION} or newer. Update Caveman to the latest stable release (release-qualified reference: {CAVEMAN_LATEST_STABLE_REFERENCE})"
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

fn validate_caveman_install(allowed_root: &Path, candidate: &Path) -> Result<ResolvedTool> {
    let candidate_metadata = fs::symlink_metadata(candidate)
        .with_context(|| format!("failed to inspect {}", candidate.display()))?;
    ensure!(
        candidate_metadata.is_dir() && !candidate_metadata.file_type().is_symlink(),
        "Caveman installation {} must be a real directory",
        candidate.display()
    );
    let allowed_root = allowed_root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", allowed_root.display()))?;
    let candidate = candidate
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", candidate.display()))?;
    ensure!(
        candidate.starts_with(&allowed_root) && candidate != allowed_root,
        "Caveman installation {} escapes managed root {}",
        candidate.display(),
        allowed_root.display()
    );

    let manifest_path = candidate.join(TOOL_MANIFEST);
    let manifest: CavemanInstallManifest =
        serde_json::from_slice(&read_bounded_file(&manifest_path, 64 * 1024)?)
            .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    ensure!(
        manifest.schema_version == 1,
        "unsupported Caveman manifest schema"
    );
    ensure!(
        manifest.id == "caveman",
        "Caveman manifest id must be caveman"
    );

    let version = Version::parse(&manifest.version)
        .with_context(|| format!("invalid Caveman version {}", manifest.version))?;
    let minimum = Version::parse(CAVEMAN_MINIMUM_SUPPORTED_VERSION)
        .context("invalid Caveman minimum supported version")?;
    ensure!(
        version.pre.is_empty() && version >= minimum,
        "Caveman {} is incompatible; Prodex requires {} or newer. Update Caveman to the latest stable release (release-qualified reference: {})",
        manifest.version,
        CAVEMAN_MINIMUM_SUPPORTED_VERSION,
        CAVEMAN_LATEST_STABLE_REFERENCE
    );
    ensure!(
        candidate.file_name().and_then(|name| name.to_str()) == Some(manifest.version.as_str()),
        "Caveman version directory does not match manifest version {}",
        manifest.version
    );
    ensure!(
        manifest.source == CAVEMAN_SOURCE,
        "unexpected Caveman source"
    );
    ensure!(
        valid_git_sha(&manifest.commit),
        "Caveman manifest commit must be a 40-character Git SHA"
    );
    ensure!(
        valid_sha256(&manifest.tree_sha256),
        "Caveman manifest tree digest must be SHA-256"
    );

    validate_required_files(&candidate)?;
    let actual_digest = tree_sha256(&candidate, b"prodex-caveman-tree-v1\0")?;
    if manifest.version == CAVEMAN_LATEST_STABLE_REFERENCE {
        ensure!(
            manifest.commit == CAVEMAN_LATEST_STABLE_COMMIT,
            "Caveman latest-stable commit does not match release-qualified metadata"
        );
        ensure!(
            crate::optional_tools::manifest_tree_sha256_supported(
                &manifest.tree_sha256,
                CAVEMAN_LATEST_STABLE_TREE_SHA256,
                CAVEMAN_LEGACY_MANIFEST_TREE_SHA256,
            ),
            "Caveman latest-stable manifest tree digest does not match release-qualified metadata"
        );
        ensure!(
            actual_digest == CAVEMAN_LATEST_STABLE_TREE_SHA256,
            "Caveman tree digest mismatch: expected {CAVEMAN_LATEST_STABLE_TREE_SHA256}, got {actual_digest}"
        );
    } else {
        ensure!(
            actual_digest == manifest.tree_sha256,
            "Caveman tree digest mismatch: manifest {}, got {actual_digest}",
            manifest.tree_sha256
        );
    }

    Ok(ResolvedTool {
        descriptor: optional_tool_descriptor(OptionalToolId::Caveman),
        source: ToolDiscoverySource::ManagedRoot,
        path: Some(candidate),
        version: Some(manifest.version),
        digest: Some(format!("sha256:{actual_digest}")),
    })
}

fn validate_required_files(root: &Path) -> Result<()> {
    for relative in ["AGENTS.md", "skills/caveman/SKILL.md"] {
        let path = root.join(relative);
        ensure!(path.is_file(), "Caveman installation is missing {relative}");
    }
    let plugin_path = root.join(".claude-plugin/plugin.json");
    let plugin: ClaudePluginManifest =
        serde_json::from_slice(&read_bounded_file(&plugin_path, 256 * 1024)?)
            .with_context(|| format!("failed to parse {}", plugin_path.display()))?;
    ensure!(
        plugin.name == "caveman",
        "Claude plugin name must be caveman"
    );
    ensure!(
        plugin.hooks.is_object(),
        "Claude plugin hooks must be an object"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "prodex-caveman-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn tree_digest_is_stable_and_manifest_independent() {
        let root = temp_dir("digest");
        fs::create_dir_all(root.join("skills/caveman")).unwrap();
        fs::write(root.join("AGENTS.md"), "@./skills/caveman/SKILL.md\n").unwrap();
        fs::write(root.join("skills/caveman/SKILL.md"), "# Caveman\n").unwrap();
        let first = tree_sha256(&root, b"prodex-caveman-tree-v1\0").unwrap();
        fs::write(root.join(TOOL_MANIFEST), "{}\n").unwrap();
        assert_eq!(
            tree_sha256(&root, b"prodex-caveman-tree-v1\0").unwrap(),
            first
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn install_validation_accepts_exact_manifest_and_rejects_changed_content() {
        let allowed_root = temp_dir("install");
        let candidate = allowed_root.join("caveman/2.3.1");
        fs::create_dir_all(candidate.join("skills/caveman")).unwrap();
        fs::create_dir_all(candidate.join(".claude-plugin")).unwrap();
        fs::write(candidate.join("AGENTS.md"), "@./skills/caveman/SKILL.md\n").unwrap();
        fs::write(candidate.join("skills/caveman/SKILL.md"), "# Caveman\n").unwrap();
        fs::write(
            candidate.join(".claude-plugin/plugin.json"),
            r#"{"name":"caveman","hooks":{}}"#,
        )
        .unwrap();
        let digest = tree_sha256(&candidate, b"prodex-caveman-tree-v1\0").unwrap();
        fs::write(
            candidate.join(TOOL_MANIFEST),
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "id": "caveman",
                "version": "2.3.1",
                "source": CAVEMAN_SOURCE,
                "commit": "0123456789abcdef0123456789abcdef01234567",
                "tree_sha256": digest,
            }))
            .unwrap(),
        )
        .unwrap();

        let tool = validate_caveman_install(&allowed_root, &candidate).unwrap();
        assert_eq!(
            tool.path.as_deref(),
            Some(candidate.canonicalize().unwrap().as_path())
        );

        fs::write(candidate.join("skills/caveman/SKILL.md"), "changed\n").unwrap();
        assert!(
            validate_caveman_install(&allowed_root, &candidate)
                .unwrap_err()
                .to_string()
                .contains("tree digest mismatch")
        );
        let _ = fs::remove_dir_all(allowed_root);
    }

    #[cfg(unix)]
    #[test]
    fn tree_validation_rejects_symlinks() {
        let root = temp_dir("symlink");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("target"), "data").unwrap();
        std::os::unix::fs::symlink(root.join("target"), root.join("escape")).unwrap();
        let error = tree_sha256(&root, b"prodex-caveman-tree-v1\0").unwrap_err();
        assert!(error.to_string().contains("contains symlink"));
        let _ = fs::remove_dir_all(root);
    }
}

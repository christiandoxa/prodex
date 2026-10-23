use super::{
    OptionalToolId, ResolvedTool, TOOL_PROBE_TIMEOUT, ToolDiscoverySource, ToolHealth,
    find_path_command, invalid_tool, optional_tool_descriptor, validate_manifest_policy,
};
use crate::discovery::{managed_optimizer_roots, newest_stable_managed_version};
use anyhow::{Context, Result};
use prodex_mojo_core::optional_tools_policy::{
    OptionalToolManifestContent, OptionalToolManifestIdentity, OptionalToolPolicyId,
    OptionalToolReleaseIdentity, optional_tool_manifest_content_error,
    optional_tool_manifest_header_error, optional_tool_manifest_identity_error,
    optional_tool_manifest_version_error, optional_tool_stable_version,
};
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
    for root in managed_optimizer_roots() {
        let Some((version, candidate)) = newest_stable_managed_version(&root, "ponytail")? else {
            continue;
        };
        anyhow::ensure!(
            prodex_mojo_core::optional_tools_policy::optional_tool_version_at_least(
                &version,
                crate::PONYTAIL_MINIMUM_SUPPORTED_VERSION
            )
            .map_err(|error| anyhow::anyhow!("failed to validate Ponytail version: {error:?}"))?,
            "installed Ponytail {version} is too old; Prodex requires {} or newer. Update Ponytail to the latest stable release (release-qualified reference: {})",
            crate::PONYTAIL_MINIMUM_SUPPORTED_VERSION,
            crate::PONYTAIL_LATEST_STABLE_REFERENCE
        );
        return Ok(Some((root, candidate)));
    }
    Ok(None)
}

fn validate_manifest_content(manifest: &ToolInstallManifest, actual_digest: &str) -> Result<()> {
    validate_manifest_policy(optional_tool_manifest_content_error(
        OptionalToolPolicyId::Ponytail,
        OptionalToolManifestContent {
            version: &manifest.version,
            commit: &manifest.commit,
            tree_sha256: &manifest.tree_sha256,
            actual_tree_sha256: actual_digest,
            latest: OptionalToolReleaseIdentity {
                version: crate::PONYTAIL_LATEST_STABLE_REFERENCE,
                commit: crate::PONYTAIL_LATEST_STABLE_COMMIT,
                tree_sha256: crate::PONYTAIL_LATEST_STABLE_TREE_SHA256,
            },
            previous: OptionalToolReleaseIdentity {
                version: crate::PONYTAIL_PREVIOUS_STABLE_REFERENCE,
                commit: crate::PONYTAIL_PREVIOUS_STABLE_COMMIT,
                tree_sha256: crate::PONYTAIL_PREVIOUS_STABLE_TREE_SHA256,
            },
            legacy: None,
            legacy_manifest_tree_sha256: crate::PONYTAIL_PREVIOUS_LEGACY_MANIFEST_TREE_SHA256,
        },
    ))
}

fn validate_install(allowed_root: &Path, candidate: &Path) -> Result<ResolvedTool> {
    let candidate = validated_managed_directory(allowed_root, candidate)?;
    let manifest_path = candidate.join(TOOL_MANIFEST);
    let manifest: ToolInstallManifest =
        serde_json::from_slice(&crate::tree::read_bounded_file(&manifest_path, 64 * 1024)?)
            .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    validate_manifest_policy(optional_tool_manifest_header_error(
        OptionalToolPolicyId::Ponytail,
        manifest.schema_version,
        &manifest.id,
    ))?;
    let version =
        optional_tool_stable_version(&manifest.version, crate::PONYTAIL_MINIMUM_SUPPORTED_VERSION)
            .map_err(|error| {
                anyhow::anyhow!("failed to validate Ponytail manifest version: {error:?}")
            })?;
    validate_manifest_policy(optional_tool_manifest_version_error(
        OptionalToolPolicyId::Ponytail,
        version,
        &manifest.version,
        crate::PONYTAIL_MINIMUM_SUPPORTED_VERSION,
        crate::PONYTAIL_LATEST_STABLE_REFERENCE,
    ))?;
    validate_manifest_policy(optional_tool_manifest_identity_error(
        OptionalToolPolicyId::Ponytail,
        OptionalToolManifestIdentity {
            version: &manifest.version,
            directory_version: candidate.file_name().and_then(|name| name.to_str()),
            source: &manifest.source,
            expected_source: PONYTAIL_SOURCE,
            commit: &manifest.commit,
            tree_sha256: &manifest.tree_sha256,
        },
    ))?;

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
    validate_manifest_content(&manifest, &digest)?;

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
    fn previous_stable_legacy_manifest_requires_vetted_identity_and_tree() {
        let manifest = ToolInstallManifest {
            schema_version: 1,
            id: "ponytail".to_string(),
            version: crate::PONYTAIL_PREVIOUS_STABLE_REFERENCE.to_string(),
            source: PONYTAIL_SOURCE.to_string(),
            commit: crate::PONYTAIL_PREVIOUS_STABLE_COMMIT.to_string(),
            tree_sha256: crate::PONYTAIL_PREVIOUS_LEGACY_MANIFEST_TREE_SHA256.to_string(),
        };
        assert!(
            validate_manifest_content(&manifest, crate::PONYTAIL_PREVIOUS_STABLE_TREE_SHA256,)
                .is_ok()
        );

        let mut wrong_commit = manifest;
        wrong_commit.commit = "0123456789abcdef0123456789abcdef01234567".to_string();
        assert_eq!(
            validate_manifest_content(&wrong_commit, crate::PONYTAIL_PREVIOUS_STABLE_TREE_SHA256)
                .unwrap_err()
                .to_string(),
            "Ponytail previous-stable metadata does not match release-qualified compatibility metadata"
        );
    }

    #[test]
    fn latest_release_content_uses_vetted_metadata_and_ordered_errors() {
        let mut manifest = ToolInstallManifest {
            schema_version: 1,
            id: "ponytail".to_string(),
            version: crate::PONYTAIL_LATEST_STABLE_REFERENCE.to_string(),
            source: PONYTAIL_SOURCE.to_string(),
            commit: crate::PONYTAIL_LATEST_STABLE_COMMIT.to_string(),
            tree_sha256: crate::PONYTAIL_LATEST_STABLE_TREE_SHA256.to_string(),
        };
        assert!(
            validate_manifest_content(&manifest, crate::PONYTAIL_LATEST_STABLE_TREE_SHA256).is_ok()
        );
        for (commit, digest, expected) in [
            ("changed", "changed", "Ponytail latest-stable commit does not match release-qualified metadata".to_string()),
            (crate::PONYTAIL_LATEST_STABLE_COMMIT, "changed", "Ponytail latest-stable manifest tree digest does not match release-qualified metadata".to_string()),
            (crate::PONYTAIL_LATEST_STABLE_COMMIT, crate::PONYTAIL_LATEST_STABLE_TREE_SHA256, format!("Ponytail tree digest mismatch: expected {}, got changed", crate::PONYTAIL_LATEST_STABLE_TREE_SHA256)),
        ] {
            manifest.commit = commit.to_string();
            manifest.tree_sha256 = digest.to_string();
            assert_eq!(validate_manifest_content(&manifest, "changed").unwrap_err().to_string(), expected);
        }
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

        let manifest_path = candidate.join(TOOL_MANIFEST);
        let original: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        for (changes, expected) in [
            (
                serde_json::json!({"schema_version": 0, "id": "wrong", "version": "bad"}),
                "unsupported Ponytail manifest schema",
            ),
            (
                serde_json::json!({"id": "wrong", "version": "bad"}),
                "Ponytail manifest id must be ponytail",
            ),
            (
                serde_json::json!({"version": "v4.11.0", "source": "wrong"}),
                "invalid Ponytail version v4.11.0",
            ),
            (
                serde_json::json!({"version": "4.12.0", "source": "wrong"}),
                "Ponytail version directory does not match manifest version 4.12.0",
            ),
            (
                serde_json::json!({"source": "wrong", "commit": "bad"}),
                "unexpected Ponytail source",
            ),
            (
                serde_json::json!({"commit": "bad", "tree_sha256": "bad"}),
                "Ponytail manifest commit must be a 40-character Git SHA",
            ),
            (
                serde_json::json!({"tree_sha256": "bad"}),
                "Ponytail manifest tree digest must be SHA-256",
            ),
        ] {
            let mut invalid = original.clone();
            for (field, value) in changes.as_object().unwrap() {
                invalid[field] = value.clone();
            }
            fs::write(&manifest_path, serde_json::to_vec(&invalid).unwrap()).unwrap();
            assert_eq!(
                validate_install(&allowed_root, &candidate)
                    .unwrap_err()
                    .to_string(),
                expected
            );
        }
        fs::write(&manifest_path, serde_json::to_vec(&original).unwrap()).unwrap();
        fs::write(candidate.join("skills/README.md"), "changed\n").unwrap();
        let actual = crate::tree::tree_sha256(&candidate, b"prodex-ponytail-tree-v1\0").unwrap();
        assert_eq!(
            validate_install(&allowed_root, &candidate)
                .unwrap_err()
                .to_string(),
            format!("Ponytail tree digest mismatch: manifest {digest}, got {actual}")
        );
        fs::remove_dir_all(allowed_root).unwrap();
    }
}

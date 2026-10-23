use crate::discovery::{managed_optimizer_roots, newest_stable_managed_version};
use crate::localization::ensure_agents_reference;
use crate::optional_tools::{
    OptionalToolId, ResolvedTool, ToolDiscoverySource, ToolHealth, ToolHealthStatus,
    optional_tool_descriptor, validate_manifest_policy,
};
use crate::tree::{read_bounded_file, tree_sha256};
use crate::{
    CAVEMAN_LATEST_STABLE_COMMIT, CAVEMAN_LATEST_STABLE_REFERENCE,
    CAVEMAN_LATEST_STABLE_TREE_SHA256, CAVEMAN_LEGACY_COMMIT, CAVEMAN_LEGACY_MANIFEST_TREE_SHA256,
    CAVEMAN_LEGACY_REFERENCE, CAVEMAN_LEGACY_TREE_SHA256, CAVEMAN_MINIMUM_SUPPORTED_VERSION,
    CAVEMAN_PREVIOUS_STABLE_COMMIT, CAVEMAN_PREVIOUS_STABLE_REFERENCE,
    CAVEMAN_PREVIOUS_STABLE_TREE_SHA256,
};
use anyhow::{Context, Result, bail, ensure};
use prodex_mojo_core::optional_tools_policy::{
    OptionalToolManifestContent, OptionalToolManifestIdentity, OptionalToolPolicyId,
    OptionalToolReleaseIdentity, optional_tool_manifest_content_error,
    optional_tool_manifest_header_error, optional_tool_manifest_identity_error,
    optional_tool_manifest_version_error, optional_tool_stable_version,
};
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
    for root in managed_optimizer_roots() {
        let Some((version, candidate)) = newest_stable_managed_version(&root, "caveman")? else {
            continue;
        };
        ensure!(
            prodex_mojo_core::optional_tools_policy::optional_tool_version_at_least(
                &version,
                CAVEMAN_MINIMUM_SUPPORTED_VERSION
            )
            .map_err(|error| anyhow::anyhow!("failed to validate Caveman version: {error:?}"))?,
            "installed Caveman {version} is too old; Prodex requires {CAVEMAN_MINIMUM_SUPPORTED_VERSION} or newer. Update Caveman to the latest stable release (release-qualified reference: {CAVEMAN_LATEST_STABLE_REFERENCE})"
        );
        return Ok(Some((root, candidate)));
    }
    Ok(None)
}

fn validate_manifest_content(manifest: &CavemanInstallManifest, actual_digest: &str) -> Result<()> {
    validate_manifest_policy(optional_tool_manifest_content_error(
        OptionalToolPolicyId::Caveman,
        OptionalToolManifestContent {
            version: &manifest.version,
            commit: &manifest.commit,
            tree_sha256: &manifest.tree_sha256,
            actual_tree_sha256: actual_digest,
            latest: OptionalToolReleaseIdentity {
                version: CAVEMAN_LATEST_STABLE_REFERENCE,
                commit: CAVEMAN_LATEST_STABLE_COMMIT,
                tree_sha256: CAVEMAN_LATEST_STABLE_TREE_SHA256,
            },
            previous: OptionalToolReleaseIdentity {
                version: CAVEMAN_PREVIOUS_STABLE_REFERENCE,
                commit: CAVEMAN_PREVIOUS_STABLE_COMMIT,
                tree_sha256: CAVEMAN_PREVIOUS_STABLE_TREE_SHA256,
            },
            legacy: Some(OptionalToolReleaseIdentity {
                version: CAVEMAN_LEGACY_REFERENCE,
                commit: CAVEMAN_LEGACY_COMMIT,
                tree_sha256: CAVEMAN_LEGACY_TREE_SHA256,
            }),
            legacy_manifest_tree_sha256: CAVEMAN_LEGACY_MANIFEST_TREE_SHA256,
        },
    ))
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
    validate_manifest_policy(optional_tool_manifest_header_error(
        OptionalToolPolicyId::Caveman,
        manifest.schema_version,
        &manifest.id,
    ))?;

    let version =
        optional_tool_stable_version(&manifest.version, CAVEMAN_MINIMUM_SUPPORTED_VERSION)
            .map_err(|error| {
                anyhow::anyhow!("failed to validate Caveman manifest version: {error:?}")
            })?;
    validate_manifest_policy(optional_tool_manifest_version_error(
        OptionalToolPolicyId::Caveman,
        version,
        &manifest.version,
        CAVEMAN_MINIMUM_SUPPORTED_VERSION,
        CAVEMAN_LATEST_STABLE_REFERENCE,
    ))?;
    validate_manifest_policy(optional_tool_manifest_identity_error(
        OptionalToolPolicyId::Caveman,
        OptionalToolManifestIdentity {
            version: &manifest.version,
            directory_version: candidate.file_name().and_then(|name| name.to_str()),
            source: &manifest.source,
            expected_source: CAVEMAN_SOURCE,
            commit: &manifest.commit,
            tree_sha256: &manifest.tree_sha256,
        },
    ))?;

    validate_required_files(&candidate)?;
    let actual_digest = tree_sha256(&candidate, b"prodex-caveman-tree-v1\0")?;
    validate_manifest_content(&manifest, &actual_digest)?;

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
    fn compiled_tree_order_preserves_unicode_nested_digest_bytes() {
        let root = temp_dir("compiled-tree-order");
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        for (name, text) in [
            ("雪.txt", "snow"),
            ("z.txt", "last-ascii"),
            ("nested/a.txt", "nested"),
            ("ø.txt", "accent"),
            ("A.txt", "upper"),
            ("prodex-tool.json", "ignored manifest"),
            (".git/ignored", "ignored metadata"),
        ] {
            fs::write(root.join(name), text).unwrap();
        }
        assert_eq!(
            tree_sha256(&root, b"prodex-caveman-tree-v1\0").unwrap(),
            "d4984b74a6d0d60f92ad87eeb72085f2d2e567f007efed3e35d06a3e0ff36544"
        );
        assert_eq!(
            tree_sha256(&root, b"prodex-ponytail-tree-v1\0").unwrap(),
            "760ea1908aeddc299e2bebbdb2683b758b517d74be1b512ea048618eb8f0002a"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn previous_stable_and_legacy_manifests_require_vetted_identity_and_tree() {
        let previous = CavemanInstallManifest {
            schema_version: 1,
            id: "caveman".to_string(),
            version: CAVEMAN_PREVIOUS_STABLE_REFERENCE.to_string(),
            source: CAVEMAN_SOURCE.to_string(),
            commit: CAVEMAN_PREVIOUS_STABLE_COMMIT.to_string(),
            tree_sha256: CAVEMAN_PREVIOUS_STABLE_TREE_SHA256.to_string(),
        };
        assert!(validate_manifest_content(&previous, CAVEMAN_PREVIOUS_STABLE_TREE_SHA256).is_ok());

        let mut legacy = CavemanInstallManifest {
            schema_version: 1,
            id: "caveman".to_string(),
            version: CAVEMAN_LEGACY_REFERENCE.to_string(),
            source: CAVEMAN_SOURCE.to_string(),
            commit: CAVEMAN_LEGACY_COMMIT.to_string(),
            tree_sha256: CAVEMAN_LEGACY_MANIFEST_TREE_SHA256.to_string(),
        };
        assert!(validate_manifest_content(&legacy, CAVEMAN_LEGACY_TREE_SHA256).is_ok());
        legacy.commit = "0123456789abcdef0123456789abcdef01234567".to_string();
        assert!(validate_manifest_content(&legacy, &legacy.tree_sha256).is_ok());
        assert_eq!(
            validate_manifest_content(&legacy, CAVEMAN_LEGACY_TREE_SHA256)
                .unwrap_err()
                .to_string(),
            format!(
                "Caveman tree digest mismatch: manifest {CAVEMAN_LEGACY_MANIFEST_TREE_SHA256}, got {CAVEMAN_LEGACY_TREE_SHA256}"
            )
        );

        let mut wrong_commit = previous;
        wrong_commit.commit = "0123456789abcdef0123456789abcdef01234567".to_string();
        assert_eq!(
            validate_manifest_content(&wrong_commit, CAVEMAN_PREVIOUS_STABLE_TREE_SHA256)
                .unwrap_err()
                .to_string(),
            "previously release-qualified Caveman 3.2.0 identity or content changed"
        );
    }

    #[test]
    fn latest_release_content_uses_vetted_metadata_and_ordered_errors() {
        let mut manifest = CavemanInstallManifest {
            schema_version: 1,
            id: "caveman".to_string(),
            version: CAVEMAN_LATEST_STABLE_REFERENCE.to_string(),
            source: CAVEMAN_SOURCE.to_string(),
            commit: CAVEMAN_LATEST_STABLE_COMMIT.to_string(),
            tree_sha256: CAVEMAN_LATEST_STABLE_TREE_SHA256.to_string(),
        };
        assert!(validate_manifest_content(&manifest, CAVEMAN_LATEST_STABLE_TREE_SHA256).is_ok());
        for (commit, digest, expected) in [
            ("changed", "changed", "Caveman latest-stable commit does not match release-qualified metadata".to_string()),
            (CAVEMAN_LATEST_STABLE_COMMIT, "changed", "Caveman latest-stable manifest tree digest does not match release-qualified metadata".to_string()),
            (CAVEMAN_LATEST_STABLE_COMMIT, CAVEMAN_LATEST_STABLE_TREE_SHA256, format!("Caveman tree digest mismatch: expected {CAVEMAN_LATEST_STABLE_TREE_SHA256}, got changed")),
        ] {
            manifest.commit = commit.to_string();
            manifest.tree_sha256 = digest.to_string();
            assert_eq!(validate_manifest_content(&manifest, "changed").unwrap_err().to_string(), expected);
        }
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

        let manifest_path = candidate.join(TOOL_MANIFEST);
        let original: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        for (changes, expected) in [
            (
                serde_json::json!({"schema_version": 0, "id": "wrong", "version": "bad"}),
                "unsupported Caveman manifest schema",
            ),
            (
                serde_json::json!({"id": "wrong", "version": "bad"}),
                "Caveman manifest id must be caveman",
            ),
            (
                serde_json::json!({"version": "v2.3.1", "source": "wrong"}),
                "invalid Caveman version v2.3.1",
            ),
            (
                serde_json::json!({"version": "2.3.2", "source": "wrong"}),
                "Caveman version directory does not match manifest version 2.3.2",
            ),
            (
                serde_json::json!({"source": "wrong", "commit": "bad"}),
                "unexpected Caveman source",
            ),
            (
                serde_json::json!({"commit": "bad", "tree_sha256": "bad"}),
                "Caveman manifest commit must be a 40-character Git SHA",
            ),
            (
                serde_json::json!({"tree_sha256": "bad"}),
                "Caveman manifest tree digest must be SHA-256",
            ),
        ] {
            let mut invalid = original.clone();
            for (field, value) in changes.as_object().unwrap() {
                invalid[field] = value.clone();
            }
            fs::write(&manifest_path, serde_json::to_vec(&invalid).unwrap()).unwrap();
            assert_eq!(
                validate_caveman_install(&allowed_root, &candidate)
                    .unwrap_err()
                    .to_string(),
                expected
            );
        }
        fs::write(&manifest_path, serde_json::to_vec(&original).unwrap()).unwrap();

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

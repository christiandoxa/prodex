use crate::discovery::managed_optimizer_roots;
use crate::localization::ensure_agents_reference;
use crate::optional_tools::{
    OptionalToolId, ResolvedTool, ToolDiscoverySource, ToolHealth, ToolHealthStatus,
    optional_tool_descriptor,
};
use crate::tree::{path_exists, read_bounded_file, tree_sha256};
use crate::{CAVEMAN_VETTED_COMMIT, CAVEMAN_VETTED_TREE_SHA256, CAVEMAN_VETTED_VERSION};
use anyhow::{Context, Result, bail, ensure};
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
            "Caveman {CAVEMAN_VETTED_VERSION} is not installed; run `prodex capability super-doctor` and install it under a managed optional-tool root"
        );
    };
    validate_caveman_install(
        &allowed_root,
        &candidate,
        CAVEMAN_VETTED_VERSION,
        CAVEMAN_VETTED_COMMIT,
        CAVEMAN_VETTED_TREE_SHA256,
    )
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
                "expected caveman/{CAVEMAN_VETTED_VERSION}/{TOOL_MANIFEST} under a managed optional-tool root"
            ),
        );
    };
    match validate_caveman_install(
        &allowed_root,
        &candidate,
        CAVEMAN_VETTED_VERSION,
        CAVEMAN_VETTED_COMMIT,
        CAVEMAN_VETTED_TREE_SHA256,
    ) {
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
        let versioned = root.join("caveman").join(CAVEMAN_VETTED_VERSION);
        if path_exists(&versioned)? {
            return Ok(Some((root, versioned)));
        }
        let compatibility = root.join("caveman");
        if path_exists(&compatibility)? {
            return Ok(Some((root, compatibility)));
        }
    }
    Ok(None)
}

fn validate_caveman_install(
    allowed_root: &Path,
    candidate: &Path,
    expected_version: &str,
    expected_commit: &str,
    expected_tree_sha256: &str,
) -> Result<ResolvedTool> {
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
    ensure!(
        manifest.version == expected_version,
        "Caveman version {} is not vetted version {expected_version}",
        manifest.version
    );
    ensure!(
        manifest.source == CAVEMAN_SOURCE,
        "unexpected Caveman source"
    );
    ensure!(
        manifest.commit == expected_commit,
        "Caveman commit does not match vetted metadata"
    );
    ensure!(
        manifest.tree_sha256 == expected_tree_sha256,
        "Caveman manifest tree digest does not match vetted metadata"
    );

    validate_required_files(&candidate)?;
    let actual_digest = tree_sha256(&candidate, b"prodex-caveman-tree-v1\0")?;
    ensure!(
        actual_digest == expected_tree_sha256,
        "Caveman tree digest mismatch: expected {expected_tree_sha256}, got {actual_digest}"
    );
    Ok(ResolvedTool {
        descriptor: optional_tool_descriptor(OptionalToolId::Caveman),
        source: ToolDiscoverySource::ManagedRoot,
        path: Some(candidate),
        version: Some(expected_version.to_string()),
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
        let candidate = allowed_root.join("caveman/1.2.3");
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
                "version": "1.2.3",
                "source": CAVEMAN_SOURCE,
                "commit": "0123456789abcdef0123456789abcdef01234567",
                "tree_sha256": digest,
            }))
            .unwrap(),
        )
        .unwrap();

        let tool = validate_caveman_install(
            &allowed_root,
            &candidate,
            "1.2.3",
            "0123456789abcdef0123456789abcdef01234567",
            &digest,
        )
        .unwrap();
        assert_eq!(
            tool.path.as_deref(),
            Some(candidate.canonicalize().unwrap().as_path())
        );

        fs::write(candidate.join("skills/caveman/SKILL.md"), "changed\n").unwrap();
        assert!(
            validate_caveman_install(
                &allowed_root,
                &candidate,
                "1.2.3",
                "0123456789abcdef0123456789abcdef01234567",
                &digest,
            )
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

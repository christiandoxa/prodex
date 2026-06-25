use crate::embedded_files::{
    CAVEMAN_COMPRESS_PLUGIN_FILES, CAVEMAN_CORE_PLUGIN_FILES, CLAUDE_CAVEMAN_PLUGIN_FILES,
    EmbeddedCavemanFile,
};
use crate::{CavemanAssetVerification, PRODEX_CAVEMAN_PLUGIN_VERSION};
use anyhow::{Context, Result, bail};

pub(crate) fn verify_embedded_caveman_assets() -> Result<CavemanAssetVerification> {
    verify_file_set("codex", CAVEMAN_CORE_PLUGIN_FILES)?;
    verify_file_set("codex-compress", CAVEMAN_COMPRESS_PLUGIN_FILES)?;
    verify_file_set("claude", CLAUDE_CAVEMAN_PLUGIN_FILES)?;
    verify_codex_manifest()?;
    verify_claude_manifest()?;

    Ok(CavemanAssetVerification {
        codex_plugin_files: CAVEMAN_CORE_PLUGIN_FILES.len() + CAVEMAN_COMPRESS_PLUGIN_FILES.len(),
        claude_plugin_files: CLAUDE_CAVEMAN_PLUGIN_FILES.len(),
        skill_files: CAVEMAN_CORE_PLUGIN_FILES
            .iter()
            .chain(CAVEMAN_COMPRESS_PLUGIN_FILES)
            .chain(CLAUDE_CAVEMAN_PLUGIN_FILES)
            .filter(|file| file.relative_path.ends_with("/SKILL.md"))
            .count(),
    })
}

fn verify_file_set(label: &str, files: &[EmbeddedCavemanFile]) -> Result<()> {
    for file in files {
        if file.relative_path.trim().is_empty() {
            bail!("{label} embedded asset has empty relative path");
        }
        if file.relative_path.starts_with('/') || file.relative_path.contains("..") {
            bail!(
                "{label} embedded asset has unsafe relative path: {}",
                file.relative_path
            );
        }
        if file.contents.trim().is_empty() {
            bail!(
                "{label} embedded asset {} has empty contents",
                file.relative_path
            );
        }
        if file.relative_path.ends_with("/SKILL.md") {
            verify_skill_frontmatter(label, file)?;
        }
    }
    Ok(())
}

fn verify_skill_frontmatter(label: &str, file: &EmbeddedCavemanFile) -> Result<()> {
    let contents = file.contents.trim_start();
    if !contents.starts_with("---\n") {
        bail!(
            "{label} embedded skill {} is missing YAML frontmatter",
            file.relative_path
        );
    }
    let Some(frontmatter_end) = contents[4..].find("\n---") else {
        bail!(
            "{label} embedded skill {} has unterminated YAML frontmatter",
            file.relative_path
        );
    };
    let frontmatter = &contents[4..4 + frontmatter_end];
    if !frontmatter.lines().any(|line| line.starts_with("name:")) {
        bail!(
            "{label} embedded skill {} is missing name frontmatter",
            file.relative_path
        );
    }
    if !frontmatter
        .lines()
        .any(|line| line.starts_with("description:"))
    {
        bail!(
            "{label} embedded skill {} is missing description frontmatter",
            file.relative_path
        );
    }
    Ok(())
}

fn verify_codex_manifest() -> Result<()> {
    let manifest = CAVEMAN_CORE_PLUGIN_FILES
        .iter()
        .find(|file| file.relative_path == ".codex-plugin/plugin.json")
        .context("Codex Caveman manifest is missing")?;
    let value: serde_json::Value = serde_json::from_str(manifest.contents)
        .context("Codex Caveman manifest is invalid JSON")?;
    if value.get("name").and_then(serde_json::Value::as_str) != Some("caveman") {
        bail!("Codex Caveman manifest name must be caveman");
    }
    if value.get("version").and_then(serde_json::Value::as_str)
        != Some(PRODEX_CAVEMAN_PLUGIN_VERSION)
    {
        bail!("Codex Caveman manifest version does not match embedded Prodex version");
    }
    if value.get("skills").and_then(serde_json::Value::as_str) != Some("./skills/") {
        bail!("Codex Caveman manifest skills pointer must be ./skills/");
    }
    if value
        .pointer("/interface/logoDark")
        .and_then(serde_json::Value::as_str)
        != Some("./assets/caveman-dark.svg")
    {
        bail!("Codex Caveman manifest logoDark pointer must be ./assets/caveman-dark.svg");
    }
    if !CAVEMAN_CORE_PLUGIN_FILES
        .iter()
        .any(|file| file.relative_path == "assets/caveman-dark.svg")
    {
        bail!("Codex Caveman dark-mode logo asset is missing");
    }
    Ok(())
}

fn verify_claude_manifest() -> Result<()> {
    let manifest = CLAUDE_CAVEMAN_PLUGIN_FILES
        .iter()
        .find(|file| file.relative_path == ".claude-plugin/plugin.json")
        .context("Claude Caveman manifest is missing")?;
    let value: serde_json::Value = serde_json::from_str(manifest.contents)
        .context("Claude Caveman manifest is invalid JSON")?;
    if value.get("name").and_then(serde_json::Value::as_str) != Some("caveman") {
        bail!("Claude Caveman manifest name must be caveman");
    }
    Ok(())
}

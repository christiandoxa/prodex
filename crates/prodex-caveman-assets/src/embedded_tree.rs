use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::embedded_files::{
    CAVEMAN_COMPRESS_PLUGIN_FILES, CAVEMAN_CORE_PLUGIN_FILES, CLAUDE_CAVEMAN_PLUGIN_FILES,
    EmbeddedCavemanFile,
};
use crate::{PRODEX_CAVEMAN_FULL_ASSETS_ENV, PRODEX_CLAUDE_CAVEMAN_PLUGIN_NAME};

pub fn install_claude_caveman_plugin(prodex_root: &Path) -> Result<PathBuf> {
    let plugin_dir = prodex_root
        .join("claude-plugins")
        .join(PRODEX_CLAUDE_CAVEMAN_PLUGIN_NAME);
    write_caveman_plugin_files(&plugin_dir, CLAUDE_CAVEMAN_PLUGIN_FILES)?;
    Ok(plugin_dir)
}

pub(crate) fn write_caveman_plugin_tree(root: &Path) -> Result<()> {
    write_caveman_plugin_files(root, CAVEMAN_CORE_PLUGIN_FILES)?;
    if caveman_full_assets_enabled() {
        write_caveman_plugin_files(root, CAVEMAN_COMPRESS_PLUGIN_FILES)?;
    }
    Ok(())
}

pub(crate) fn write_caveman_plugin_files(root: &Path, files: &[EmbeddedCavemanFile]) -> Result<()> {
    for file in files {
        let path = root.join(file.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, file.contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn caveman_full_assets_enabled() -> bool {
    env::var(PRODEX_CAVEMAN_FULL_ASSETS_ENV)
        .ok()
        .is_some_and(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "" | "0" | "false" | "no" | "off"
            )
        })
}

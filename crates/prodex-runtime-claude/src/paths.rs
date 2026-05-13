use crate::constants::{
    DEFAULT_CLAUDE_CONFIG_DIR_NAME, DEFAULT_CLAUDE_CONFIG_FILE_NAME,
    DEFAULT_CLAUDE_SETTINGS_FILE_NAME, PRODEX_CLAUDE_CONFIG_DIR_NAME,
    PRODEX_CLAUDE_LEGACY_IMPORT_MARKER_NAME, PRODEX_SHARED_CLAUDE_DIR_NAME,
};
use anyhow::{Context, Result};
use dirs::home_dir;
use std::path::{Path, PathBuf};

pub fn runtime_proxy_claude_config_value(codex_home: &Path, key: &str) -> Option<String> {
    codex_config::codex_config_value(codex_home, key)
}

pub fn runtime_proxy_claude_config_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(PRODEX_CLAUDE_CONFIG_DIR_NAME)
}

pub fn runtime_proxy_shared_claude_config_dir(prodex_root: &Path) -> PathBuf {
    prodex_root.join(PRODEX_SHARED_CLAUDE_DIR_NAME)
}

pub fn runtime_proxy_claude_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join(DEFAULT_CLAUDE_CONFIG_FILE_NAME)
}

pub fn runtime_proxy_claude_settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(DEFAULT_CLAUDE_SETTINGS_FILE_NAME)
}

pub fn runtime_proxy_claude_legacy_import_marker_path(config_dir: &Path) -> PathBuf {
    config_dir.join(PRODEX_CLAUDE_LEGACY_IMPORT_MARKER_NAME)
}

pub fn legacy_default_claude_config_dir() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CLAUDE_CONFIG_DIR_NAME))
}

pub fn legacy_default_claude_config_path() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CLAUDE_CONFIG_FILE_NAME))
}

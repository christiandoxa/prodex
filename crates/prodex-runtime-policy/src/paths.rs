use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

use crate::types::PRODEX_POLICY_FILE_NAME;

pub fn runtime_policy_path(root: &Path) -> PathBuf {
    root.join(PRODEX_POLICY_FILE_NAME)
}

pub fn resolve_runtime_policy_path(root: &Path, value: &str) -> Result<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("policy path values cannot be empty");
    }
    let path = PathBuf::from(trimmed);
    Ok(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

use anyhow::{Result, bail};
#[cfg(test)]
use std::env;
use std::path::{Path, PathBuf};

use super::types::PRODEX_POLICY_FILE_NAME;

pub(crate) fn runtime_policy_path(root: &Path) -> PathBuf {
    root.join(PRODEX_POLICY_FILE_NAME)
}

#[cfg(test)]
pub(crate) fn runtime_policy_enabled_for_current_process() -> bool {
    env::var_os("PRODEX_HOME").is_some()
}

#[cfg(not(test))]
pub(crate) fn runtime_policy_enabled_for_current_process() -> bool {
    true
}

pub(crate) fn resolve_runtime_policy_path(root: &Path, value: &str) -> Result<PathBuf> {
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

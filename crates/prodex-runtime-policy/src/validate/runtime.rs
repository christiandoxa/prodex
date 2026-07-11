use crate::types::RuntimePolicyFile;
use anyhow::{Result, bail};
use std::path::Path;

pub(super) fn validate_runtime_policy(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    if policy
        .runtime
        .log_dir
        .as_deref()
        .is_some_and(|log_dir| log_dir.trim().is_empty())
    {
        bail!("runtime.log_dir in {} cannot be empty", path.display());
    }
    Ok(())
}

use super::super::ensure_managed_profiles_root;
use crate::AppPaths;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn create_temporary_login_home(paths: &AppPaths) -> Result<PathBuf> {
    ensure_managed_profiles_root(paths)?;

    for attempt in 0..100 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let candidate = paths
            .managed_profiles_root
            .join(format!(".login-{}-{stamp}-{attempt}", std::process::id()));
        if candidate.exists() {
            continue;
        }
        secret_store::ensure_private_directory(&candidate)
            .with_context(|| format!("failed to secure {}", candidate.display()))?;
        return Ok(candidate);
    }

    bail!("failed to allocate a temporary CODEX_HOME for login")
}

use super::super::ensure_managed_profiles_root;
use crate::AppPaths;
use anyhow::{Context, Result, bail};
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct TemporaryLoginHome(PathBuf);

impl Deref for TemporaryLoginHome {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for TemporaryLoginHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn create_temporary_login_home(paths: &AppPaths) -> Result<TemporaryLoginHome> {
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
        return Ok(TemporaryLoginHome(candidate));
    }

    bail!("failed to allocate a temporary CODEX_HOME for login")
}

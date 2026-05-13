use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::{
    IMPORT_AUTH_UPDATE_JOURNAL_DIR, IMPORT_AUTH_UPDATE_JOURNAL_VERSION, ImportedProfilesCommit,
};

pub fn validate_import_auth_update_journal_version(version: u32) -> Result<()> {
    if version != IMPORT_AUTH_UPDATE_JOURNAL_VERSION {
        bail!("unsupported auth update journal version {}", version);
    }
    Ok(())
}

pub fn profile_import_auth_update_journal_root(prodex_root: impl AsRef<Path>) -> PathBuf {
    prodex_root.as_ref().join(IMPORT_AUTH_UPDATE_JOURNAL_DIR)
}

pub fn profile_import_auth_update_journal_paths(
    prodex_root: impl AsRef<Path>,
) -> Result<Vec<PathBuf>> {
    let journal_root = profile_import_auth_update_journal_root(prodex_root);
    let entries = match fs::read_dir(&journal_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", journal_root.display()));
        }
    };
    let mut journal_paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .with_context(|| format!("failed to read entry in {}", journal_root.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    journal_paths.retain(|path| path.is_file());
    journal_paths.sort();
    Ok(journal_paths)
}

pub fn ensure_profile_import_auth_update_journal_root(
    prodex_root: impl AsRef<Path>,
) -> Result<PathBuf> {
    let journal_root = profile_import_auth_update_journal_root(prodex_root);
    fs::create_dir_all(&journal_root)
        .with_context(|| format!("failed to create {}", journal_root.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&journal_root, permissions)
            .with_context(|| format!("failed to secure {}", journal_root.display()))?;
    }
    Ok(journal_root)
}

pub fn unique_profile_import_auth_update_journal_path(
    prodex_root: impl AsRef<Path>,
    profile_name: &str,
    token: &str,
) -> Result<PathBuf> {
    let journal_root = ensure_profile_import_auth_update_journal_root(prodex_root)?;
    Ok(journal_root.join(format!("{profile_name}-{token}.json")))
}

pub fn profile_import_staging_home(
    managed_profiles_root: impl AsRef<Path>,
    profile_name: &str,
    token: &str,
) -> PathBuf {
    managed_profiles_root
        .as_ref()
        .join(format!(".import-{profile_name}-{token}"))
}

pub fn cleanup_imported_auth_update_journals(commit: &ImportedProfilesCommit) {
    for update in &commit.auth_updates {
        let Some(journal_path) = update.journal_path.as_deref() else {
            continue;
        };
        let _ = fs::remove_file(journal_path);
        if let Some(parent) = journal_path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

pub fn remove_committed_import_homes(committed_homes: &[PathBuf]) {
    for home in committed_homes.iter().rev() {
        let _ = fs::remove_dir_all(home);
    }
}

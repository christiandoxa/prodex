use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use zeroize::Zeroizing;

use crate::{
    IMPORT_AUTH_UPDATE_JOURNAL_DIR, IMPORT_AUTH_UPDATE_JOURNAL_VERSION,
    ImportedExistingProfileAuthUpdateJournal, ImportedProfilesCommit,
};

const IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES: u64 = 1024 * 1024;

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
    if !ensure_import_auth_update_journal_root_is_directory(&journal_root, false)? {
        return Ok(Vec::new());
    }
    let entries = match fs::read_dir(&journal_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", journal_root.display()));
        }
    };
    let mut journal_paths = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", journal_root.display()))?;
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?
            .is_file()
        {
            journal_paths.push(entry.path());
        }
    }
    journal_paths.sort();
    Ok(journal_paths)
}

pub fn read_profile_import_auth_update_journal(
    path: impl AsRef<Path>,
) -> Result<ImportedExistingProfileAuthUpdateJournal> {
    let path = path.as_ref();
    let bytes = read_profile_import_auth_update_journal_bytes(path)?;
    let journal: ImportedExistingProfileAuthUpdateJournal = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_import_auth_update_journal_version(journal.version)
        .with_context(|| format!("in {}", path.display()))?;
    Ok(journal)
}

pub fn write_profile_import_auth_update_journal(
    path: impl AsRef<Path>,
    journal: &ImportedExistingProfileAuthUpdateJournal,
) -> Result<()> {
    let path = path.as_ref();
    let bytes = Zeroizing::new(
        serde_json::to_vec_pretty(journal)
            .context("failed to serialize profile import auth update journal")?,
    );
    if bytes.len() as u64 > IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES {
        bail!(
            "profile import auth update journal {} exceeds safe size limit ({} bytes)",
            path.display(),
            IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES
        );
    }
    secret_store::write_private_file_atomic(path, &bytes)
        .with_context(|| format!("failed to replace {}", path.display()))
}

pub fn ensure_profile_import_auth_update_journal_root(
    prodex_root: impl AsRef<Path>,
) -> Result<PathBuf> {
    let journal_root = profile_import_auth_update_journal_root(prodex_root);
    fs::create_dir_all(&journal_root)
        .with_context(|| format!("failed to create {}", journal_root.display()))?;
    ensure_import_auth_update_journal_root_is_directory(&journal_root, true)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&journal_root, permissions)
            .with_context(|| format!("failed to secure {}", journal_root.display()))?;
    }
    Ok(journal_root)
}

fn ensure_import_auth_update_journal_root_is_directory(
    journal_root: &Path,
    required: bool,
) -> Result<bool> {
    let metadata = match fs::symlink_metadata(journal_root) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && !required => return Ok(false),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", journal_root.display()));
        }
    };
    if metadata.file_type().is_symlink() {
        bail!(
            "profile import auth update journal root {} is a symlink",
            journal_root.display()
        );
    }
    if !metadata.is_dir() {
        bail!(
            "profile import auth update journal root {} is not a directory",
            journal_root.display()
        );
    }
    Ok(true)
}

fn read_profile_import_auth_update_journal_bytes(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    match secret_store::read_private_file_bounded(path, IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES) {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => bail!("failed to read {}", path.display()),
        Err(error)
            if error.kind() == std::io::ErrorKind::InvalidData
                && error.to_string().contains("safe size limit") =>
        {
            bail!(
                "profile import auth update journal {} exceeds safe size limit ({} bytes)",
                path.display(),
                IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES
            )
        }
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::InvalidInput
                    | std::io::ErrorKind::InvalidData
                    | std::io::ErrorKind::NotADirectory
                    | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            bail!(
                "profile import auth update journal {} is not a regular file",
                path.display()
            )
        }
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
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

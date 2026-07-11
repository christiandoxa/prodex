use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

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
    let text = read_profile_import_auth_update_journal_text(path)?;
    let journal: ImportedExistingProfileAuthUpdateJournal = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_import_auth_update_journal_version(journal.version)
        .with_context(|| format!("in {}", path.display()))?;
    Ok(journal)
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

fn read_profile_import_auth_update_journal_text(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        bail!(
            "profile import auth update journal {} is not a regular file",
            path.display()
        );
    }
    if metadata.len() > IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES {
        bail!(
            "profile import auth update journal {} exceeds safe size limit ({} bytes)",
            path.display(),
            IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES
        );
    }

    let file =
        fs::File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    let opened_metadata = file
        .metadata()
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if !same_import_auth_update_journal_metadata(&metadata, &opened_metadata) {
        bail!(
            "profile import auth update journal {} changed while reading",
            path.display()
        );
    }

    let mut text = String::new();
    file.take(IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES.saturating_add(1))
        .read_to_string(&mut text)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if text.len() as u64 > IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES {
        bail!(
            "profile import auth update journal {} exceeds safe size limit ({} bytes)",
            path.display(),
            IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES
        );
    }
    Ok(text)
}

#[cfg(unix)]
fn same_import_auth_update_journal_metadata(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn same_import_auth_update_journal_metadata(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    true
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

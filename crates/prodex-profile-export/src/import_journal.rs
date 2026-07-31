use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use zeroize::Zeroizing;

use crate::{
    IMPORT_AUTH_UPDATE_JOURNAL_DIR, IMPORT_AUTH_UPDATE_JOURNAL_VERSION,
    ImportedExistingProfileAuthUpdateJournal, ImportedProfilesCommit,
    PROFILE_LIFECYCLE_JOURNAL_DIR, PROFILE_LIFECYCLE_JOURNAL_VERSION, ProfileLifecycleJournal,
};

const IMPORT_AUTH_UPDATE_JOURNAL_MAX_BYTES: u64 = 1024 * 1024;
const PROFILE_LIFECYCLE_JOURNAL_MAX_BYTES: u64 = 1024 * 1024;

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
            validate_profile_import_auth_update_journal_path(entry.path())?;
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
    validate_profile_import_auth_update_journal_path(path)?;
    let bytes = read_profile_import_auth_update_journal_bytes(path)?;
    let journal: ImportedExistingProfileAuthUpdateJournal = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_import_auth_update_journal_version(journal.version)
        .with_context(|| format!("in {}", path.display()))?;
    validate_filename_component(&journal.profile_name, "profile name")?;
    for file in journal
        .previous_secret_files
        .iter()
        .map(|file| file.path.as_str())
        .chain(
            journal
                .next_secret_files
                .iter()
                .map(|file| file.path.as_str()),
        )
    {
        validate_filename_component(file, "auth update secret file")?;
    }
    Ok(journal)
}

pub fn write_profile_import_auth_update_journal(
    path: impl AsRef<Path>,
    journal: &ImportedExistingProfileAuthUpdateJournal,
) -> Result<()> {
    let path = path.as_ref();
    validate_journal_filename(path, "auth update journal")?;
    validate_filename_component(&journal.profile_name, "profile name")?;
    for file in journal
        .previous_secret_files
        .iter()
        .map(|file| file.path.as_str())
        .chain(
            journal
                .next_secret_files
                .iter()
                .map(|file| file.path.as_str()),
        )
    {
        validate_filename_component(file, "auth update secret file")?;
    }
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
    validate_filename_component(profile_name, "profile name")?;
    validate_filename_component(token, "auth update journal token")?;
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
    if let Some(path) = commit.lifecycle_journal_path.as_deref() {
        if validate_profile_lifecycle_journal_path(path).is_err() {
            return;
        }
        cleanup_profile_lifecycle_journal(path);
        match fs::symlink_metadata(path) {
            Ok(_) => return,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return,
        }
    }
    for update in &commit.auth_updates {
        let Some(journal_path) = update.journal_path.as_deref() else {
            continue;
        };
        if validate_profile_import_auth_update_journal_path(journal_path).is_err() {
            continue;
        }
        let _ = fs::remove_file(journal_path);
        if let Some(parent) = journal_path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

pub fn cleanup_profile_import_auth_update_journal(path: impl AsRef<Path>) {
    let path = path.as_ref();
    if validate_profile_import_auth_update_journal_path(path).is_err() {
        return;
    }
    let _ = fs::remove_file(path);
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
}

pub fn remove_committed_import_homes(committed_homes: &[PathBuf]) {
    for home in committed_homes.iter().rev() {
        let _ = fs::remove_dir_all(home);
    }
}

pub fn profile_lifecycle_journal_root(prodex_root: impl AsRef<Path>) -> PathBuf {
    prodex_root.as_ref().join(PROFILE_LIFECYCLE_JOURNAL_DIR)
}

pub fn profile_lifecycle_journal_paths(prodex_root: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    journal_paths(&profile_lifecycle_journal_root(prodex_root))
}

pub fn read_profile_lifecycle_journal(path: impl AsRef<Path>) -> Result<ProfileLifecycleJournal> {
    let path = path.as_ref();
    validate_profile_lifecycle_journal_path(path)?;
    let bytes = read_bounded_private_file(path, PROFILE_LIFECYCLE_JOURNAL_MAX_BYTES)?;
    let journal: ProfileLifecycleJournal = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if journal.version != PROFILE_LIFECYCLE_JOURNAL_VERSION {
        bail!(
            "unsupported profile lifecycle journal version {}",
            journal.version
        );
    }
    validate_profile_lifecycle_operation(&journal.operation)?;
    Ok(journal)
}

pub fn write_profile_lifecycle_journal(
    path: impl AsRef<Path>,
    journal: &ProfileLifecycleJournal,
) -> Result<()> {
    let path = path.as_ref();
    validate_profile_lifecycle_journal_path(path)?;
    validate_profile_lifecycle_operation(&journal.operation)?;
    let bytes = Zeroizing::new(
        serde_json::to_vec_pretty(journal)
            .context("failed to serialize profile lifecycle journal")?,
    );
    if bytes.len() as u64 > PROFILE_LIFECYCLE_JOURNAL_MAX_BYTES {
        bail!(
            "profile lifecycle journal {} exceeds safe size limit ({} bytes)",
            path.display(),
            PROFILE_LIFECYCLE_JOURNAL_MAX_BYTES
        );
    }
    secret_store::write_private_file_atomic(path, &bytes)
        .with_context(|| format!("failed to replace {}", path.display()))
}

pub fn unique_profile_lifecycle_journal_path(
    prodex_root: impl AsRef<Path>,
    operation: &str,
    token: &str,
) -> Result<PathBuf> {
    validate_profile_lifecycle_operation(operation)?;
    validate_filename_component(token, "profile lifecycle journal token")?;
    let root = ensure_profile_lifecycle_journal_root(prodex_root)?;
    Ok(root.join(format!("{operation}-{token}.json")))
}

pub fn validate_profile_lifecycle_operation(operation: &str) -> Result<()> {
    if matches!(operation, "import" | "login" | "manage" | "remove") {
        return Ok(());
    }
    bail!("unknown profile lifecycle operation '{operation}'")
}

pub fn validate_profile_lifecycle_journal_path(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let filename = journal_filename(path)?;
    let stem = filename
        .strip_suffix(".json")
        .context("profile lifecycle journal filename must end with .json")?;
    let (operation, token) = stem
        .split_once('-')
        .context("profile lifecycle journal filename is missing its token")?;
    validate_profile_lifecycle_operation(operation)?;
    validate_filename_component(token, "profile lifecycle journal token")
}

pub fn validate_profile_import_auth_update_journal_path(path: impl AsRef<Path>) -> Result<()> {
    validate_journal_filename(path.as_ref(), "auth update journal")
}

pub fn ensure_profile_lifecycle_journal_root(prodex_root: impl AsRef<Path>) -> Result<PathBuf> {
    let prodex_root = prodex_root.as_ref();
    fs::create_dir_all(prodex_root)
        .with_context(|| format!("failed to create {}", prodex_root.display()))?;
    secret_store::ensure_private_directory(prodex_root)
        .with_context(|| format!("failed to secure {}", prodex_root.display()))?;
    let root = profile_lifecycle_journal_root(prodex_root);
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    secret_store::ensure_private_directory(&root)
        .with_context(|| format!("failed to secure {}", root.display()))?;
    ensure_journal_root_is_directory(&root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("failed to secure {}", root.display()))?;
    }
    Ok(root)
}

pub fn cleanup_profile_lifecycle_journal(path: impl AsRef<Path>) {
    let path = path.as_ref();
    if validate_profile_lifecycle_journal_path(path).is_err() {
        return;
    }
    let _ = fs::remove_file(path);
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
}

fn journal_paths(journal_root: &Path) -> Result<Vec<PathBuf>> {
    let metadata = match fs::symlink_metadata(journal_root) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", journal_root.display()));
        }
    };
    if metadata.file_type().is_symlink() {
        bail!(
            "profile lifecycle journal root {} is a symlink",
            journal_root.display()
        );
    }
    if !metadata.is_dir() {
        bail!(
            "profile lifecycle journal root {} is not a directory",
            journal_root.display()
        );
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(journal_root)
        .with_context(|| format!("failed to read {}", journal_root.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", journal_root.display()))?;
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?
            .is_file()
        {
            validate_profile_lifecycle_journal_path(entry.path())?;
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn journal_filename(path: &Path) -> Result<&str> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        bail!("journal path contains a parent traversal component");
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .context("journal path must have a valid filename")
}

fn validate_journal_filename(path: &Path, label: &str) -> Result<()> {
    let filename = journal_filename(path)?;
    let stem = filename
        .strip_suffix(".json")
        .with_context(|| format!("{label} filename must end with .json"))?;
    validate_filename_component(stem, label)
}

fn validate_filename_component(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || value.contains('/')
        || value.contains('\\')
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        bail!("{label} contains an unsafe filename component")
    }
    Ok(())
}

fn ensure_journal_root_is_directory(root: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(root)
        .with_context(|| format!("failed to inspect {}", root.display()))?;
    if metadata.file_type().is_symlink() {
        bail!(
            "profile lifecycle journal root {} is a symlink",
            root.display()
        );
    }
    if !metadata.is_dir() {
        bail!(
            "profile lifecycle journal root {} is not a directory",
            root.display()
        );
    }
    Ok(())
}

fn read_bounded_private_file(path: &Path, max_bytes: u64) -> Result<Zeroizing<Vec<u8>>> {
    match secret_store::read_private_file_bounded(path, max_bytes) {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => bail!("failed to read {}", path.display()),
        Err(error)
            if error.kind() == std::io::ErrorKind::InvalidData
                && error.to_string().contains("safe size limit") =>
        {
            bail!(
                "profile lifecycle journal {} exceeds safe size limit ({} bytes)",
                path.display(),
                max_bytes
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
                "profile lifecycle journal {} is not a regular file",
                path.display()
            )
        }
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

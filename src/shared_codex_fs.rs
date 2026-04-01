use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SharedCodexEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SharedCodexEntry {
    name: String,
    kind: SharedCodexEntryKind,
}

pub(crate) fn copy_codex_home(source: &Path, destination: &Path) -> Result<()> {
    if !source.is_dir() {
        bail!("copy source {} is not a directory", source.display());
    }

    if same_path(source, destination) {
        bail!("copy source and destination are the same path");
    }

    if destination.exists() && !dir_is_empty(destination)? {
        bail!(
            "destination {} already exists and is not empty",
            destination.display()
        );
    }

    create_codex_home_if_missing(destination)?;
    copy_directory_contents(source, destination)
}

pub(crate) fn copy_directory_contents(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory {}", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source.display()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read metadata for {}", source_path.display()))?;

        if file_type.is_dir() {
            create_codex_home_if_missing(&destination_path)?;
            copy_directory_contents(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        } else if file_type.is_symlink() {
            #[cfg(unix)]
            {
                let target = fs::read_link(&source_path)
                    .with_context(|| format!("failed to read symlink {}", source_path.display()))?;
                std::os::unix::fs::symlink(target, &destination_path).with_context(|| {
                    format!("failed to recreate symlink {}", destination_path.display())
                })?;
            }
            #[cfg(not(unix))]
            {
                bail!("symlinks are not supported on this platform");
            }
        }
    }

    Ok(())
}

pub(crate) fn prepare_managed_codex_home(paths: &AppPaths, codex_home: &Path) -> Result<()> {
    create_codex_home_if_missing(codex_home)?;
    migrate_legacy_shared_codex_root(paths)?;
    seed_legacy_default_codex_home(paths)?;
    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    for entry in shared_codex_entries(paths, codex_home)? {
        ensure_shared_codex_entry(paths, codex_home, &entry)?;
    }

    Ok(())
}

fn seed_legacy_default_codex_home(paths: &AppPaths) -> Result<()> {
    if env::var_os("PRODEX_SHARED_CODEX_HOME").is_some() {
        return Ok(());
    }

    let legacy_root = legacy_default_codex_home()?;
    if same_path(&paths.shared_codex_root, &legacy_root) || !legacy_root.is_dir() {
        return Ok(());
    }

    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    let mut entries = SHARED_CODEX_DIR_NAMES
        .iter()
        .map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::Directory,
        })
        .chain(SHARED_CODEX_FILE_NAMES.iter().map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::File,
        }))
        .collect::<Vec<_>>();

    let mut sqlite_entries = BTreeSet::new();
    collect_shared_codex_sqlite_entries(&legacy_root, &mut sqlite_entries)?;
    for name in sqlite_entries {
        entries.push(SharedCodexEntry {
            name,
            kind: SharedCodexEntryKind::File,
        });
    }

    for entry in entries {
        let legacy_path = legacy_root.join(&entry.name);
        let shared_path = paths.shared_codex_root.join(&entry.name);
        seed_shared_codex_entry(&legacy_path, &shared_path, entry.kind)?;
    }

    Ok(())
}

fn migrate_legacy_shared_codex_root(paths: &AppPaths) -> Result<()> {
    if same_path(&paths.shared_codex_root, &paths.legacy_shared_codex_root)
        || !paths.legacy_shared_codex_root.exists()
    {
        return Ok(());
    }

    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    let mut entries = SHARED_CODEX_DIR_NAMES
        .iter()
        .map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::Directory,
        })
        .chain(SHARED_CODEX_FILE_NAMES.iter().map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::File,
        }))
        .collect::<Vec<_>>();

    let mut sqlite_entries = BTreeSet::new();
    collect_shared_codex_sqlite_entries(&paths.legacy_shared_codex_root, &mut sqlite_entries)?;
    for name in sqlite_entries {
        entries.push(SharedCodexEntry {
            name,
            kind: SharedCodexEntryKind::File,
        });
    }

    for entry in entries {
        let legacy_path = paths.legacy_shared_codex_root.join(&entry.name);
        let shared_path = paths.shared_codex_root.join(&entry.name);
        migrate_shared_codex_entry(&legacy_path, &shared_path, entry.kind)?;
    }

    Ok(())
}

fn shared_codex_entries(paths: &AppPaths, codex_home: &Path) -> Result<Vec<SharedCodexEntry>> {
    let mut entries = SHARED_CODEX_DIR_NAMES
        .iter()
        .map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::Directory,
        })
        .chain(SHARED_CODEX_FILE_NAMES.iter().map(|name| SharedCodexEntry {
            name: (*name).to_string(),
            kind: SharedCodexEntryKind::File,
        }))
        .collect::<Vec<_>>();

    let mut sqlite_entries = BTreeSet::new();
    let mut scan_roots = vec![paths.shared_codex_root.clone(), codex_home.to_path_buf()];
    scan_roots.sort();
    scan_roots.dedup();

    for root in scan_roots {
        collect_shared_codex_sqlite_entries(&root, &mut sqlite_entries)?;
    }

    for name in sqlite_entries {
        entries.push(SharedCodexEntry {
            name,
            kind: SharedCodexEntryKind::File,
        });
    }

    Ok(entries)
}

fn collect_shared_codex_sqlite_entries(root: &Path, names: &mut BTreeSet<String>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry.with_context(|| format!("failed to read entry in {}", root.display()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if is_shared_codex_sqlite_name(&file_name) {
            names.insert(file_name.into_owned());
        }
    }

    Ok(())
}

fn is_shared_codex_sqlite_name(file_name: &str) -> bool {
    SHARED_CODEX_SQLITE_PREFIXES
        .iter()
        .any(|prefix| file_name.starts_with(prefix))
        && SHARED_CODEX_SQLITE_SUFFIXES
            .iter()
            .any(|suffix| file_name.ends_with(suffix))
}

fn ensure_shared_codex_entry(
    paths: &AppPaths,
    codex_home: &Path,
    entry: &SharedCodexEntry,
) -> Result<()> {
    let local_path = codex_home.join(&entry.name);
    let shared_path = paths.shared_codex_root.join(&entry.name);
    if let Some(parent) = shared_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    migrate_shared_codex_entry(&local_path, &shared_path, entry.kind)?;

    if entry.kind == SharedCodexEntryKind::Directory && !shared_path.exists() {
        create_codex_home_if_missing(&shared_path)?;
    }

    ensure_symlink_to_shared(&local_path, &shared_path, entry.kind)
}

fn migrate_shared_codex_entry(
    local_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    let metadata = match fs::symlink_metadata(local_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", local_path.display()));
        }
    };

    if metadata.file_type().is_symlink() {
        remove_path(local_path)?;
        return Ok(());
    }

    match kind {
        SharedCodexEntryKind::Directory => {
            if !metadata.is_dir() {
                bail!(
                    "expected {} to be a directory for shared Codex state",
                    local_path.display()
                );
            }

            if !shared_path.exists() {
                move_directory(local_path, shared_path)?;
                return Ok(());
            }
            if !shared_path.is_dir() {
                bail!(
                    "expected {} to be a directory for shared Codex state",
                    shared_path.display()
                );
            }

            copy_directory_contents(local_path, shared_path)?;
            fs::remove_dir_all(local_path)
                .with_context(|| format!("failed to remove {}", local_path.display()))?;
        }
        SharedCodexEntryKind::File => {
            if !metadata.is_file() {
                bail!(
                    "expected {} to be a file for shared Codex state",
                    local_path.display()
                );
            }

            if !shared_path.exists() {
                move_file(local_path, shared_path)?;
                return Ok(());
            }
            if !shared_path.is_file() {
                bail!(
                    "expected {} to be a file for shared Codex state",
                    shared_path.display()
                );
            }

            if local_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "history.jsonl")
            {
                append_file_contents(local_path, shared_path)?;
            }

            fs::remove_file(local_path)
                .with_context(|| format!("failed to remove {}", local_path.display()))?;
        }
    }

    Ok(())
}

fn seed_shared_codex_entry(
    legacy_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    let metadata = match fs::symlink_metadata(legacy_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", legacy_path.display()));
        }
    };

    if metadata.file_type().is_symlink() {
        return Ok(());
    }

    if let Some(parent) = shared_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match kind {
        SharedCodexEntryKind::Directory => {
            if !metadata.is_dir() {
                bail!(
                    "expected {} to be a directory for shared Codex state",
                    legacy_path.display()
                );
            }

            create_codex_home_if_missing(shared_path)?;
            copy_directory_contents(legacy_path, shared_path)?;
        }
        SharedCodexEntryKind::File => {
            if !metadata.is_file() {
                bail!(
                    "expected {} to be a file for shared Codex state",
                    legacy_path.display()
                );
            }

            if !shared_path.exists() {
                fs::copy(legacy_path, shared_path).with_context(|| {
                    format!(
                        "failed to copy legacy shared Codex file {} to {}",
                        legacy_path.display(),
                        shared_path.display()
                    )
                })?;
            } else if legacy_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "history.jsonl")
            {
                append_file_contents(legacy_path, shared_path)?;
            }
        }
    }

    Ok(())
}

fn move_directory(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            create_codex_home_if_missing(destination)?;
            copy_directory_contents(source, destination)?;
            fs::remove_dir_all(source)
                .with_context(|| format!("failed to remove {}", source.display()))
        }
    }
}

fn move_file(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(source, destination).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source.display(),
                    destination.display()
                )
            })?;
            fs::remove_file(source)
                .with_context(|| format!("failed to remove {}", source.display()))
        }
    }
}

fn append_file_contents(source: &Path, destination: &Path) -> Result<()> {
    let content =
        fs::read(source).with_context(|| format!("failed to read {}", source.display()))?;
    if content.is_empty() {
        return Ok(());
    }

    use std::io::Write as _;

    let mut destination_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(destination)
        .with_context(|| format!("failed to open {}", destination.display()))?;

    let destination_len = destination_file
        .metadata()
        .with_context(|| format!("failed to inspect {}", destination.display()))?
        .len();
    if destination_len > 0 {
        destination_file
            .write_all(b"\n")
            .with_context(|| format!("failed to append separator to {}", destination.display()))?;
    }

    destination_file.write_all(&content).with_context(|| {
        format!(
            "failed to append {} to {}",
            source.display(),
            destination.display()
        )
    })
}

fn ensure_symlink_to_shared(
    local_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    if local_path.exists() {
        remove_path(local_path)?;
    } else if fs::symlink_metadata(local_path).is_ok() {
        remove_path(local_path)?;
    }

    create_symlink(shared_path, local_path, kind)
}

fn create_symlink(target: &Path, link: &Path, kind: SharedCodexEntryKind) -> Result<()> {
    #[cfg(unix)]
    {
        let _ = kind;
        std::os::unix::fs::symlink(target, link).with_context(|| {
            format!(
                "failed to link shared Codex state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(windows)]
    {
        match kind {
            SharedCodexEntryKind::Directory => std::os::windows::fs::symlink_dir(target, link),
            SharedCodexEntryKind::File => std::os::windows::fs::symlink_file(target, link),
        }
        .with_context(|| {
            format!(
                "failed to link shared Codex state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = kind;
        bail!("shared Codex session links are not supported on this platform");
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        fs::remove_file(path)
            .or_else(|_| fs::remove_dir(path))
            .with_context(|| format!("failed to remove symbolic link {}", path.display()))?;
        return Ok(());
    }

    if metadata.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

pub(crate) fn create_codex_home_if_missing(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o700);
        let _ = fs::set_permissions(path, permissions);
    }
    Ok(())
}

fn dir_is_empty(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    let mut entries =
        fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(entries.next().is_none())
}

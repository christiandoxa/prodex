use super::*;

pub(super) fn ensure_shared_codex_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

pub(super) fn load_shared_codex_entry_metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn ensure_shared_codex_directory(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    if metadata.is_dir() {
        return Ok(());
    }
    bail!(
        "expected {} to be a directory for shared Codex state",
        path.display()
    );
}

fn ensure_shared_codex_file(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    if metadata.is_file() {
        return Ok(());
    }
    bail!(
        "expected {} to be a file for shared Codex state",
        path.display()
    );
}

fn ensure_shared_codex_path_is_directory(path: &Path) -> Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    bail!(
        "expected {} to be a directory for shared Codex state",
        path.display()
    );
}

fn ensure_shared_codex_path_is_file(path: &Path) -> Result<()> {
    if path.is_file() {
        return Ok(());
    }
    bail!(
        "expected {} to be a file for shared Codex state",
        path.display()
    );
}

fn ensure_shared_codex_target_is_directory(path: &Path) -> Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    bail!(
        "expected {} to be a directory for shared Codex state",
        path.display()
    );
}

fn ensure_shared_codex_target_is_file(path: &Path) -> Result<()> {
    if path.is_file() {
        return Ok(());
    }
    bail!(
        "expected {} to be a file for shared Codex state",
        path.display()
    );
}

pub(super) fn copy_shared_codex_file(source: &Path, destination: &Path) -> Result<()> {
    copy_shared_codex_file_replacing_existing(source, destination, "failed to copy")
}

pub(super) fn copy_shared_codex_file_replacing_existing(
    source: &Path,
    destination: &Path,
    context: &str,
) -> Result<()> {
    ensure_shared_codex_parent_dir(destination)?;
    remove_existing_shared_codex_file_destination(destination)?;
    fs::copy(source, destination).with_context(|| {
        format!(
            "{context} {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

pub(super) fn move_directory(source: &Path, destination: &Path) -> Result<()> {
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

pub(super) fn move_file(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_shared_codex_file_replacing_existing(source, destination, "failed to copy")?;
            fs::remove_file(source)
                .with_context(|| format!("failed to remove {}", source.display()))
        }
    }
}

pub(super) fn ensure_symlink_to_shared(
    local_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    if let Some(parent) = local_path.parent() {
        create_codex_home_if_missing(parent)?;
    }
    if local_path.exists() || fs::symlink_metadata(local_path).is_ok() {
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

fn remove_existing_shared_codex_file_destination(path: &Path) -> Result<()> {
    let Some(metadata) = load_shared_codex_entry_metadata(path)? else {
        return Ok(());
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        bail!(
            "expected {} to be a file for shared Codex state",
            path.display()
        );
    }
    make_shared_codex_path_writable_for_removal(path, &metadata)?;
    fs::remove_file(path)
        .or_else(|_| fs::remove_dir(path))
        .with_context(|| format!("failed to remove {}", path.display()))?;
    Ok(())
}

fn make_shared_codex_path_writable_for_removal(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    if metadata.file_type().is_symlink() || !metadata.permissions().readonly() {
        return Ok(());
    }

    let mut permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(windows)]
    {
        permissions.set_readonly(false);
    }
    #[cfg(not(any(unix, windows)))]
    {
        permissions.set_readonly(false);
    }
    fs::set_permissions(path, permissions)
        .with_context(|| format!("failed to make {} writable", path.display()))?;
    Ok(())
}

pub(super) fn remove_path(path: &Path) -> Result<()> {
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
        make_shared_codex_path_writable_for_removal(path, &metadata)?;
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

pub(super) fn dir_is_empty(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    let mut entries =
        fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(entries.next().is_none())
}

pub(super) fn ensure_shared_codex_directory_public(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<()> {
    ensure_shared_codex_directory(path, metadata)
}

pub(super) fn ensure_shared_codex_file_public(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    ensure_shared_codex_file(path, metadata)
}

pub(super) fn ensure_shared_codex_path_is_directory_public(path: &Path) -> Result<()> {
    ensure_shared_codex_path_is_directory(path)
}

pub(super) fn ensure_shared_codex_path_is_file_public(path: &Path) -> Result<()> {
    ensure_shared_codex_path_is_file(path)
}

pub(super) fn ensure_shared_codex_target_is_directory_public(path: &Path) -> Result<()> {
    ensure_shared_codex_target_is_directory(path)
}

pub(super) fn ensure_shared_codex_target_is_file_public(path: &Path) -> Result<()> {
    ensure_shared_codex_target_is_file(path)
}

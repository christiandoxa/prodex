use super::*;

pub(super) fn migrate_shared_codex_entry(
    local_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    let Some(metadata) = load_shared_codex_entry_metadata(local_path)? else {
        return Ok(());
    };

    if metadata.file_type().is_symlink() {
        migrate_shared_codex_symlink_target(local_path, shared_path, kind)?;
        remove_path(local_path)?;
        return Ok(());
    }

    match kind {
        SharedCodexEntryKind::Directory => {
            migrate_shared_codex_directory_entry(local_path, shared_path, &metadata)
        }
        SharedCodexEntryKind::File => {
            migrate_shared_codex_file_entry(local_path, shared_path, &metadata)
        }
    }
}

fn migrate_shared_codex_directory_entry(
    local_path: &Path,
    shared_path: &Path,
    metadata: &fs::Metadata,
) -> Result<()> {
    ensure_shared_codex_directory_public(local_path, metadata)?;
    if !shared_path.exists() {
        move_directory(local_path, shared_path)?;
        return Ok(());
    }

    ensure_shared_codex_path_is_directory_public(shared_path)?;
    copy_directory_contents(local_path, shared_path)?;
    fs::remove_dir_all(local_path)
        .with_context(|| format!("failed to remove {}", local_path.display()))
}

fn migrate_shared_codex_file_entry(
    local_path: &Path,
    shared_path: &Path,
    metadata: &fs::Metadata,
) -> Result<()> {
    ensure_shared_codex_file_public(local_path, metadata)?;
    if !shared_path.exists() {
        move_file(local_path, shared_path)?;
        return Ok(());
    }

    ensure_shared_codex_path_is_file_public(shared_path)?;
    if is_history_jsonl(local_path) {
        merge_history_files(local_path, shared_path)?;
    }
    fs::remove_file(local_path)
        .with_context(|| format!("failed to remove {}", local_path.display()))
}

fn migrate_shared_codex_symlink_target(
    local_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    let target_path = shared_codex_symlink_target_path(local_path)?;

    if same_path(&target_path, shared_path) || !target_path.exists() {
        return Ok(());
    }

    ensure_shared_codex_parent_dir(shared_path)?;

    match kind {
        SharedCodexEntryKind::Directory => {
            migrate_shared_codex_symlink_directory_entry(&target_path, shared_path)
        }
        SharedCodexEntryKind::File => {
            migrate_shared_codex_symlink_file_entry(local_path, &target_path, shared_path)
        }
    }
}

fn shared_codex_symlink_target_path(local_path: &Path) -> Result<PathBuf> {
    let target = fs::read_link(local_path)
        .with_context(|| format!("failed to read symlink {}", local_path.display()))?;
    Ok(if target.is_absolute() {
        target
    } else {
        local_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(target)
    })
}

fn migrate_shared_codex_symlink_directory_entry(
    target_path: &Path,
    shared_path: &Path,
) -> Result<()> {
    ensure_shared_codex_target_is_directory_public(target_path)?;
    if !shared_path.exists() {
        create_codex_home_if_missing(shared_path)?;
    } else {
        ensure_shared_codex_path_is_directory_public(shared_path)?;
    }
    copy_directory_contents(target_path, shared_path)
}

fn migrate_shared_codex_symlink_file_entry(
    local_path: &Path,
    target_path: &Path,
    shared_path: &Path,
) -> Result<()> {
    ensure_shared_codex_target_is_file_public(target_path)?;
    if !shared_path.exists() {
        copy_shared_codex_file(target_path, shared_path)?;
        return Ok(());
    }

    ensure_shared_codex_path_is_file_public(shared_path)?;
    if is_history_jsonl(local_path) {
        merge_history_files(target_path, shared_path)?;
    }
    Ok(())
}

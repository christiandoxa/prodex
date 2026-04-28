use super::*;

pub(crate) fn prepare_managed_codex_home(paths: &AppPaths, codex_home: &Path) -> Result<()> {
    create_codex_home_if_missing(codex_home)?;
    migrate_legacy_shared_codex_roots(paths)?;
    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    for entry in shared_codex_entries(paths, codex_home)? {
        ensure_shared_codex_entry(paths, codex_home, &entry)?;
    }

    Ok(())
}

fn migrate_legacy_shared_codex_roots(paths: &AppPaths) -> Result<()> {
    migrate_legacy_shared_codex_root(paths, &paths.legacy_shared_codex_root)?;
    if env::var_os("PRODEX_SHARED_CODEX_HOME").is_none() {
        let previous_default_root = prodex_previous_default_shared_codex_root(&paths.root);
        migrate_legacy_shared_codex_root(paths, &previous_default_root)?;
    }
    Ok(())
}

fn migrate_legacy_shared_codex_root(paths: &AppPaths, legacy_root: &Path) -> Result<()> {
    if same_path(&paths.shared_codex_root, legacy_root) || !legacy_root.exists() {
        return Ok(());
    }

    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    for entry in shared_codex_entries_for_roots([legacy_root])? {
        let legacy_path = legacy_root.join(&entry.name);
        let shared_path = paths.shared_codex_root.join(&entry.name);
        migrate_shared_codex_entry(&legacy_path, &shared_path, entry.kind)?;
    }

    Ok(())
}

fn shared_codex_entries(paths: &AppPaths, codex_home: &Path) -> Result<Vec<SharedCodexEntry>> {
    let mut scan_roots = vec![paths.shared_codex_root.clone(), codex_home.to_path_buf()];
    scan_roots.sort();
    scan_roots.dedup();
    shared_codex_entries_for_roots(scan_roots.iter().map(PathBuf::as_path))
}

fn shared_codex_entries_for_roots<'a>(
    scan_roots: impl IntoIterator<Item = &'a Path>,
) -> Result<Vec<SharedCodexEntry>> {
    let mut entries = shared_codex_manifest_entries();
    let mut sqlite_entries = BTreeSet::new();

    for root in scan_roots {
        collect_shared_codex_sqlite_entries(root, &mut sqlite_entries)?;
    }

    entries.extend(sqlite_entries.into_iter().map(SharedCodexEntry::file));
    Ok(entries)
}

fn shared_codex_manifest_entries() -> Vec<SharedCodexEntry> {
    SHARED_CODEX_DIR_NAMES
        .iter()
        .map(|name| SharedCodexEntry::directory(name))
        .chain(
            SHARED_CODEX_FILE_NAMES
                .iter()
                .map(|name| SharedCodexEntry::file(*name)),
        )
        .collect()
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
    ensure_shared_codex_parent_dir(&shared_path)?;

    migrate_shared_codex_entry(&local_path, &shared_path, entry.kind)?;

    if entry.kind == SharedCodexEntryKind::Directory && !shared_path.exists() {
        create_codex_home_if_missing(&shared_path)?;
    }

    ensure_symlink_to_shared(&local_path, &shared_path, entry.kind)
}

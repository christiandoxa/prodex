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

impl SharedCodexEntry {
    fn directory(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: SharedCodexEntryKind::Directory,
        }
    }

    fn file(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: SharedCodexEntryKind::File,
        }
    }
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
        copy_directory_entry(&source_path, &destination_path, file_type)?;
    }

    Ok(())
}

fn copy_directory_entry(
    source_path: &Path,
    destination_path: &Path,
    file_type: fs::FileType,
) -> Result<()> {
    if file_type.is_dir() {
        create_codex_home_if_missing(destination_path)?;
        return copy_directory_contents(source_path, destination_path);
    }

    if file_type.is_file() {
        fs::copy(source_path, destination_path).with_context(|| {
            format!(
                "failed to copy {} to {}",
                source_path.display(),
                destination_path.display()
            )
        })?;
        return Ok(());
    }

    if file_type.is_symlink() {
        return recreate_symlink(source_path, destination_path);
    }

    Ok(())
}

fn recreate_symlink(source_path: &Path, destination_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let target = fs::read_link(source_path)
            .with_context(|| format!("failed to read symlink {}", source_path.display()))?;
        std::os::unix::fs::symlink(target, destination_path)
            .with_context(|| format!("failed to recreate symlink {}", destination_path.display()))
    }

    #[cfg(not(unix))]
    {
        let _ = (source_path, destination_path);
        bail!("symlinks are not supported on this platform");
    }
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

    for entry in shared_codex_entries_for_roots([legacy_root.as_path()])? {
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

    for entry in shared_codex_entries_for_roots([paths.legacy_shared_codex_root.as_path()])? {
        let legacy_path = paths.legacy_shared_codex_root.join(&entry.name);
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

fn ensure_shared_codex_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

fn load_shared_codex_entry_metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn migrate_shared_codex_entry(
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
    ensure_shared_codex_directory(local_path, metadata)?;
    if !shared_path.exists() {
        move_directory(local_path, shared_path)?;
        return Ok(());
    }

    ensure_shared_codex_path_is_directory(shared_path)?;
    copy_directory_contents(local_path, shared_path)?;
    fs::remove_dir_all(local_path)
        .with_context(|| format!("failed to remove {}", local_path.display()))
}

fn migrate_shared_codex_file_entry(
    local_path: &Path,
    shared_path: &Path,
    metadata: &fs::Metadata,
) -> Result<()> {
    ensure_shared_codex_file(local_path, metadata)?;
    if !shared_path.exists() {
        move_file(local_path, shared_path)?;
        return Ok(());
    }

    ensure_shared_codex_path_is_file(shared_path)?;
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
    ensure_shared_codex_target_is_directory(target_path)?;
    if !shared_path.exists() {
        create_codex_home_if_missing(shared_path)?;
    } else {
        ensure_shared_codex_path_is_directory(shared_path)?;
    }
    copy_directory_contents(target_path, shared_path)
}

fn migrate_shared_codex_symlink_file_entry(
    local_path: &Path,
    target_path: &Path,
    shared_path: &Path,
) -> Result<()> {
    ensure_shared_codex_target_is_file(target_path)?;
    if !shared_path.exists() {
        copy_shared_codex_file(target_path, shared_path)?;
        return Ok(());
    }

    ensure_shared_codex_path_is_file(shared_path)?;
    if is_history_jsonl(local_path) {
        merge_history_files(target_path, shared_path)?;
    }
    Ok(())
}

fn seed_shared_codex_entry(
    legacy_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    let Some(metadata) = load_shared_codex_entry_metadata(legacy_path)? else {
        return Ok(());
    };

    if metadata.file_type().is_symlink() {
        return Ok(());
    }

    ensure_shared_codex_parent_dir(shared_path)?;

    match kind {
        SharedCodexEntryKind::Directory => {
            seed_shared_codex_directory_entry(legacy_path, shared_path, &metadata)
        }
        SharedCodexEntryKind::File => {
            seed_shared_codex_file_entry(legacy_path, shared_path, &metadata)
        }
    }
}

fn seed_shared_codex_directory_entry(
    legacy_path: &Path,
    shared_path: &Path,
    metadata: &fs::Metadata,
) -> Result<()> {
    ensure_shared_codex_directory(legacy_path, metadata)?;
    if shared_path.exists() {
        ensure_shared_codex_path_is_directory(shared_path)?;

        // Legacy seeding is a one-time bootstrap. Recopying populated
        // session trees from ~/.codex on every `prodex run` adds large
        // disk I/O directly to the startup hot path.
        if !dir_is_empty(shared_path)? {
            return Ok(());
        }
    } else {
        create_codex_home_if_missing(shared_path)?;
    }

    copy_directory_contents(legacy_path, shared_path)
}

fn seed_shared_codex_file_entry(
    legacy_path: &Path,
    shared_path: &Path,
    metadata: &fs::Metadata,
) -> Result<()> {
    ensure_shared_codex_file(legacy_path, metadata)?;
    if !shared_path.exists() {
        return copy_legacy_shared_codex_file(legacy_path, shared_path);
    }

    ensure_shared_codex_path_is_file(shared_path)?;
    if is_history_jsonl(legacy_path) {
        // Legacy default CODEX_HOME seeding should stay one-shot so
        // startup does not reread and rewrite large history files on
        // every `prodex run`.
        return Ok(());
    }
    Ok(())
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

fn copy_shared_codex_file(source: &Path, destination: &Path) -> Result<()> {
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn copy_legacy_shared_codex_file(source: &Path, destination: &Path) -> Result<()> {
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy legacy shared Codex file {} to {}",
            source.display(),
            destination.display()
        )
    })?;
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

fn is_history_jsonl(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "history.jsonl")
}

fn merge_history_files(source: &Path, destination: &Path) -> Result<()> {
    #[derive(Debug)]
    struct HistoryLine {
        ts: Option<i64>,
        line: String,
        order: usize,
    }

    fn load_history_lines(
        path: &Path,
        merged: &mut Vec<HistoryLine>,
        seen: &mut BTreeSet<String>,
    ) -> Result<()> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        for raw_line in content.lines() {
            let line = raw_line.trim_end_matches('\r');
            if line.is_empty() || !seen.insert(line.to_string()) {
                continue;
            }

            let ts = serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|value| value.get("ts").and_then(serde_json::Value::as_i64));
            merged.push(HistoryLine {
                ts,
                line: line.to_string(),
                order: merged.len(),
            });
        }

        Ok(())
    }

    let mut merged = Vec::new();
    let mut seen = BTreeSet::new();

    if destination.exists() {
        load_history_lines(destination, &mut merged, &mut seen)?;
    }
    load_history_lines(source, &mut merged, &mut seen)?;

    merged.sort_by(|left, right| match (left.ts, right.ts) {
        (Some(left_ts), Some(right_ts)) => {
            left_ts.cmp(&right_ts).then(left.order.cmp(&right.order))
        }
        _ => left.order.cmp(&right.order),
    });

    let mut content = String::new();
    for (index, entry) in merged.iter().enumerate() {
        if index > 0 {
            content.push('\n');
        }
        content.push_str(&entry.line);
    }

    fs::write(destination, content)
        .with_context(|| format!("failed to write merged history {}", destination.display()))
}

fn ensure_symlink_to_shared(
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

use super::*;
use crate::image_attachments::{
    codex_session_image_attachments_are_stable, persist_codex_session_file_image_attachments,
    rewrite_codex_persisted_attachment_paths,
};
use chrono::{DateTime, Utc};
use filetime::FileTime;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::UNIX_EPOCH;

const SESSION_TIMESTAMP_PREFIX: &str = "\"timestamp\":\"";
const SESSION_MAINTENANCE_CACHE_VERSION: u8 = 3;
const SESSION_MAINTENANCE_CACHE_FILE: &str = "shared-codex-session-maintenance-v1.json";

#[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
struct SessionMaintenanceCache {
    version: u8,
    files: BTreeMap<String, SessionFileFingerprint>,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
struct SessionFileFingerprint {
    len: u64,
    modified_secs: u64,
    modified_nanos: u32,
    changed_secs: i64,
    changed_nanos: i64,
    identity: u64,
}

pub fn prepare_managed_codex_home(paths: &AppPaths, codex_home: &Path) -> Result<()> {
    prepare_managed_codex_home_internal(paths, codex_home, true)
}

pub fn prepare_managed_codex_home_for_runtime_launch(
    paths: &AppPaths,
    codex_home: &Path,
) -> Result<()> {
    prepare_managed_codex_home_internal(paths, codex_home, false)
}

fn prepare_managed_codex_home_internal(
    paths: &AppPaths,
    codex_home: &Path,
    maintain_sessions: bool,
) -> Result<()> {
    ensure_managed_profiles_root(paths)?;
    ensure_managed_codex_home_is_not_symlink(codex_home)?;
    create_codex_home_if_missing(codex_home)?;
    migrate_legacy_shared_codex_roots(paths)?;
    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    for entry in shared_codex_entries(paths, codex_home)? {
        ensure_shared_codex_entry(paths, codex_home, &entry)?;
    }
    if maintain_sessions {
        maintain_managed_codex_sessions(paths)?;
    }

    Ok(())
}

fn ensure_managed_codex_home_is_not_symlink(codex_home: &Path) -> Result<()> {
    let Some(metadata) = load_shared_codex_entry_metadata(codex_home)? else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() {
        bail!(
            "managed Codex home {} must not be a symbolic link",
            codex_home.display()
        );
    }
    if !metadata.is_dir() {
        bail!(
            "managed Codex home {} must be a directory",
            codex_home.display()
        );
    }
    Ok(())
}

pub fn maintain_managed_codex_sessions(paths: &AppPaths) -> Result<()> {
    let Some(_maintenance_lock) = try_lock_codex_session_maintenance(&paths.shared_codex_root)?
    else {
        return Ok(());
    };
    let cache_path = paths.root.join(SESSION_MAINTENANCE_CACHE_FILE);
    let previous = load_session_maintenance_cache(&cache_path);
    let mut next = SessionMaintenanceCache {
        version: SESSION_MAINTENANCE_CACHE_VERSION,
        files: BTreeMap::new(),
    };

    maintain_codex_sessions_in_dir(
        &paths.shared_codex_root,
        &paths.shared_codex_root.join("sessions"),
        &previous,
        &mut next,
    )?;
    maintain_codex_sessions_in_dir(
        &paths.shared_codex_root,
        &paths.shared_codex_root.join("archived_sessions"),
        &previous,
        &mut next,
    )?;
    persist_codex_goal_attachment_paths(&paths.shared_codex_root)?;

    if next != previous {
        // The cache only avoids repeat work. A read-only or contended cache must never prevent
        // attachment persistence, session ordering repair, or launch.
        let _ = save_session_maintenance_cache(&cache_path, &next);
    }
    Ok(())
}

fn maintain_codex_sessions_in_dir(
    codex_home: &Path,
    sessions_dir: &Path,
    previous: &SessionMaintenanceCache,
    next: &mut SessionMaintenanceCache,
) -> Result<()> {
    if !sessions_dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(sessions_dir)
        .with_context(|| format!("failed to read {}", sessions_dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", sessions_dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read metadata for {}", path.display()))?;
        if file_type.is_dir() {
            maintain_codex_sessions_in_dir(codex_home, &path, previous, next)?;
            continue;
        }
        if !file_type.is_file()
            || path
                .extension()
                .is_none_or(|extension| extension != "jsonl")
        {
            continue;
        }

        let key = path
            .strip_prefix(codex_home)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let before = session_file_fingerprint(&path)?;
        if previous.version == SESSION_MAINTENANCE_CACHE_VERSION
            && previous.files.get(&key) == Some(&before)
        {
            next.files.insert(key, before);
            continue;
        }

        let Some(contents) = persist_codex_session_file_image_attachments(codex_home, &path)?
        else {
            continue;
        };
        restore_codex_session_file_modified_time(&path, &contents)?;
        if codex_session_image_attachments_are_stable(codex_home, &contents) {
            next.files.insert(key, session_file_fingerprint(&path)?);
        }
    }

    Ok(())
}

fn persist_codex_goal_attachment_paths(codex_home: &Path) -> Result<()> {
    if !codex_home.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(codex_home)
        .with_context(|| format!("failed to read {}", codex_home.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", codex_home.display()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with("goals_") && file_name.ends_with(".sqlite") {
            persist_codex_goal_attachment_paths_in_db(codex_home, &entry.path())?;
        }
    }
    Ok(())
}

fn persist_codex_goal_attachment_paths_in_db(codex_home: &Path, db_path: &Path) -> Result<()> {
    if !path_looks_like_sqlite_db(db_path) {
        return Ok(());
    }
    let Ok(conn) = Connection::open(db_path) else {
        return Ok(());
    };
    let has_thread_goals = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'thread_goals'",
            [],
            |_| Ok(()),
        )
        .optional()
        .ok()
        .flatten()
        .is_some();
    if !has_thread_goals {
        return Ok(());
    }

    let mut stmt = match conn.prepare("SELECT thread_id, objective FROM thread_goals") {
        Ok(stmt) => stmt,
        Err(_) => return Ok(()),
    };
    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(rows) => rows,
        Err(_) => return Ok(()),
    };
    let mut updates = Vec::new();
    for row in rows {
        let Ok((thread_id, objective)) = row else {
            continue;
        };
        let rewritten = rewrite_codex_persisted_attachment_paths(codex_home, &objective)?;
        if rewritten != objective {
            updates.push((thread_id, rewritten));
        }
    }
    drop(stmt);

    for (thread_id, objective) in updates {
        conn.execute(
            "UPDATE thread_goals SET objective = ? WHERE thread_id = ?",
            params![objective, thread_id],
        )
        .with_context(|| format!("failed to update goal attachments in {}", db_path.display()))?;
    }
    Ok(())
}

fn path_looks_like_sqlite_db(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() {
        return false;
    }
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut header = [0; 16];
    file.read_exact(&mut header).is_ok() && &header == b"SQLite format 3\0"
}

fn session_file_fingerprint(path: &Path) -> Result<SessionFileFingerprint> {
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to inspect {}", path.display()))?;
    let modified = metadata
        .modified()
        .with_context(|| format!("failed to read modified time for {}", path.display()))?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    #[cfg(unix)]
    let (changed_secs, changed_nanos, identity) = {
        use std::os::unix::fs::MetadataExt;
        (metadata.ctime(), metadata.ctime_nsec(), metadata.ino())
    };
    #[cfg(not(unix))]
    let (changed_secs, changed_nanos, identity) = (
        i64::try_from(modified.as_secs()).unwrap_or(i64::MAX),
        i64::from(modified.subsec_nanos()),
        0,
    );
    Ok(SessionFileFingerprint {
        len: metadata.len(),
        modified_secs: modified.as_secs(),
        modified_nanos: modified.subsec_nanos(),
        changed_secs,
        changed_nanos,
        identity,
    })
}

fn load_session_maintenance_cache(path: &Path) -> SessionMaintenanceCache {
    fs::read(path)
        .ok()
        .and_then(|contents| serde_json::from_slice(&contents).ok())
        .filter(|cache: &SessionMaintenanceCache| {
            cache.version == SESSION_MAINTENANCE_CACHE_VERSION
        })
        .unwrap_or_default()
}

fn save_session_maintenance_cache(path: &Path, cache: &SessionMaintenanceCache) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temp_path = path.with_extension(format!("{}.tmp", std::process::id()));
    let contents =
        serde_json::to_vec(cache).context("failed to serialize session maintenance cache")?;
    fs::write(&temp_path, contents)
        .with_context(|| format!("failed to write {}", temp_path.display()))?;
    match fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(first_err) if path.exists() => {
            fs::remove_file(path).with_context(|| {
                format!(
                    "failed to remove session maintenance cache {} after rename failed: {first_err}",
                    path.display()
                )
            })?;
            fs::rename(&temp_path, path).with_context(|| {
                format!(
                    "failed to replace session maintenance cache {} after initial rename failed: {first_err}",
                    path.display()
                )
            })
        }
        Err(err) => Err(err).with_context(|| {
            format!(
                "failed to replace session maintenance cache {}",
                path.display()
            )
        }),
    }
}

fn restore_codex_session_file_modified_time(session_file: &Path, contents: &str) -> Result<()> {
    let Some(timestamp) = last_session_event_timestamp(contents) else {
        return Ok(());
    };
    let metadata = fs::metadata(session_file)
        .with_context(|| format!("failed to inspect {}", session_file.display()))?;
    let modified_time =
        FileTime::from_unix_time(timestamp.timestamp(), timestamp.timestamp_subsec_nanos());
    filetime::set_file_times(
        session_file,
        FileTime::from_last_access_time(&metadata),
        modified_time,
    )
    .with_context(|| {
        format!(
            "failed to restore session modified time for {}",
            session_file.display()
        )
    })
}

fn last_session_event_timestamp(contents: &str) -> Option<DateTime<Utc>> {
    contents
        .lines()
        .filter_map(session_line_timestamp)
        .next_back()
}

fn session_line_timestamp(line: &str) -> Option<DateTime<Utc>> {
    let start = line.find(SESSION_TIMESTAMP_PREFIX)? + SESSION_TIMESTAMP_PREFIX.len();
    let end = line[start..].find('"')? + start;
    DateTime::parse_from_rfc3339(&line[start..end])
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
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
    let mut dynamic_file_entries = BTreeSet::new();

    for root in scan_roots {
        collect_dynamic_shared_codex_file_entries(root, &mut dynamic_file_entries)?;
    }

    entries.extend(dynamic_file_entries.into_iter().map(SharedCodexEntry::file));
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

fn collect_dynamic_shared_codex_file_entries(
    root: &Path,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry.with_context(|| format!("failed to read entry in {}", root.display()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if is_shared_codex_sqlite_name(&file_name)
            || is_shared_codex_profile_v2_config_name(&file_name)
        {
            names.insert(file_name.into_owned());
        }
    }

    Ok(())
}

fn is_shared_codex_profile_v2_config_name(file_name: &str) -> bool {
    let Some(profile_name) = file_name.strip_suffix(SHARED_CODEX_PROFILE_V2_CONFIG_SUFFIX) else {
        return false;
    };
    !profile_name.is_empty()
        && profile_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
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

#[cfg(test)]
#[path = "../tests/src/prepare.rs"]
mod tests;

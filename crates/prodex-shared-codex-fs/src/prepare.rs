use self::goal_attachments::{
    persist_codex_goal_attachment_path_for_thread, persist_codex_goal_attachment_paths,
    session_thread_id,
};
use super::*;
use crate::image_attachments::{
    codex_session_image_attachments_are_stable, codex_session_persisted_attachment_paths,
    is_codex_session_rollout_file, persist_codex_session_file_image_attachments,
};
use chrono::{DateTime, Datelike, Utc};
use filetime::FileTime;
use prodex_session_store::repair_codex_session_metadata_prefix;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use std::time::UNIX_EPOCH;

#[path = "prepare/goal_attachments.rs"]
mod goal_attachments;

const SESSION_TIMESTAMP_PREFIX: &str = "\"timestamp\":\"";
const SESSION_MAINTENANCE_CACHE_VERSION: u8 = 4;
const SESSION_MAINTENANCE_CACHE_FILE: &str = "shared-codex-session-maintenance-v1.json";
const RECENT_SESSION_MAINTENANCE_CACHE_VERSION: u8 = 2;
const RECENT_SESSION_MAINTENANCE_CACHE_FILE: &str = "shared-codex-session-recent-v1.json";

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

#[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
struct RecentSessionMaintenanceCache {
    version: u8,
    key: String,
    fingerprint: Option<SessionFileFingerprint>,
    attachment_paths: Vec<String>,
    thread_id: Option<String>,
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
    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;
    #[cfg(windows)]
    ensure_windows_shared_codex_symlink_support(&paths.shared_codex_root, codex_home)?;
    migrate_legacy_shared_codex_roots(paths)?;

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

/// Maintains only the most recently modified active rollout from today or yesterday.
///
/// Normal child exit must not revisit historical active or archived sessions. The date-bound
/// lookup keeps attachment and metadata repair available for a fresh rollout or a recent resume;
/// explicit full maintenance remains available through `maintain_managed_codex_sessions`.
pub fn maintain_recent_managed_codex_sessions(paths: &AppPaths) -> Result<Option<PathBuf>> {
    let started = Instant::now();
    let Some(_maintenance_lock) = try_lock_codex_session_maintenance(&paths.shared_codex_root)?
    else {
        return Ok(None);
    };

    let Some(session_file) = find_recent_session_file(&paths.shared_codex_root) else {
        return Ok(None);
    };
    let (session_files_opened, session_bytes_read) =
        maintain_one_managed_codex_session_file(paths, &session_file)?;
    let _ =
        prodex_session_store::repair_state_db_rollout_path(&paths.shared_codex_root, &session_file);
    emit_recent_session_maintenance_timing(started, session_files_opened, session_bytes_read);
    Ok(Some(session_file))
}

/// Maintains one rollout whose path was already identified by a resume repair.
pub fn maintain_managed_codex_session_file(paths: &AppPaths, session_file: &Path) -> Result<()> {
    let Some(_maintenance_lock) = try_lock_codex_session_maintenance(&paths.shared_codex_root)?
    else {
        return Ok(());
    };
    let started = Instant::now();
    let (session_files_opened, session_bytes_read) =
        maintain_one_managed_codex_session_file(paths, session_file)?;
    let _ =
        prodex_session_store::repair_state_db_rollout_path(&paths.shared_codex_root, session_file);
    emit_recent_session_maintenance_timing(started, session_files_opened, session_bytes_read);
    Ok(())
}

fn maintain_one_managed_codex_session_file(
    paths: &AppPaths,
    session_file: &Path,
) -> Result<(u64, u64)> {
    let Ok(relative_path) = session_file.strip_prefix(&paths.shared_codex_root) else {
        return Ok((0, 0));
    };
    let key = relative_path.to_string_lossy().into_owned();
    let fingerprint = session_file_fingerprint(session_file)?;
    let recent_cache_path = paths.root.join(RECENT_SESSION_MAINTENANCE_CACHE_FILE);
    let previous_recent = load_recent_session_maintenance_cache(&recent_cache_path);
    let recent_cache_hit = previous_recent.version == RECENT_SESSION_MAINTENANCE_CACHE_VERSION
        && previous_recent.key == key
        && previous_recent.fingerprint == Some(fingerprint)
        && previous_recent
            .attachment_paths
            .iter()
            .all(|path| stable_attachment_path_exists(Path::new(path)));
    if recent_cache_hit {
        if let Some(thread_id) = previous_recent.thread_id.as_deref() {
            persist_codex_goal_attachment_path_for_thread(&paths.shared_codex_root, thread_id)?;
        }
        return Ok((0, 0));
    }

    let mut session_files_opened = 1_u64;
    let mut session_bytes_read = 0_u64;
    let Some(mut contents) =
        persist_codex_session_file_image_attachments(&paths.shared_codex_root, session_file)?
    else {
        return Ok((0, 0));
    };
    session_bytes_read = session_bytes_read.saturating_add(contents.len() as u64);
    if repair_codex_session_metadata_prefix(session_file, &contents)? {
        session_files_opened += 1;
        let Some(repaired_contents) =
            persist_codex_session_file_image_attachments(&paths.shared_codex_root, session_file)?
        else {
            return Ok((session_files_opened, session_bytes_read));
        };
        session_bytes_read = session_bytes_read.saturating_add(repaired_contents.len() as u64);
        contents = repaired_contents;
    }
    restore_codex_session_file_modified_time(session_file, &contents)?;
    if let Some(thread_id) = session_thread_id(&contents) {
        persist_codex_goal_attachment_path_for_thread(&paths.shared_codex_root, &thread_id)?;
    }
    let stable = codex_session_image_attachments_are_stable(&paths.shared_codex_root, &contents);

    if stable {
        save_recent_session_maintenance_cache(
            &recent_cache_path,
            &RecentSessionMaintenanceCache {
                version: RECENT_SESSION_MAINTENANCE_CACHE_VERSION,
                key,
                fingerprint: Some(session_file_fingerprint(session_file)?),
                attachment_paths: codex_session_persisted_attachment_paths(&contents)
                    .into_iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect(),
                thread_id: session_thread_id(&contents),
            },
        );
    }
    Ok((session_files_opened, session_bytes_read))
}

fn emit_recent_session_maintenance_timing(
    started: Instant,
    session_files_opened: u64,
    session_bytes_read: u64,
) {
    if std::env::var_os("PRODEX_RUNTIME_TIMINGS").is_some() {
        eprintln!(
            "prodex_runtime_timing stage=shutdown.session_maintenance_ms duration_ms={} sessions_walked=1 archived_sessions_walked=0 session_files_opened={session_files_opened} attachment_files_processed={session_files_opened} session_bytes_read={session_bytes_read}",
            started.elapsed().as_secs_f64() * 1000.0,
        );
        eprintln!(
            "prodex_runtime_timing stage=shutdown.attachment_maintenance_ms duration_ms={} session_files_opened={session_files_opened} session_bytes_read={session_bytes_read}",
            started.elapsed().as_secs_f64() * 1000.0,
        );
    }
}

fn find_recent_session_file(codex_home: &Path) -> Option<PathBuf> {
    // ponytail: scan only the two current date buckets; replace with a Codex-emitted session path
    // when an exact O(1) fresh-session handoff is available.
    let today = Utc::now().date_naive();
    [Some(today), today.pred_opt()]
        .into_iter()
        .flatten()
        .filter_map(|date| {
            let date_dir = codex_home.join("sessions").join(format!(
                "{}/{:02}/{:02}",
                date.year(),
                date.month(),
                date.day()
            ));
            find_newest_session_file_in_dir(&date_dir)
        })
        .max_by_key(|(_, modified)| *modified)
        .map(|(path, _)| path)
}

fn find_newest_session_file_in_dir(dir: &Path) -> Option<(PathBuf, std::time::SystemTime)> {
    let mut newest = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if let Some(candidate) = find_newest_session_file_in_dir(&path)
                && newest
                    .as_ref()
                    .is_none_or(|(_, modified)| candidate.1 > *modified)
            {
                newest = Some(candidate);
            }
        } else if file_type.is_file()
            && is_codex_session_rollout_file(&path)
            && let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified())
            && newest
                .as_ref()
                .is_none_or(|(_, current)| modified > *current)
        {
            newest = Some((path, modified));
        }
    }
    newest
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
        if !file_type.is_file() || !is_codex_session_rollout_file(&path) {
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

        let Some(mut contents) = persist_codex_session_file_image_attachments(codex_home, &path)?
        else {
            continue;
        };
        if repair_codex_session_metadata_prefix(&path, &contents)? {
            let Some(repaired_contents) =
                persist_codex_session_file_image_attachments(codex_home, &path)?
            else {
                continue;
            };
            contents = repaired_contents;
        }
        restore_codex_session_file_modified_time(&path, &contents)?;
        if codex_session_image_attachments_are_stable(codex_home, &contents) {
            next.files.insert(key, session_file_fingerprint(&path)?);
        }
    }

    Ok(())
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

fn load_recent_session_maintenance_cache(path: &Path) -> RecentSessionMaintenanceCache {
    fs::read(path)
        .ok()
        .and_then(|contents| serde_json::from_slice(&contents).ok())
        .unwrap_or_default()
}

fn stable_attachment_path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
}

fn save_recent_session_maintenance_cache(path: &Path, cache: &RecentSessionMaintenanceCache) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(contents) = serde_json::to_vec(cache) {
        let _ = fs::write(path, contents);
    }
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

#[cfg(test)]
#[path = "../tests/src/recent_maintenance.rs"]
mod recent_maintenance_tests;

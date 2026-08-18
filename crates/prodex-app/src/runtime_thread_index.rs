use crate::{AppPaths, ChildProcessPlan};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const THREAD_INDEX_PAGE_LIMIT: u64 = 100;
const THREAD_INDEX_TIMEOUT: Duration = Duration::from_secs(60);
const TARGETED_THREAD_INDEX_TIMEOUT: Duration = Duration::from_secs(3);
const THREAD_INDEX_CLEANUP_TIMEOUT: Duration = Duration::from_secs(1);
const THREAD_INDEX_DIRTY_FILE: &str = "thread-index-dirty.json";
const THREAD_INDEX_DIRTY_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Copy)]
enum ThreadIndexRepairScope {
    Full,
    Latest,
}

#[derive(Debug, Deserialize, Serialize)]
struct ThreadIndexDirtyMarker {
    schema_version: u8,
    rollout_path: String,
}

/// Runs Codex's own scan-and-repair listing against the exact child home and environment.
///
/// `useStateDbOnly` stays false deliberately: Codex owns the SQLite schema and its normal
/// listing path is the compatibility layer that repairs rollout/index divergence.
pub(crate) fn reconcile_codex_thread_index(
    codex_binary: &OsStr,
    child: &ChildProcessPlan,
) -> Result<()> {
    reconcile_codex_thread_index_with_scope(
        codex_binary,
        child,
        ThreadIndexRepairScope::Full,
        THREAD_INDEX_TIMEOUT,
    )
}

/// Runs one bounded active-session scan to repair a missing latest SQLite row.
pub(crate) fn reconcile_latest_codex_thread_index(
    codex_binary: &OsStr,
    child: &ChildProcessPlan,
) -> Result<()> {
    reconcile_codex_thread_index_with_scope(
        codex_binary,
        child,
        ThreadIndexRepairScope::Latest,
        TARGETED_THREAD_INDEX_TIMEOUT,
    )
}

fn reconcile_codex_thread_index_with_scope(
    codex_binary: &OsStr,
    child: &ChildProcessPlan,
    scope: ThreadIndexRepairScope,
    timeout: Duration,
) -> Result<()> {
    let mut command = Command::new(codex_binary);
    command
        .arg("app-server")
        .env("CODEX_HOME", &child.codex_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for key in &child.removed_env {
        command.env_remove(key);
    }
    for (key, value) in &child.extra_env {
        command.env(key, value);
    }
    crate::configure_child_process_group(&mut command, true);
    let mut process = command.spawn().with_context(|| {
        format!(
            "failed to start {} app-server for thread index reconciliation",
            codex_binary.to_string_lossy()
        )
    })?;
    let stdin = process
        .stdin
        .take()
        .context("failed to capture thread index reconciliation stdin")?;
    let stdout = process
        .stdout
        .take()
        .context("failed to capture thread index reconciliation stdout")?;
    let (completion_tx, completion_rx) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("prodex-thread-index-reconciliation".to_string())
        .spawn(move || {
            let result = reconcile_codex_thread_index_protocol_with_scope(
                &mut BufReader::new(stdout),
                &mut BufWriter::new(stdin),
                scope,
            );
            if completion_tx.send(result).is_err() {
                // The caller has already timed out; the process cleanup below still owns
                // termination and reaping.
            }
        })
        .context("failed to start thread index reconciliation worker")?;

    let result = match completion_rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err(anyhow::anyhow!("thread index reconciliation timed out"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(anyhow::anyhow!(
            "thread index reconciliation worker stopped"
        )),
    };
    let _ = crate::terminate_child_process_tree(&mut process, true);
    let _ = process.wait();
    crate::join_thread_with_timeout(
        worker,
        THREAD_INDEX_CLEANUP_TIMEOUT,
        "thread index reconciliation worker",
    )?;
    result
}

#[cfg(test)]
pub(crate) fn reconcile_codex_thread_index_protocol(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
) -> Result<()> {
    reconcile_codex_thread_index_protocol_with_scope(reader, writer, ThreadIndexRepairScope::Full)
}

fn reconcile_codex_thread_index_protocol_with_scope(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    scope: ThreadIndexRepairScope,
) -> Result<()> {
    let mut request_id = 1_u64;
    write_app_server_message(
        writer,
        &serde_json::json!({
            "id": request_id,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "prodex-thread-index-reconciliation",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }
        }),
    )?;
    read_app_server_response(reader, request_id)?;
    write_app_server_message(writer, &serde_json::json!({"method": "initialized"}))?;

    let archives = match scope {
        ThreadIndexRepairScope::Full => [false, true].as_slice(),
        ThreadIndexRepairScope::Latest => [false].as_slice(),
    };
    for &archived in archives {
        let mut cursor = None;
        let mut seen_cursors = HashSet::new();
        loop {
            request_id += 1;
            write_app_server_message(
                writer,
                &serde_json::json!({
                    "id": request_id,
                    "method": "thread/list",
                    "params": {
                        "archived": archived,
                        "cursor": cursor,
                        "limit": match scope {
                            ThreadIndexRepairScope::Full => THREAD_INDEX_PAGE_LIMIT,
                            ThreadIndexRepairScope::Latest => 1,
                        },
                        "modelProviders": [],
                        "sortKey": "updated_at",
                        "sourceKinds": [],
                        "useStateDbOnly": false,
                    }
                }),
            )?;
            let result = read_app_server_response(reader, request_id)?;
            let next_cursor = match result.get("nextCursor") {
                None | Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::String(cursor)) => Some(cursor.clone()),
                Some(_) => bail!("Codex app-server returned an invalid thread list cursor"),
            };
            let Some(next_cursor) = next_cursor else {
                break;
            };
            if matches!(scope, ThreadIndexRepairScope::Latest) {
                break;
            }
            if !seen_cursors.insert(next_cursor.clone()) {
                bail!("Codex app-server repeated a thread list cursor");
            }
            cursor = Some(next_cursor);
        }
    }
    Ok(())
}

fn write_app_server_message(writer: &mut impl Write, message: &serde_json::Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, message).context("failed to encode app-server request")?;
    writer.write_all(b"\n")?;
    writer.flush().context("failed to send app-server request")
}

fn read_app_server_response(
    reader: &mut impl BufRead,
    request_id: u64,
) -> Result<serde_json::Value> {
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            bail!("Codex app-server stopped during thread index reconciliation");
        }
        let mut message: serde_json::Value = serde_json::from_str(&line)
            .context("Codex app-server returned invalid JSON during thread index reconciliation")?;
        if message.get("id").and_then(serde_json::Value::as_u64) != Some(request_id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            let detail = error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown app-server error");
            bail!("Codex thread index reconciliation failed: {detail}");
        }
        return message
            .get_mut("result")
            .map(serde_json::Value::take)
            .context("Codex app-server response is missing its result");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LatestThreadIndexState {
    Present,
    Stale,
    Missing,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatabaseThreadIndexState {
    Present,
    Stale,
    Missing,
    Unavailable,
}

pub(crate) fn latest_thread_index_state(
    child: &ChildProcessPlan,
    session_file: &Path,
) -> LatestThreadIndexState {
    let Some(session_id) = codex_session_id_from_path(session_file) else {
        return LatestThreadIndexState::Unavailable;
    };
    let sqlite_home = child
        .extra_env
        .iter()
        .find(|(key, _)| key == "CODEX_SQLITE_HOME")
        .map(|(_, value)| Path::new(value))
        .unwrap_or(&child.codex_home);
    let Ok(entries) = fs::read_dir(sqlite_home) else {
        return LatestThreadIndexState::Unavailable;
    };
    let mut queryable_database = false;
    let mut stale_row = false;
    for entry in entries.flatten() {
        match inspect_thread_index_database(&entry, sqlite_home, session_file, &session_id) {
            DatabaseThreadIndexState::Present => return LatestThreadIndexState::Present,
            DatabaseThreadIndexState::Stale => {
                queryable_database = true;
                stale_row = true;
            }
            DatabaseThreadIndexState::Missing => queryable_database = true,
            DatabaseThreadIndexState::Unavailable => {}
        }
    }
    if stale_row {
        return LatestThreadIndexState::Stale;
    }
    if queryable_database {
        LatestThreadIndexState::Missing
    } else {
        LatestThreadIndexState::Unavailable
    }
}

fn inspect_thread_index_database(
    entry: &fs::DirEntry,
    sqlite_home: &Path,
    session_file: &Path,
    session_id: &str,
) -> DatabaseThreadIndexState {
    let path = entry.path();
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return DatabaseThreadIndexState::Unavailable;
    };
    if !name.starts_with("state_") || !name.ends_with(".sqlite") {
        return DatabaseThreadIndexState::Unavailable;
    }
    let Ok(metadata) = entry.file_type() else {
        return DatabaseThreadIndexState::Unavailable;
    };
    if !metadata.is_file() || metadata.is_symlink() {
        return DatabaseThreadIndexState::Unavailable;
    }
    let Ok(connection) = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return DatabaseThreadIndexState::Unavailable;
    };
    let Ok(mut statement) =
        connection.prepare("SELECT rollout_path FROM threads WHERE id = ?1 OR id = ?2")
    else {
        return DatabaseThreadIndexState::Unavailable;
    };
    let thread_id = format!("thread_{session_id}");
    let Ok(rows) = statement.query_map(rusqlite::params![session_id, thread_id], |row| {
        row.get::<_, String>(0)
    }) else {
        return DatabaseThreadIndexState::Unavailable;
    };
    let mut found_row = false;
    for row in rows.flatten() {
        found_row = true;
        if state_db_rollout_path_matches(sqlite_home, session_file, &row) {
            return DatabaseThreadIndexState::Present;
        }
    }
    if found_row {
        DatabaseThreadIndexState::Stale
    } else {
        DatabaseThreadIndexState::Missing
    }
}

fn state_db_rollout_path_matches(sqlite_home: &Path, session_file: &Path, stored: &str) -> bool {
    let stored_path = Path::new(stored);
    let stored_path = if stored_path.is_absolute() {
        stored_path.to_path_buf()
    } else {
        sqlite_home.join(stored_path)
    };
    let stored_plain = plain_rollout_path(&stored_path);
    let session_plain = plain_rollout_path(session_file);
    stored_path == session_file
        || stored_plain == session_plain
        || fs::canonicalize(&stored_path)
            .ok()
            .zip(fs::canonicalize(session_file).ok())
            .is_some_and(|(stored, current)| stored == current)
        || fs::canonicalize(stored_plain)
            .ok()
            .zip(fs::canonicalize(session_plain).ok())
            .is_some_and(|(stored, current)| stored == current)
}

fn plain_rollout_path(path: &Path) -> PathBuf {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return path.to_path_buf();
    };
    name.strip_suffix(".zst")
        .map_or_else(|| path.to_path_buf(), |name| path.with_file_name(name))
}

fn codex_session_id_from_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let file_name = file_name
        .strip_suffix(".jsonl.zst")
        .or_else(|| file_name.strip_suffix(".jsonl"))
        .unwrap_or(file_name);
    let stem = Path::new(file_name).file_stem()?.to_str()?;
    if uuid::Uuid::parse_str(stem).is_ok() {
        return Some(stem.to_string());
    }
    stem.split('-')
        .collect::<Vec<_>>()
        .windows(5)
        .map(|parts| parts.join("-"))
        .find(|candidate| uuid::Uuid::parse_str(candidate).is_ok())
}

pub(crate) fn repair_dirty_thread_index(paths: &AppPaths, child: &ChildProcessPlan) {
    let marker_path = paths.root.join(THREAD_INDEX_DIRTY_FILE);
    let Some(session_file) = dirty_marker_session_file(paths, &marker_path) else {
        return;
    };
    let started = std::time::Instant::now();
    repair_latest_thread_index(paths, child, &session_file);
    crate::runtime_launch::emit_runtime_timing("startup.thread_index_targeted_repair_ms", started);
}

pub(crate) fn repair_latest_thread_index_after_child(
    paths: &AppPaths,
    child: &ChildProcessPlan,
    session_file: &Path,
) {
    let started = std::time::Instant::now();
    repair_latest_thread_index(paths, child, session_file);
    crate::runtime_launch::emit_runtime_timing("shutdown.thread_index_targeted_repair_ms", started);
}

fn repair_latest_thread_index(paths: &AppPaths, child: &ChildProcessPlan, session_file: &Path) {
    match latest_thread_index_state(child, session_file) {
        LatestThreadIndexState::Present => {
            if dirty_marker_targets(paths, session_file) {
                clear_dirty_marker(&paths.root.join(THREAD_INDEX_DIRTY_FILE));
            }
        }
        LatestThreadIndexState::Stale | LatestThreadIndexState::Missing => {
            if reconcile_latest_codex_thread_index(&child.binary, child).is_ok()
                && matches!(
                    latest_thread_index_state(child, session_file),
                    LatestThreadIndexState::Present
                )
            {
                if dirty_marker_targets(paths, session_file) {
                    clear_dirty_marker(&paths.root.join(THREAD_INDEX_DIRTY_FILE));
                }
            } else {
                save_dirty_marker(paths, session_file);
            }
        }
        LatestThreadIndexState::Unavailable if state_db_files_exist(child) => {
            save_dirty_marker(paths, session_file)
        }
        LatestThreadIndexState::Unavailable => {}
    }
}

fn state_db_files_exist(child: &ChildProcessPlan) -> bool {
    let sqlite_home = child
        .extra_env
        .iter()
        .find(|(key, _)| key == "CODEX_SQLITE_HOME")
        .map(|(_, value)| Path::new(value))
        .unwrap_or(&child.codex_home);
    fs::read_dir(sqlite_home)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("state_") && name.ends_with(".sqlite"))
        })
}

fn dirty_marker_session_file(paths: &AppPaths, marker_path: &Path) -> Option<PathBuf> {
    let contents = fs::read(marker_path).ok()?;
    let marker: ThreadIndexDirtyMarker = serde_json::from_slice(&contents).ok()?;
    if marker.schema_version != THREAD_INDEX_DIRTY_SCHEMA_VERSION {
        return None;
    }
    let relative = Path::new(&marker.rollout_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    let path = paths.shared_codex_root.join(relative);
    let metadata = fs::symlink_metadata(&path).ok()?;
    (metadata.file_type().is_file() && !metadata.file_type().is_symlink()).then_some(path)
}

fn save_dirty_marker(paths: &AppPaths, session_file: &Path) {
    let Ok(relative) = session_file.strip_prefix(&paths.shared_codex_root) else {
        return;
    };
    let marker = ThreadIndexDirtyMarker {
        schema_version: THREAD_INDEX_DIRTY_SCHEMA_VERSION,
        rollout_path: relative.to_string_lossy().into_owned(),
    };
    let Ok(contents) = serde_json::to_vec(&marker) else {
        return;
    };
    if fs::create_dir_all(&paths.root).is_ok() {
        let _ = crate::runtime_store::write_private_file_atomic(
            &paths.root.join(THREAD_INDEX_DIRTY_FILE),
            &contents,
        );
    }
}

fn clear_dirty_marker(path: &Path) {
    let _ = fs::remove_file(path);
}

fn dirty_marker_targets(paths: &AppPaths, session_file: &Path) -> bool {
    let Ok(relative) = session_file.strip_prefix(&paths.shared_codex_root) else {
        return false;
    };
    let marker_path = paths.root.join(THREAD_INDEX_DIRTY_FILE);
    let Ok(contents) = fs::read(marker_path) else {
        return false;
    };
    let Ok(marker) = serde_json::from_slice::<ThreadIndexDirtyMarker>(&contents) else {
        return false;
    };
    marker.schema_version == THREAD_INDEX_DIRTY_SCHEMA_VERSION
        && marker.rollout_path == relative.to_string_lossy()
}

#[cfg(test)]
mod tests {
    use super::{
        ChildProcessPlan, LatestThreadIndexState, ThreadIndexRepairScope,
        latest_thread_index_state, reconcile_codex_thread_index_protocol,
        reconcile_codex_thread_index_protocol_with_scope,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reconciliation_scans_all_active_and_archived_pages() {
        let responses = concat!(
            "{\"id\":1,\"result\":{}}\n",
            "{\"method\":\"remoteControl/status/changed\",\"params\":{}}\n",
            "{\"id\":2,\"result\":{\"data\":[],\"nextCursor\":\"active-next\"}}\n",
            "{\"id\":3,\"result\":{\"data\":[],\"nextCursor\":null}}\n",
            "{\"id\":4,\"result\":{\"data\":[],\"nextCursor\":null}}\n",
        );
        let mut reader = std::io::Cursor::new(responses.as_bytes());
        let mut written = Vec::new();

        reconcile_codex_thread_index_protocol(&mut reader, &mut written).unwrap();

        let requests = String::from_utf8(written)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 5);
        assert_eq!(requests[0]["method"], "initialize");
        assert_eq!(requests[1]["method"], "initialized");
        assert_eq!(requests[2]["method"], "thread/list");
        assert_eq!(requests[2]["params"]["archived"], false);
        assert_eq!(requests[2]["params"]["cursor"], serde_json::Value::Null);
        assert_eq!(requests[2]["params"]["useStateDbOnly"], false);
        assert_eq!(
            requests[2]["params"]["modelProviders"],
            serde_json::json!([])
        );
        assert_eq!(requests[3]["params"]["cursor"], "active-next");
        assert_eq!(requests[4]["params"]["archived"], true);
    }

    #[test]
    fn reconciliation_rejects_repeated_cursor() {
        let responses = concat!(
            "{\"id\":1,\"result\":{}}\n",
            "{\"id\":2,\"result\":{\"nextCursor\":\"same\"}}\n",
            "{\"id\":3,\"result\":{\"nextCursor\":\"same\"}}\n",
        );
        let mut reader = std::io::Cursor::new(responses.as_bytes());
        let mut written = Vec::new();

        let error = reconcile_codex_thread_index_protocol(&mut reader, &mut written).unwrap_err();

        assert!(error.to_string().contains("repeated"));
    }

    #[test]
    fn targeted_reconciliation_requests_only_the_newest_active_page() {
        let responses = concat!(
            "{\"id\":1,\"result\":{}}\n",
            "{\"id\":2,\"result\":{\"data\":[],\"nextCursor\":\"ignored\"}}\n",
        );
        let mut reader = std::io::Cursor::new(responses.as_bytes());
        let mut written = Vec::new();

        reconcile_codex_thread_index_protocol_with_scope(
            &mut reader,
            &mut written,
            ThreadIndexRepairScope::Latest,
        )
        .unwrap();

        let requests = String::from_utf8(written)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[2]["params"]["archived"], false);
        assert_eq!(requests[2]["params"]["limit"], 1);
        assert_eq!(requests[2]["params"]["sortKey"], "updated_at");
    }

    #[test]
    fn latest_thread_index_state_detects_missing_and_present_rows() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "prodex-thread-index-state-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("sessions")).unwrap();
        fs::create_dir_all(root.join("overlay")).unwrap();
        let session_id = "01900000-0000-7000-8000-000000000005";
        let session_file = root
            .join("sessions")
            .join(format!("rollout-{session_id}.jsonl.zst"));
        fs::write(
            &session_file,
            zstd::stream::encode_all(&b"session"[..], 3).unwrap(),
        )
        .unwrap();
        let database = root.join("state_test.sqlite");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute(
                "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL)",
                [],
            )
            .unwrap();
        drop(connection);

        let mut child = ChildProcessPlan::new("codex".into(), root.join("overlay"));
        child
            .extra_env
            .push(("CODEX_SQLITE_HOME".into(), root.clone().into_os_string()));
        assert_eq!(
            latest_thread_index_state(&child, &session_file),
            LatestThreadIndexState::Missing
        );
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute(
                "INSERT INTO threads (id, rollout_path) VALUES (?1, ?2)",
                rusqlite::params![session_id, "/stale/rollout.jsonl"],
            )
            .unwrap();
        assert_eq!(
            latest_thread_index_state(&child, &session_file),
            LatestThreadIndexState::Stale
        );
        connection
            .execute(
                "UPDATE threads SET rollout_path = ?1 WHERE id = ?2",
                rusqlite::params![
                    "sessions/rollout-01900000-0000-7000-8000-000000000005.jsonl",
                    session_id
                ],
            )
            .unwrap();
        assert_eq!(
            latest_thread_index_state(&child, &session_file),
            LatestThreadIndexState::Present
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn targeted_reconciliation_kills_a_hanging_app_server() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::Instant;

        let root = std::env::temp_dir().join(format!(
            "prodex-thread-index-hang-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let script = root.join("fake-codex.sh");
        fs::write(
            &script,
            "#!/bin/sh\nread line\nprintf '%s\\n' '{\"id\":1,\"result\":{}}'\nread line\nsleep 30\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&script, permissions).unwrap();
        let child = ChildProcessPlan::new(script.into_os_string(), root.clone());
        let started = Instant::now();
        let result = super::reconcile_latest_codex_thread_index(&child.binary, &child);
        assert!(result.is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        let _ = fs::remove_dir_all(root);
    }
}

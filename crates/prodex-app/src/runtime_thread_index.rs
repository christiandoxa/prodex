use crate::{AppPaths, ChildProcessPlan};
use anyhow::{Context, Result, bail};
pub(crate) use prodex_mojo_core::runtime::thread_index::ThreadIndexState as LatestThreadIndexState;
use prodex_mojo_core::runtime::thread_index::{
    self as thread_index_mojo, ThreadIndexProtocol, ThreadIndexProtocolStep,
    ThreadIndexRepairAction, ThreadIndexRepairProgress, ThreadIndexScope,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[path = "runtime_thread_index_json.rs"]
mod thread_index_json;
use thread_index_json::{THREAD_INDEX_MAX_JSON_BYTES, dirty_marker_value, thread_index_json_tree};

const THREAD_INDEX_TIMEOUT: Duration = Duration::from_secs(60);
const TARGETED_THREAD_INDEX_TIMEOUT: Duration = Duration::from_secs(3);
const THREAD_INDEX_CLEANUP_TIMEOUT: Duration = Duration::from_secs(1);
const THREAD_INDEX_DIRTY_FILE: &str = "thread-index-dirty.json";

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
        ThreadIndexScope::Full,
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
        ThreadIndexScope::Latest,
        TARGETED_THREAD_INDEX_TIMEOUT,
    )
}

fn reconcile_codex_thread_index_with_scope(
    codex_binary: &OsStr,
    child: &ChildProcessPlan,
    scope: ThreadIndexScope,
    timeout: Duration,
) -> Result<()> {
    crate::validate_selected_codex_binary(codex_binary)?;
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
    reconcile_codex_thread_index_protocol_with_scope(reader, writer, ThreadIndexScope::Full)
}

fn reconcile_codex_thread_index_protocol_with_scope(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    scope: ThreadIndexScope,
) -> Result<()> {
    let (mut protocol, step) =
        mojo_result(ThreadIndexProtocol::start(scope, env!("CARGO_PKG_VERSION")))?;
    if write_protocol_step(writer, step)? {
        return Ok(());
    }
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            if write_protocol_step(writer, mojo_result(protocol.eof())?)? {
                return Ok(());
            }
            continue;
        }
        if line.len() > THREAD_INDEX_MAX_JSON_BYTES {
            return Err(anyhow::anyhow!(
                "Mojo thread-index JSON input exceeded its ABI bound"
            ));
        }
        let step = match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(value) => {
                let tree = thread_index_json_tree(&value)?;
                mojo_result(protocol.response(&tree.nodes, &tree.raw, line.len()))?
            }
            Err(error) => {
                let step = mojo_result(protocol.invalid_json(line.len()))?;
                if write_invalid_json_step(writer, step, error)? {
                    return Ok(());
                }
                continue;
            }
        };
        if write_protocol_step(writer, step)? {
            return Ok(());
        }
    }
}

fn mojo_result<T>(result: std::result::Result<T, prodex_mojo_core::MojoError>) -> Result<T> {
    result.map_err(|error| anyhow::anyhow!("Mojo thread-index ABI failed: {error:?}"))
}

fn write_protocol_step(writer: &mut impl Write, step: ThreadIndexProtocolStep) -> Result<bool> {
    match step {
        ThreadIndexProtocolStep::Ignore => Ok(false),
        ThreadIndexProtocolStep::Send(messages) => {
            for message in messages {
                write_app_server_message(writer, &message)?;
            }
            Ok(false)
        }
        ThreadIndexProtocolStep::Done => Ok(true),
        ThreadIndexProtocolStep::Error(message) => bail!("{message}"),
    }
}

fn write_invalid_json_step(
    writer: &mut impl Write,
    step: ThreadIndexProtocolStep,
    parse_error: serde_json::Error,
) -> Result<bool> {
    match step {
        ThreadIndexProtocolStep::Error(message) => {
            Err(anyhow::Error::new(parse_error).context(message))
        }
        step => write_protocol_step(writer, step),
    }
}

fn write_app_server_message(writer: &mut impl Write, message: &[u8]) -> Result<()> {
    writer.write_all(message)?;
    writer.write_all(b"\n")?;
    writer.flush().context("failed to send app-server request")
}

pub(crate) fn latest_thread_index_state(
    child: &ChildProcessPlan,
    session_file: &Path,
) -> Result<LatestThreadIndexState> {
    let Some(session_id) = codex_session_id_from_path(session_file) else {
        return Ok(LatestThreadIndexState::Unavailable);
    };
    let sqlite_home = child
        .extra_env
        .iter()
        .find(|(key, _)| key == "CODEX_SQLITE_HOME")
        .map(|(_, value)| Path::new(value))
        .unwrap_or(&child.codex_home);
    let Ok(entries) = fs::read_dir(sqlite_home) else {
        return Ok(LatestThreadIndexState::Unavailable);
    };
    let mut state = LatestThreadIndexState::Unavailable;
    for entry in entries.flatten() {
        let observation =
            inspect_thread_index_database(&entry, sqlite_home, session_file, &session_id);
        state = mojo_result(thread_index_mojo::combine_state(state, observation))?;
        if state == LatestThreadIndexState::Present {
            return Ok(state);
        }
    }
    Ok(state)
}

const THREAD_PREFERENCE_SCAN_LIMIT: usize = 4_096;

fn runtime_thread_state_database_paths(sqlite_home: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(sqlite_home) else {
        return Vec::new();
    };
    let mut paths = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if !name.starts_with("state_") || !name.ends_with(".sqlite") {
                return None;
            }
            let file_type = entry.file_type().ok()?;
            (file_type.is_file() && !file_type.is_symlink()).then_some(path)
        })
        .collect::<Vec<_>>();
    paths.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    paths
}

fn open_runtime_thread_state_database(path: &Path) -> Option<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
}

pub(crate) fn runtime_thread_workspace_for_session(
    sqlite_home: &Path,
    session_id: &str,
) -> Option<PathBuf> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return None;
    }
    let thread_id = format!("thread_{session_id}");
    for path in runtime_thread_state_database_paths(sqlite_home) {
        let Some(connection) = open_runtime_thread_state_database(&path) else {
            continue;
        };
        let workspace = connection
            .query_row(
                "SELECT cwd FROM threads WHERE id = ?1 OR id = ?2 LIMIT 1",
                rusqlite::params![session_id, thread_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten()
            .map(|value| PathBuf::from(value.trim()))
            .filter(|workspace| workspace.is_absolute());
        if workspace.is_some() {
            return workspace;
        }
    }
    None
}

pub(crate) fn latest_runtime_thread_model_selection(
    sqlite_home: &Path,
    provider: prodex_provider_core::ProviderId,
) -> Option<(String, Option<String>)> {
    let mut best: Option<(i64, String, Option<String>)> = None;
    for path in runtime_thread_state_database_paths(sqlite_home) {
        let Some(connection) = open_runtime_thread_state_database(&path) else {
            continue;
        };
        let mut statement = match connection.prepare(
            "SELECT model_provider, model, reasoning_effort, updated_at_ms \
             FROM threads WHERE model IS NOT NULL \
             ORDER BY updated_at_ms DESC LIMIT ?1",
        ) {
            Ok(statement) => statement,
            Err(_) => match connection.prepare(
                "SELECT model_provider, model, reasoning_effort, updated_at * 1000 \
                 FROM threads WHERE model IS NOT NULL \
                 ORDER BY updated_at DESC LIMIT ?1",
            ) {
                Ok(statement) => statement,
                Err(_) => continue,
            },
        };
        let Ok(rows) = statement.query_map([THREAD_PREFERENCE_SCAN_LIMIT as i64], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3).unwrap_or_default(),
            ))
        }) else {
            continue;
        };
        for row in rows.flatten() {
            let (model_provider, model, reasoning_effort, updated_at_ms) = row;
            let Some(model_provider) = model_provider.as_deref() else {
                continue;
            };
            if prodex_provider_core::provider_implementation_registry()
                .resolve_model_provider_id(model_provider)
                != Some(provider)
            {
                continue;
            }
            let Some(model) = model
                .as_deref()
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .map(str::to_string)
            else {
                continue;
            };
            let reasoning_effort = reasoning_effort
                .as_deref()
                .map(str::trim)
                .filter(|effort| !effort.is_empty())
                .map(str::to_string);
            if best
                .as_ref()
                .is_none_or(|(best_updated, _, _)| updated_at_ms > *best_updated)
            {
                best = Some((updated_at_ms, model, reasoning_effort));
            }
            break;
        }
    }
    best.map(|(_, model, reasoning_effort)| (model, reasoning_effort))
}

fn inspect_thread_index_database(
    entry: &fs::DirEntry,
    sqlite_home: &Path,
    session_file: &Path,
    session_id: &str,
) -> LatestThreadIndexState {
    let path = entry.path();
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return LatestThreadIndexState::Unavailable;
    };
    if !name.starts_with("state_") || !name.ends_with(".sqlite") {
        return LatestThreadIndexState::Unavailable;
    }
    let Ok(metadata) = entry.file_type() else {
        return LatestThreadIndexState::Unavailable;
    };
    if !metadata.is_file() || metadata.is_symlink() {
        return LatestThreadIndexState::Unavailable;
    }
    let Ok(connection) = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return LatestThreadIndexState::Unavailable;
    };
    let Ok(mut statement) =
        connection.prepare("SELECT rollout_path FROM threads WHERE id = ?1 OR id = ?2")
    else {
        return LatestThreadIndexState::Unavailable;
    };
    let thread_id = format!("thread_{session_id}");
    let Ok(rows) = statement.query_map(rusqlite::params![session_id, thread_id], |row| {
        row.get::<_, String>(0)
    }) else {
        return LatestThreadIndexState::Unavailable;
    };
    let mut found_row = false;
    for row in rows.flatten() {
        found_row = true;
        if state_db_rollout_path_matches(sqlite_home, session_file, &row) {
            return LatestThreadIndexState::Present;
        }
    }
    if found_row {
        LatestThreadIndexState::Stale
    } else {
        LatestThreadIndexState::Missing
    }
}

fn state_db_rollout_path_matches(sqlite_home: &Path, session_file: &Path, stored: &str) -> bool {
    let stored_path = Path::new(stored);
    let stored_path = if stored_path.is_absolute() {
        stored_path.to_path_buf()
    } else {
        sqlite_home.join(stored_path)
    };
    if stored_path.components().any(|component| {
        matches!(component, Component::Normal(name) if name.to_string_lossy().starts_with(".prodex-overlay-"))
    }) {
        return false;
    }
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
    let _ = repair_latest_thread_index(paths, child, &session_file);
    crate::runtime_launch::emit_runtime_timing("startup.thread_index_targeted_repair_ms", started);
}

pub(crate) fn repair_latest_thread_index_after_child(
    paths: &AppPaths,
    child: &ChildProcessPlan,
    session_file: &Path,
) {
    let started = std::time::Instant::now();
    let _ = repair_latest_thread_index(paths, child, session_file);
    crate::runtime_launch::emit_runtime_timing("shutdown.thread_index_targeted_repair_ms", started);
}

fn repair_latest_thread_index(
    paths: &AppPaths,
    child: &ChildProcessPlan,
    session_file: &Path,
) -> Result<()> {
    let index_child = persistent_index_child(paths, child, session_file);
    let initial_state = latest_thread_index_state(&index_child, session_file)?;
    let mut progress = ThreadIndexRepairProgress::Initial;
    loop {
        let action = mojo_result(thread_index_mojo::repair_action(initial_state, progress))?;
        match action {
            ThreadIndexRepairAction::CheckDatabaseFiles => {
                progress = ThreadIndexRepairProgress::DatabaseFilesChecked {
                    exist: state_db_files_exist(&index_child),
                };
            }
            ThreadIndexRepairAction::Reconcile => {
                let succeeded =
                    reconcile_latest_codex_thread_index(&index_child.binary, &index_child).is_ok();
                let verified_state = if succeeded {
                    latest_thread_index_state(&index_child, session_file)?
                } else {
                    LatestThreadIndexState::Unavailable
                };
                progress = ThreadIndexRepairProgress::ReconciliationFinished {
                    succeeded,
                    verified_state,
                };
            }
            ThreadIndexRepairAction::CheckDirtyMarker => {
                let after_reconciliation = matches!(
                    progress,
                    ThreadIndexRepairProgress::ReconciliationFinished { .. }
                );
                progress = ThreadIndexRepairProgress::DirtyMarkerChecked {
                    matches: dirty_marker_targets(paths, session_file)?,
                    after_reconciliation,
                };
            }
            ThreadIndexRepairAction::ClearDirtyMarker => {
                clear_dirty_marker(&paths.root.join(THREAD_INDEX_DIRTY_FILE));
                return Ok(());
            }
            ThreadIndexRepairAction::SaveDirtyMarker => {
                save_dirty_marker(paths, session_file)?;
                return Ok(());
            }
            ThreadIndexRepairAction::Noop => return Ok(()),
        }
    }
}

fn persistent_index_child(
    paths: &AppPaths,
    child: &ChildProcessPlan,
    session_file: &Path,
) -> ChildProcessPlan {
    if !session_file.starts_with(&paths.shared_codex_root)
        || prodex_core::same_path(&child.codex_home, &paths.shared_codex_root)
    {
        return child.clone();
    }

    let mut index_child = child.clone();
    index_child.codex_home = paths.shared_codex_root.clone();
    index_child
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
    if contents.len() > THREAD_INDEX_MAX_JSON_BYTES {
        return None;
    }
    let rollout_path = match dirty_marker_value(&contents) {
        Some(value) => {
            let tree = thread_index_json_tree(&value).ok()?;
            mojo_result(thread_index_mojo::dirty_marker_path(
                Some((&tree.nodes, &tree.raw)),
                contents.len(),
            ))
            .ok()??
        }
        None => mojo_result(thread_index_mojo::dirty_marker_path(None, contents.len())).ok()??,
    };
    let relative = Path::new(&rollout_path);
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

fn save_dirty_marker(paths: &AppPaths, session_file: &Path) -> Result<()> {
    let Ok(relative) = session_file.strip_prefix(&paths.shared_codex_root) else {
        return Ok(());
    };
    let rollout_path = relative.to_string_lossy();
    let contents = mojo_result(thread_index_mojo::dirty_marker_contents(&rollout_path))?;
    if fs::create_dir_all(&paths.root).is_ok() {
        let _ = crate::runtime_store::write_private_file_atomic(
            &paths.root.join(THREAD_INDEX_DIRTY_FILE),
            &contents,
        );
    }
    Ok(())
}

fn clear_dirty_marker(path: &Path) {
    let _ = fs::remove_file(path);
}

fn dirty_marker_targets(paths: &AppPaths, session_file: &Path) -> Result<bool> {
    let Ok(relative) = session_file.strip_prefix(&paths.shared_codex_root) else {
        return Ok(false);
    };
    let marker_path = paths.root.join(THREAD_INDEX_DIRTY_FILE);
    let Ok(contents) = fs::read(marker_path) else {
        return Ok(false);
    };
    if contents.len() > THREAD_INDEX_MAX_JSON_BYTES {
        return Ok(false);
    }
    let target = relative.to_string_lossy();
    let matched = match dirty_marker_value(&contents) {
        Some(value) => {
            let tree = thread_index_json_tree(&value)?;
            thread_index_mojo::dirty_marker_targets(
                Some((&tree.nodes, &tree.raw)),
                &target,
                contents.len(),
            )
        }
        None => thread_index_mojo::dirty_marker_targets(None, &target, contents.len()),
    };
    mojo_result(matched)
}

#[cfg(test)]
#[path = "runtime_thread_index_tests.rs"]
mod tests;

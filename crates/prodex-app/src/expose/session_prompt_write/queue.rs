use super::{
    OpenProcessFile, ProcessRecord, QUEUE_COMMAND_OUTPUT_LIMIT, QUEUE_COMMAND_TIMEOUT,
    ResolvedTarget, SessionPromptWriteError, first_codex_positional_arg, is_control_socket,
    is_rollout_file_name,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

#[cfg(unix)]
use crate::app_server_control::{
    AppServerRequestOutcome, UnixAppServerSocket, connect_unix_socket, request_result,
};
#[cfg(unix)]
use tungstenite::Message as WsMessage;
pub(crate) fn resolve_thread_identity(
    files: &[OpenProcessFile],
) -> std::result::Result<String, SessionPromptWriteError> {
    let modern = files
        .iter()
        .filter_map(|file| modern_thread_id(&file.path))
        .collect::<BTreeSet<_>>();
    let legacy = files
        .iter()
        .filter_map(|file| legacy_thread_id(&file.path))
        .collect::<BTreeSet<_>>();
    if modern.len() > 1 || legacy.len() > 1 {
        return Err(SessionPromptWriteError::ThreadIdentityConflict);
    }
    match (modern.into_iter().next(), legacy.into_iter().next()) {
        (Some(modern), Some(legacy)) if modern != legacy => {
            Err(SessionPromptWriteError::ThreadIdentityConflict)
        }
        (Some(thread_id), _) | (_, Some(thread_id)) => Ok(thread_id),
        (None, None) => Err(SessionPromptWriteError::ThreadIdentityUnavailable),
    }
}

pub(crate) fn modern_thread_id(path: &Path) -> Option<String> {
    let parent = path.parent()?.file_name()?.to_str()?;
    if parent != "thread-writer-locks" {
        return None;
    }
    let name = path.file_name()?.to_str()?.strip_suffix(".lock")?;
    Some(Uuid::parse_str(name).ok()?.to_string())
}

pub(crate) fn legacy_thread_id(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if !is_rollout_file_name(name) {
        return None;
    }
    let name = name
        .strip_suffix(".jsonl.zst")
        .or_else(|| name.strip_suffix(".jsonl"))?;
    let parts = name.split('-').collect::<Vec<_>>();
    let mut found = None;
    for window in parts.windows(5) {
        let candidate = window.join("-");
        if Uuid::parse_str(&candidate).is_ok() {
            if found.is_some() {
                return None;
            }
            found = Some(candidate);
        }
    }
    found.and_then(|value| Uuid::parse_str(&value).ok().map(|id| id.to_string()))
}

#[derive(Clone, Copy)]
pub(crate) enum DatabaseKind {
    Queue,
    State,
}

pub(crate) fn exact_open_database(
    files: &[OpenProcessFile],
    kind: DatabaseKind,
) -> std::result::Result<Option<PathBuf>, SessionPromptWriteError> {
    let mut paths = BTreeSet::new();
    for file in files {
        let Some(name) = file.path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let matches = match kind {
            DatabaseKind::Queue => name == "queue_1.sqlite",
            DatabaseKind::State => name.starts_with("state_") && name.ends_with(".sqlite"),
        };
        if !matches {
            continue;
        }
        if file.path.to_string_lossy().ends_with(" (deleted)") {
            return Err(SessionPromptWriteError::QueueDbUnavailable);
        }
        let path = file
            .path
            .canonicalize()
            .map_err(|_| SessionPromptWriteError::QueueDbUnavailable)?;
        paths.insert(path);
    }
    match paths.len() {
        0 => Ok(None),
        1 => Ok(paths.into_iter().next()),
        _ => Err(SessionPromptWriteError::QueueDbUnavailable),
    }
}

pub(crate) fn remote_endpoint(
    process: &ProcessRecord,
    open_files: &[OpenProcessFile],
    codex_home: &Path,
) -> Option<String> {
    if first_codex_positional_arg(&process.argv) != Some("app-server") {
        return None;
    }
    let mut index = 1;
    let mut value = None;
    while index < process.argv.len() {
        let argument = process.argv[index].as_str();
        if argument == "--listen" {
            value = process.argv.get(index + 1).cloned();
            break;
        }
        if let Some(value) = argument.strip_prefix("--listen=") {
            return valid_unix_endpoint(value, codex_home);
        }
        index += 1;
    }
    let value = value?;
    let endpoint = valid_unix_endpoint(&value, codex_home)?;
    let endpoint_path = endpoint.strip_prefix("unix://")?;
    let endpoint_path = Path::new(endpoint_path).canonicalize().ok()?;
    open_files
        .iter()
        .any(|file| {
            is_control_socket(&file.path)
                && file
                    .path
                    .canonicalize()
                    .ok()
                    .is_some_and(|path| path == endpoint_path)
        })
        .then_some(endpoint)
}

fn valid_unix_endpoint(value: &str, codex_home: &Path) -> Option<String> {
    let raw_path = value.strip_prefix("unix://")?;
    let path = if raw_path.is_empty() {
        codex_home.join("app-server-control/app-server-control.sock")
    } else {
        PathBuf::from(raw_path)
    };
    if !path.is_absolute()
        || value.chars().any(char::is_control)
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        || !path.starts_with(codex_home)
    {
        return None;
    }
    Some(format!("unix://{}", path.display()))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum QueueRequestOutcome {
    #[default]
    Rejected,
    Preflight,
    Accepted,
    Ambiguous,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct QueueInvocation {
    pub(crate) outcome: QueueRequestOutcome,
    pub(crate) exit_code: Option<i32>,
    pub(crate) message_id: Option<String>,
    pub(crate) submission_id: Option<String>,
    pub(crate) queued: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct QueuePreemptResult {
    pub(crate) current_turn_id: Option<String>,
    pub(crate) current_turn_interrupted: bool,
    pub(crate) cancelled_submission_ids: Vec<String>,
    pub(crate) remaining_submission_ids: Vec<String>,
    pub(crate) queue_empty_at_boundary: bool,
    pub(crate) session_ready: bool,
}

impl QueueInvocation {
    pub(crate) fn accepted(
        exit_code: Option<i32>,
        message_id: Option<String>,
        submission_id: Option<String>,
        queued: bool,
    ) -> Self {
        Self {
            outcome: QueueRequestOutcome::Accepted,
            exit_code,
            message_id,
            submission_id,
            queued,
        }
    }

    pub(crate) fn ambiguous() -> Self {
        Self {
            outcome: QueueRequestOutcome::Ambiguous,
            ..Self::default()
        }
    }

    pub(crate) fn preflight() -> Self {
        Self {
            outcome: QueueRequestOutcome::Preflight,
            ..Self::default()
        }
    }
}

pub(crate) trait QueueControl {
    fn check_capability(
        &self,
        target: &ResolvedTarget,
    ) -> std::result::Result<(), SessionPromptWriteError>;
    fn persisted_thread(
        &self,
        state_db: &Path,
        thread_id: &str,
    ) -> std::result::Result<bool, SessionPromptWriteError>;
    fn rollout_path(
        &self,
        state_db: &Path,
        thread_id: &str,
    ) -> std::result::Result<Option<PathBuf>, SessionPromptWriteError>;
    fn queue_once(&self, target: &ResolvedTarget, message: &str) -> QueueInvocation;
    fn preempt(
        &self,
        _target: &ResolvedTarget,
    ) -> std::result::Result<QueuePreemptResult, SessionPromptWriteError> {
        Err(SessionPromptWriteError::QueueUnsupported)
    }
    fn loaded_thread_addressable(
        &self,
        target: &ResolvedTarget,
    ) -> std::result::Result<bool, SessionPromptWriteError> {
        let _ = target;
        Ok(false)
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SystemQueueControl;

impl QueueControl for SystemQueueControl {
    fn check_capability(
        &self,
        target: &ResolvedTarget,
    ) -> std::result::Result<(), SessionPromptWriteError> {
        if target
            .remote_endpoint
            .as_deref()
            .is_some_and(|endpoint| endpoint.starts_with("unix://"))
        {
            return Ok(());
        }
        let output = run_codex_command(target, ["queue", "--help"])
            .map_err(|_| SessionPromptWriteError::QueueUnsupported)?;
        let text = String::from_utf8_lossy(&output);
        if text.contains("--thread") && text.contains("--message") {
            Ok(())
        } else {
            Err(SessionPromptWriteError::QueueUnsupported)
        }
    }

    fn persisted_thread(
        &self,
        state_db: &Path,
        thread_id: &str,
    ) -> std::result::Result<bool, SessionPromptWriteError> {
        let connection = open_read_only_database(state_db)?;
        let legacy_thread_id = format!("thread_{thread_id}");
        connection
            .query_row(
                "SELECT id FROM threads WHERE id = ?1 OR id = ?2 LIMIT 1",
                params![thread_id, legacy_thread_id],
                |row| row.get::<_, String>(0),
            )
            .map(|_| true)
            .or_else(|error| {
                matches!(error, rusqlite::Error::QueryReturnedNoRows)
                    .then_some(false)
                    .ok_or(error)
            })
            .map_err(|_| SessionPromptWriteError::SessionNotQueueAddressable)
    }

    fn rollout_path(
        &self,
        state_db: &Path,
        thread_id: &str,
    ) -> std::result::Result<Option<PathBuf>, SessionPromptWriteError> {
        let connection = open_read_only_database(state_db)
            .map_err(|_| SessionPromptWriteError::OutputSourceUnavailable)?;
        let legacy_thread_id = format!("thread_{thread_id}");
        connection
            .query_row(
                "SELECT rollout_path FROM threads WHERE id = ?1 OR id = ?2 LIMIT 1",
                params![thread_id, legacy_thread_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|path| path.map(PathBuf::from))
            .map_err(|_| SessionPromptWriteError::OutputSourceUnavailable)
    }

    fn queue_once(&self, target: &ResolvedTarget, message: &str) -> QueueInvocation {
        #[cfg(unix)]
        if target
            .remote_endpoint
            .as_deref()
            .is_some_and(|endpoint| endpoint.starts_with("unix://"))
        {
            return app_server_queue_add_once(target, message);
        }

        let mut arguments = vec![
            OsString::from("queue"),
            OsString::from("--thread"),
            OsString::from(&target.thread_id),
            OsString::from("--message"),
            OsString::from(message),
        ];
        if let Some(remote_endpoint) = target.remote_endpoint.as_deref() {
            arguments.extend([OsString::from("--remote"), OsString::from(remote_endpoint)]);
        }
        let Ok(output) = run_codex_command(target, arguments) else {
            return QueueInvocation::ambiguous();
        };
        let Some(message_id) = parse_message_id(&output) else {
            return QueueInvocation::ambiguous();
        };
        QueueInvocation::accepted(Some(0), Some(message_id), None, true)
    }

    fn preempt(
        &self,
        target: &ResolvedTarget,
    ) -> std::result::Result<QueuePreemptResult, SessionPromptWriteError> {
        #[cfg(unix)]
        if target
            .remote_endpoint
            .as_deref()
            .is_some_and(|endpoint| endpoint.starts_with("unix://"))
        {
            return super::preempt::app_server_preempt(target);
        }
        Err(SessionPromptWriteError::QueueUnsupported)
    }

    #[cfg(unix)]
    fn loaded_thread_addressable(
        &self,
        target: &ResolvedTarget,
    ) -> std::result::Result<bool, SessionPromptWriteError> {
        let Some(mut socket) = app_server_socket(target)? else {
            return Ok(false);
        };
        let mut request_id = 1;
        Ok(app_server_thread_activity(&mut socket, target, false, &mut request_id)?.is_some())
    }
}

#[cfg(unix)]
fn app_server_queue_add_once(target: &ResolvedTarget, message: &str) -> QueueInvocation {
    let Ok(Some(mut socket)) = app_server_socket(target) else {
        return QueueInvocation::preflight();
    };
    let mut request_id = 1;
    if !matches!(
        app_server_thread_activity(&mut socket, target, false, &mut request_id),
        Ok(Some(_))
    ) {
        return QueueInvocation::preflight();
    }
    let message_id = Uuid::now_v7().to_string();
    let params = serde_json::json!({
        "threadId": target.thread_id,
        "clientUserMessageId": message_id,
        "input": [{"type": "text", "text": message, "textElements": []}],
    });
    let result = match request_result(
        &mut socket,
        super::preempt::next_request_id(&mut request_id),
        "thread/queue/add",
        params,
    ) {
        AppServerRequestOutcome::Accepted(result) => result,
        AppServerRequestOutcome::Rejected => return QueueInvocation::default(),
        AppServerRequestOutcome::Ambiguous => return QueueInvocation::ambiguous(),
    };
    let Some(queued_submission) = result.get("queuedSubmission") else {
        return QueueInvocation::ambiguous();
    };
    let Some(submission_id) = queued_submission
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
    else {
        return QueueInvocation::ambiguous();
    };
    if queued_submission
        .get("clientUserMessageId")
        .and_then(serde_json::Value::as_str)
        != Some(message_id.as_str())
        || !queued_submission
            .get("input")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|input| {
                let [text] = input.as_slice() else {
                    return false;
                };
                text.get("type").and_then(serde_json::Value::as_str) == Some("text")
                    && text.get("text").and_then(serde_json::Value::as_str) == Some(message)
                    && text
                        .get("textElements")
                        .is_none_or(|elements| elements.as_array().is_some_and(Vec::is_empty))
            })
    {
        return QueueInvocation::ambiguous();
    }
    QueueInvocation::accepted(Some(0), Some(message_id), Some(submission_id), true)
}

#[cfg(unix)]
pub(super) fn app_server_socket(
    target: &ResolvedTarget,
) -> std::result::Result<Option<UnixAppServerSocket>, SessionPromptWriteError> {
    let Some(endpoint) = target.remote_endpoint.as_deref() else {
        return Ok(None);
    };
    let Some(socket_path) = endpoint.strip_prefix("unix://") else {
        return Ok(None);
    };
    let socket_path = Path::new(socket_path);
    if !socket_path.is_absolute() {
        return Ok(None);
    }
    let socket = connect_unix_socket(socket_path)
        .map_err(|_| SessionPromptWriteError::SessionNotQueueAddressable)?;
    Ok(Some(socket))
}

#[cfg(unix)]
#[derive(Debug)]
pub(super) struct AppServerThreadActivity {
    pub(super) active: bool,
    pub(super) active_turn_id: Option<String>,
}

#[cfg(unix)]
pub(super) fn app_server_thread_activity(
    socket: &mut UnixAppServerSocket,
    target: &ResolvedTarget,
    include_turns: bool,
    request_id: &mut u64,
) -> std::result::Result<Option<AppServerThreadActivity>, SessionPromptWriteError> {
    let Some(initialize) = app_server_request_result(
        socket,
        super::preempt::next_request_id(request_id),
        "initialize",
        serde_json::json!({
            "clientInfo": {
                "name": "prodex-session-bridge",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {"experimentalApi": true},
        }),
    )?
    else {
        return Ok(None);
    };
    let Some(server_home) = initialize
        .get("codexHome")
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(None);
    };
    if !prodex_core::same_path(Path::new(server_home), &target.environment.codex_home) {
        return Ok(None);
    }
    socket
        .send(WsMessage::Text(
            serde_json::json!({"method": "initialized"})
                .to_string()
                .into(),
        ))
        .map_err(|_| SessionPromptWriteError::SessionNotQueueAddressable)?;
    let Some(result) = app_server_request_result(
        socket,
        super::preempt::next_request_id(request_id),
        "thread/read",
        serde_json::json!({
            "threadId": target.thread_id,
            "includeTurns": include_turns,
        }),
    )?
    else {
        return Ok(None);
    };
    let Some(thread) = result.get("thread") else {
        return Ok(None);
    };
    let id_matches =
        thread.get("id").and_then(serde_json::Value::as_str) == Some(&target.thread_id);
    let session_matches = thread.get("sessionId").and_then(serde_json::Value::as_str)
        == Some(target.thread_id.as_str());
    let non_ephemeral = thread.get("ephemeral").and_then(serde_json::Value::as_bool) == Some(false);
    let accepts_input = thread
        .get("canAcceptDirectInput")
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    if !(id_matches && session_matches && non_ephemeral && accepts_input) {
        return Ok(None);
    }
    let Some(thread_cwd) = thread.get("cwd").and_then(serde_json::Value::as_str) else {
        return Ok(None);
    };
    if !prodex_core::same_path(Path::new(thread_cwd), Path::new(&target.environment.pwd)) {
        return Ok(None);
    }
    let Some(status) = thread
        .get("status")
        .and_then(|status| status.get("type"))
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(None);
    };
    let active = match status {
        "active" => true,
        "idle" => false,
        _ => return Ok(None),
    };
    let active_turn_id = if include_turns {
        thread_active_turn_id(thread)?
    } else {
        None
    };
    Ok(Some(AppServerThreadActivity {
        active,
        active_turn_id,
    }))
}

#[cfg(unix)]
fn thread_active_turn_id(
    thread: &serde_json::Value,
) -> std::result::Result<Option<String>, SessionPromptWriteError> {
    let Some(turns) = thread.get("turns").and_then(serde_json::Value::as_array) else {
        return Err(SessionPromptWriteError::VerificationInconclusive);
    };
    let active_turns = turns
        .iter()
        .filter(|turn| {
            turn.get("status")
                .and_then(|status| status.get("type"))
                .and_then(serde_json::Value::as_str)
                == Some("inProgress")
        })
        .map(|turn| {
            turn.get("id")
                .and_then(serde_json::Value::as_str)
                .filter(|id| !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control))
                .map(str::to_string)
                .ok_or(SessionPromptWriteError::VerificationInconclusive)
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if active_turns.len() > 1 {
        return Err(SessionPromptWriteError::VerificationInconclusive);
    }
    Ok(active_turns.into_iter().next())
}

#[cfg(unix)]
fn app_server_request_result(
    socket: &mut UnixAppServerSocket,
    request_id: u64,
    method: &str,
    params: serde_json::Value,
) -> std::result::Result<Option<serde_json::Value>, SessionPromptWriteError> {
    match request_result(socket, request_id, method, params) {
        AppServerRequestOutcome::Accepted(value) => Ok(Some(value)),
        AppServerRequestOutcome::Rejected | AppServerRequestOutcome::Ambiguous => Ok(None),
    }
}

fn open_read_only_database(
    path: &Path,
) -> std::result::Result<Connection, SessionPromptWriteError> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| SessionPromptWriteError::VerificationInconclusive)
}

fn run_codex_command<I, S>(target: &ResolvedTarget, arguments: I) -> anyhow::Result<Vec<u8>>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut command = Command::new(&target.writer.executable);
    command.env_clear().current_dir(&target.environment.pwd);
    command.env("HOME", &target.environment.home);
    command.env("CODEX_HOME", &target.environment.codex_home);
    command.env("CODEX_SQLITE_HOME", &target.environment.codex_sqlite_home);
    command.env("PWD", &target.environment.pwd);
    let arguments = arguments
        .into_iter()
        .map(Into::into)
        .collect::<Vec<OsString>>();
    command.args(arguments);
    let output = crate::command_output_with_timeout(
        &mut command,
        QUEUE_COMMAND_TIMEOUT,
        QUEUE_COMMAND_OUTPUT_LIMIT,
        "Codex queue",
    )?;
    let mut bytes = output.stdout;
    bytes.extend_from_slice(&output.stderr);
    if !output.status.success() {
        anyhow::bail!("Codex queue exited unsuccessfully")
    }
    Ok(bytes)
}

fn parse_message_id(output: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(output);
    let suffix = text.strip_prefix("Queued message ").or_else(|| {
        text.lines()
            .find_map(|line| line.strip_prefix("Queued message "))
    })?;
    let candidate = suffix
        .split_whitespace()
        .next()?
        .trim_matches(|character: char| !character.is_ascii_hexdigit() && character != '-');
    Uuid::parse_str(candidate).ok().map(|id| id.to_string())
}

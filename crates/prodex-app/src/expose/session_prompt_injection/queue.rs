use super::{
    OpenProcessFile, ProcessRecord, PromptInjectionError, QUEUE_COMMAND_OUTPUT_LIMIT,
    QUEUE_COMMAND_TIMEOUT, ResolvedTarget, first_codex_positional_arg, is_control_socket,
    is_rollout_file_name,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;
pub(crate) fn resolve_thread_identity(
    files: &[OpenProcessFile],
) -> std::result::Result<String, PromptInjectionError> {
    let modern = files
        .iter()
        .filter_map(|file| modern_thread_id(&file.path))
        .collect::<BTreeSet<_>>();
    let legacy = files
        .iter()
        .filter_map(|file| legacy_thread_id(&file.path))
        .collect::<BTreeSet<_>>();
    if modern.len() > 1 || legacy.len() > 1 {
        return Err(PromptInjectionError::ThreadIdentityUnavailable);
    }
    match (modern.into_iter().next(), legacy.into_iter().next()) {
        (Some(modern), Some(legacy)) if modern != legacy => {
            Err(PromptInjectionError::ThreadIdentityConflict)
        }
        (Some(thread_id), _) | (_, Some(thread_id)) => Ok(thread_id),
        (None, None) => Err(PromptInjectionError::ThreadIdentityUnavailable),
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
) -> std::result::Result<Option<PathBuf>, PromptInjectionError> {
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
            return Err(PromptInjectionError::QueueDbUnavailable);
        }
        let path = file
            .path
            .canonicalize()
            .map_err(|_| PromptInjectionError::QueueDbUnavailable)?;
        paths.insert(path);
    }
    match paths.len() {
        0 => Ok(None),
        1 => Ok(paths.into_iter().next()),
        _ => Err(PromptInjectionError::QueueDbUnavailable),
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

#[derive(Clone, Debug, Default)]
pub(crate) struct QueueSnapshot {
    pub(crate) item_ids: BTreeSet<String>,
    pub(crate) revision: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct QueueInvocation {
    pub(crate) succeeded: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) message_id: Option<String>,
}

pub(crate) trait QueueControl {
    fn check_capability(
        &self,
        target: &ResolvedTarget,
    ) -> std::result::Result<(), PromptInjectionError>;
    fn persisted_thread(
        &self,
        state_db: &Path,
        thread_id: &str,
    ) -> std::result::Result<bool, PromptInjectionError>;
    fn rollout_path(
        &self,
        state_db: &Path,
        thread_id: &str,
    ) -> std::result::Result<Option<PathBuf>, PromptInjectionError>;
    fn snapshot(
        &self,
        queue_db: &Path,
        thread_id: &str,
    ) -> std::result::Result<QueueSnapshot, PromptInjectionError>;
    fn queue_once(&self, target: &ResolvedTarget, message: &str) -> QueueInvocation;
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SystemQueueControl;

impl QueueControl for SystemQueueControl {
    fn check_capability(
        &self,
        target: &ResolvedTarget,
    ) -> std::result::Result<(), PromptInjectionError> {
        let output = run_codex_command(target, ["queue", "--help"])
            .map_err(|_| PromptInjectionError::QueueUnsupported)?;
        let text = String::from_utf8_lossy(&output);
        if text.contains("--thread") && text.contains("--message") {
            Ok(())
        } else {
            Err(PromptInjectionError::QueueUnsupported)
        }
    }

    fn persisted_thread(
        &self,
        state_db: &Path,
        thread_id: &str,
    ) -> std::result::Result<bool, PromptInjectionError> {
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
            .map_err(|_| PromptInjectionError::SessionNotQueueAddressable)
    }

    fn rollout_path(
        &self,
        state_db: &Path,
        thread_id: &str,
    ) -> std::result::Result<Option<PathBuf>, PromptInjectionError> {
        let connection = open_read_only_database(state_db)
            .map_err(|_| PromptInjectionError::OutputSourceUnavailable)?;
        let legacy_thread_id = format!("thread_{thread_id}");
        connection
            .query_row(
                "SELECT rollout_path FROM threads WHERE id = ?1 OR id = ?2 LIMIT 1",
                params![thread_id, legacy_thread_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|path| path.map(PathBuf::from))
            .map_err(|_| PromptInjectionError::OutputSourceUnavailable)
    }

    fn snapshot(
        &self,
        queue_db: &Path,
        thread_id: &str,
    ) -> std::result::Result<QueueSnapshot, PromptInjectionError> {
        let connection = open_read_only_database(queue_db)?;
        let mut statement = connection
            .prepare(
                "SELECT id, thread_id, queue_order, created_at_ms, updated_at_ms \
                 FROM queued_items WHERE thread_id = ?1 ORDER BY queue_order LIMIT 101",
            )
            .map_err(|_| PromptInjectionError::VerificationInconclusive)?;
        let rows = statement
            .query_map(params![thread_id], |row| {
                let id = row.get::<_, String>(0)?;
                let stored_thread_id = row.get::<_, String>(1)?;
                let _queue_order = row.get::<_, i64>(2)?;
                let _created_at_ms = row.get::<_, i64>(3)?;
                let _updated_at_ms = row.get::<_, i64>(4)?;
                Ok((id, stored_thread_id))
            })
            .map_err(|_| PromptInjectionError::VerificationInconclusive)?;
        let mut item_ids = BTreeSet::new();
        for row in rows {
            let (id, stored_thread_id) =
                row.map_err(|_| PromptInjectionError::VerificationInconclusive)?;
            if stored_thread_id != thread_id {
                return Err(PromptInjectionError::VerificationInconclusive);
            }
            item_ids.insert(id);
        }
        let revision = connection
            .query_row(
                "SELECT revision FROM queued_thread_revisions WHERE thread_id = ?1 LIMIT 1",
                params![thread_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|_| PromptInjectionError::VerificationInconclusive)?;
        Ok(QueueSnapshot { item_ids, revision })
    }

    fn queue_once(&self, target: &ResolvedTarget, message: &str) -> QueueInvocation {
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
            return QueueInvocation::default();
        };
        let message_id = parse_message_id(&output);
        QueueInvocation {
            succeeded: true,
            exit_code: Some(0),
            message_id,
        }
    }
}

fn open_read_only_database(path: &Path) -> std::result::Result<Connection, PromptInjectionError> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| PromptInjectionError::VerificationInconclusive)
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

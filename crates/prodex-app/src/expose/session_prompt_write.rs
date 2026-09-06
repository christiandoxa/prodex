//! Safe prompt delivery to the one interactive Codex session owned by this workspace.
//!
//! This module deliberately keeps process discovery and queue transport together. The queue
//! target is not a path guessed from a home directory: it is assembled from the proven writer's
//! process-bound files and environment, then checked again immediately before queueing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[path = "session_prompt_write/output.rs"]
mod output;
#[path = "session_prompt_write/process.rs"]
mod process;
#[path = "session_prompt_write/queue.rs"]
mod queue;
#[path = "session_prompt_write/read.rs"]
mod read;
#[path = "session_prompt_write/write.rs"]
mod write;
pub(super) use self::output::*;
pub(super) use self::process::*;
pub(super) use self::queue::*;

pub(super) const SESSION_PROMPT_WRITE_MAX_MESSAGE_BYTES: usize = 64 * 1024;

const TARGET_ENV_KEYS: [&str; 4] = ["HOME", "CODEX_HOME", "CODEX_SQLITE_HOME", "PWD"];
const QUEUE_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
const QUEUE_COMMAND_OUTPUT_LIMIT: usize = 64 * 1024;
const PROCESS_ANCESTRY_LIMIT: usize = 64;
const OUTPUT_CURSOR_VERSION: u8 = 1;
// JSON escaping can expand a bounded bridge prompt beyond 128 KiB.
const OUTPUT_READ_MAX_BYTES: usize = 512 * 1024;
const OUTPUT_READ_MAX_LINE_BYTES: usize = 64 * 1024;
const OUTPUT_VERIFY_MAX_LINE_BYTES: usize = 512 * 1024;
const OUTPUT_READ_MAX_TEXT_BYTES: usize = 8 * 1024;
const OUTPUT_READ_MAX_TOTAL_TEXT_BYTES: usize = 256 * 1024;
const OUTPUT_SOURCE_PROBE_BYTES: usize = 64 * 1024;
const OUTPUT_SKIP_MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SessionPromptWriteError {
    NoSession,
    AmbiguousSession,
    NoCodexWriter,
    AmbiguousCodexWriter,
    ThreadIdentityUnavailable,
    ThreadIdentityConflict,
    QueueDbUnavailable,
    SessionNotQueueAddressable,
    TargetEnvironmentUnavailable,
    QueueUnsupported,
    StaleTarget,
    QueueFailed,
    WriteAmbiguous,
    VerificationInconclusive,
    OutputSourceUnavailable,
    OutputSourceAmbiguous,
    OutputSourceChanged,
    InvalidCursor,
    StaleCursor,
    RecoveryFailed,
    OutputReadFailed,
}

impl SessionPromptWriteError {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::NoSession => "no_session",
            Self::AmbiguousSession => "ambiguous_session",
            Self::NoCodexWriter => "no_codex_writer",
            Self::AmbiguousCodexWriter => "ambiguous_codex_writer",
            Self::ThreadIdentityUnavailable => "thread_identity_unavailable",
            Self::ThreadIdentityConflict => "thread_identity_conflict",
            Self::QueueDbUnavailable => "queue_db_unavailable",
            Self::SessionNotQueueAddressable => "session_not_queue_addressable",
            Self::TargetEnvironmentUnavailable => "target_environment_unavailable",
            Self::QueueUnsupported => "queue_unsupported",
            Self::StaleTarget => "stale_target",
            Self::QueueFailed => "queue_failed",
            Self::WriteAmbiguous => "write_ambiguous",
            Self::VerificationInconclusive => "verification_inconclusive",
            Self::OutputSourceUnavailable => "output_source_unavailable",
            Self::OutputSourceAmbiguous => "output_source_ambiguous",
            Self::OutputSourceChanged => "output_source_changed",
            Self::InvalidCursor => "invalid_cursor",
            Self::StaleCursor => "stale_cursor",
            Self::RecoveryFailed => "recovery_failed",
            Self::OutputReadFailed => "output_read_failed",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct SessionPromptWriteRequest {
    pub(super) workspace_root: PathBuf,
    pub(super) message: String,
    pub(super) cwd: Option<String>,
    pub(super) prodex_pid: Option<u32>,
    pub(super) thread_id: Option<String>,
    pub(super) binding_key: String,
}

#[derive(Debug)]
pub(super) struct SessionPromptWriteSuccess {
    pub(super) prodex_pid: u32,
    pub(super) codex_pid: u32,
    pub(super) thread_id: String,
    pub(super) message_id: Option<String>,
    pub(super) submission_id: Option<String>,
    pub(super) output_cursor: Option<String>,
    pub(super) queue_exit: i32,
    pub(super) verification: &'static str,
}

#[derive(Clone, Debug)]
pub(super) struct PromptOutputReadRequest {
    pub(super) workspace_root: PathBuf,
    pub(super) cursor: Option<String>,
    pub(super) limit: usize,
    pub(super) wait_ms: u64,
    pub(super) prodex_pid: Option<u32>,
    pub(super) thread_id: Option<String>,
    pub(super) binding_key: String,
    pub(super) shutdown: Option<Arc<AtomicBool>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PromptOutputEvent {
    pub(super) sequence: u64,
    pub(super) timestamp: String,
    pub(super) kind: String,
    pub(super) name: Option<String>,
    pub(super) status: Option<String>,
    pub(super) text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PromptOutputReadSuccess {
    pub(super) prodex_pid: u32,
    pub(super) codex_pid: u32,
    pub(super) thread_id: String,
    pub(super) source: &'static str,
    pub(super) events: Vec<PromptOutputEvent>,
    pub(super) next_cursor: String,
    pub(super) has_more: bool,
}

pub(super) trait ExistingSessionPromptWrite: Send + Sync {
    fn write(
        &self,
        request: SessionPromptWriteRequest,
    ) -> std::result::Result<SessionPromptWriteSuccess, SessionPromptWriteError>;
    fn read_output(
        &self,
        request: PromptOutputReadRequest,
    ) -> std::result::Result<PromptOutputReadSuccess, SessionPromptWriteError>;
}

pub(super) struct SessionPromptWriteService<P = SystemProcessInspector, Q = SystemQueueControl> {
    process: P,
    queue: Q,
    bindings: Mutex<HashMap<String, SessionBinding>>,
}

#[derive(Clone, Debug)]
struct SessionBinding {
    target: ResolvedTarget,
    source_id: Option<String>,
}

impl Default for SessionPromptWriteService {
    fn default() -> Self {
        Self {
            process: SystemProcessInspector,
            queue: SystemQueueControl,
            bindings: Mutex::new(HashMap::new()),
        }
    }
}

impl<P, Q> SessionPromptWriteService<P, Q> {
    #[cfg(test)]
    pub(super) fn with_adapters(process: P, queue: Q) -> Self {
        Self {
            process,
            queue,
            bindings: Mutex::new(HashMap::new()),
        }
    }
}

impl<P, Q> ExistingSessionPromptWrite for SessionPromptWriteService<P, Q>
where
    P: ProcessInspector + Send + Sync,
    Q: QueueControl + Send + Sync,
{
    fn write(
        &self,
        request: SessionPromptWriteRequest,
    ) -> std::result::Result<SessionPromptWriteSuccess, SessionPromptWriteError> {
        let workspace_root = write::canonical_session_prompt_write_workspace(&request)?;
        let binding = self.binding(&request.binding_key)?;
        let mut target =
            self.resolve_session_prompt_write_target(&request, &workspace_root, binding.as_ref())?;
        let rollout_before = match self.output_source(&target) {
            Ok(path) => {
                let metadata = std::fs::metadata(&path)
                    .map_err(|_| SessionPromptWriteError::OutputSourceChanged)?;
                let source_id = output_source_id(&path, &target.thread_id)?;
                Some((path, metadata.len(), source_id))
            }
            Err(SessionPromptWriteError::OutputSourceUnavailable) => None,
            Err(error) => return Err(error),
        };
        target = self.revalidate(&target, &workspace_root)?;

        let invocation = self.queue.queue_once(&target, &request.message);
        let verification = self.verify_queue_invocation(
            &request,
            &workspace_root,
            &target,
            rollout_before.as_ref(),
            &invocation,
        )?;

        let output_cursor = rollout_before
            .as_ref()
            .and_then(|before| output_cursor_anchor(&target, before));
        let result = SessionPromptWriteSuccess {
            prodex_pid: target.prodex.pid,
            codex_pid: target.writer.pid,
            thread_id: target.thread_id.clone(),
            message_id: invocation.message_id,
            submission_id: invocation.submission_id,
            output_cursor,
            queue_exit: invocation.exit_code.unwrap_or_default(),
            verification,
        };
        self.remember_binding(
            &request.binding_key,
            target,
            rollout_before
                .as_ref()
                .map(|(_, _, source_id)| source_id.clone()),
        )?;
        Ok(result)
    }

    fn read_output(
        &self,
        request: PromptOutputReadRequest,
    ) -> std::result::Result<PromptOutputReadSuccess, SessionPromptWriteError> {
        SessionPromptWriteService::read_output(self, request)
    }
}

impl<P, Q> SessionPromptWriteService<P, Q>
where
    P: ProcessInspector,
    Q: QueueControl,
{
    fn binding(
        &self,
        key: &str,
    ) -> std::result::Result<Option<SessionBinding>, SessionPromptWriteError> {
        self.bindings
            .lock()
            .map(|bindings| bindings.get(key).cloned())
            .map_err(|_| SessionPromptWriteError::VerificationInconclusive)
    }

    fn verify_binding(
        &self,
        binding: Option<&SessionBinding>,
        target: &ResolvedTarget,
    ) -> std::result::Result<(), SessionPromptWriteError> {
        let Some(binding) = binding else {
            return Ok(());
        };
        if !same_process_identity(&binding.target.prodex, &target.prodex)
            || !same_process_identity(&binding.target.writer, &target.writer)
            || binding.target.prodex.uid != target.prodex.uid
            || binding.target.thread_id != target.thread_id
            || binding.target.environment != target.environment
            || binding.target.queue_db != target.queue_db
            || binding.target.state_db != target.state_db
            || binding.target.remote_endpoint != target.remote_endpoint
        {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        Ok(())
    }

    fn target_session_is_addressable(
        &self,
        target: &ResolvedTarget,
    ) -> std::result::Result<bool, SessionPromptWriteError> {
        if first_codex_positional_arg(&target.writer.argv) == Some("app-server") {
            return self.queue.loaded_thread_addressable(target);
        }
        if self
            .queue
            .persisted_thread(&target.state_db, &target.thread_id)?
        {
            return Ok(true);
        }
        self.queue.loaded_thread_addressable(target)
    }

    fn revalidate_persisted(
        &self,
        target: &ResolvedTarget,
        workspace_root: &Path,
    ) -> std::result::Result<ResolvedTarget, SessionPromptWriteError> {
        let target = self.revalidate(target, workspace_root)?;
        if self
            .queue
            .persisted_thread(&target.state_db, &target.thread_id)
            .map_err(|_| SessionPromptWriteError::StaleTarget)?
        {
            Ok(target)
        } else {
            Err(SessionPromptWriteError::StaleTarget)
        }
    }

    fn remember_binding(
        &self,
        key: &str,
        target: ResolvedTarget,
        source_id: Option<String>,
    ) -> std::result::Result<(), SessionPromptWriteError> {
        let mut bindings = self
            .bindings
            .lock()
            .map_err(|_| SessionPromptWriteError::VerificationInconclusive)?;
        if let Some(existing) = bindings.get(key)
            && (!same_process_identity(&existing.target.prodex, &target.prodex)
                || !same_process_identity(&existing.target.writer, &target.writer)
                || existing.target.thread_id != target.thread_id)
        {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        let source_id = source_id.or_else(|| {
            bindings
                .get(key)
                .and_then(|binding| binding.source_id.clone())
        });
        bindings.insert(key.to_string(), SessionBinding { target, source_id });
        Ok(())
    }

    fn resolve_target(
        &self,
        workspace_root: &Path,
        requested_pid: Option<u32>,
    ) -> std::result::Result<ProcessRecord, SessionPromptWriteError> {
        let candidates = self.session_candidates(workspace_root, requested_pid)?;
        match candidates.as_slice() {
            [candidate] if candidate.birth_identity.is_some() => Ok(candidate.clone()),
            [candidate] => {
                let _ = candidate;
                Err(SessionPromptWriteError::VerificationInconclusive)
            }
            _ => Err(SessionPromptWriteError::AmbiguousSession),
        }
    }

    fn session_candidates(
        &self,
        workspace_root: &Path,
        requested_pid: Option<u32>,
    ) -> std::result::Result<Vec<ProcessRecord>, SessionPromptWriteError> {
        let uid = self.process.current_uid()?;
        let candidates = self
            .process
            .list()?
            .into_iter()
            .filter(|process| {
                process.uid == uid
                    && process.state.live()
                    && prodex_core::same_path(&process.cwd, workspace_root)
                    && is_plain_prodex_session(process)
                    && requested_pid.is_none_or(|pid| process.pid == pid)
            })
            .collect::<Vec<_>>();
        (!candidates.is_empty())
            .then_some(candidates)
            .ok_or(SessionPromptWriteError::NoSession)
    }

    fn resolve_target_for_request(
        &self,
        workspace_root: &Path,
        requested_pid: Option<u32>,
        requested_thread_id: Option<&str>,
        already_narrowed: bool,
    ) -> std::result::Result<ResolvedTarget, SessionPromptWriteError> {
        if !already_narrowed
            && requested_pid.is_none()
            && let Some(thread_id) = requested_thread_id
        {
            return self.resolve_target_for_thread(workspace_root, thread_id);
        }
        self.resolve_target(workspace_root, requested_pid)
            .and_then(|target| self.resolve_writer(target, workspace_root))
    }

    fn resolve_target_for_thread(
        &self,
        workspace_root: &Path,
        requested_thread_id: &str,
    ) -> std::result::Result<ResolvedTarget, SessionPromptWriteError> {
        let candidates = self.session_candidates(workspace_root, None)?;
        let mut matches = Vec::new();
        let mut retryable = None;
        let mut definitive = None;
        for candidate in candidates {
            match self.resolve_writer(candidate, workspace_root) {
                Ok(target) if target.thread_id == requested_thread_id => matches.push(target),
                Ok(_) => {}
                Err(error) if write::session_prompt_write_resolution_retryable(error) => {
                    retryable.get_or_insert(error);
                }
                Err(error) => {
                    definitive.get_or_insert(error);
                }
            }
        }
        match matches.as_slice() {
            [target] if retryable.is_none() && definitive.is_none() => Ok(target.clone()),
            [] => Err(retryable
                .or(definitive)
                .unwrap_or(SessionPromptWriteError::StaleTarget)),
            [_] => Err(retryable
                .or(definitive)
                .unwrap_or(SessionPromptWriteError::AmbiguousSession)),
            _ => Err(SessionPromptWriteError::AmbiguousSession),
        }
    }

    fn resolve_writer(
        &self,
        prodex: ProcessRecord,
        workspace_root: &Path,
    ) -> std::result::Result<ResolvedTarget, SessionPromptWriteError> {
        let processes = self.process.list()?;
        let by_pid = processes
            .iter()
            .map(|process| (process.pid, process))
            .collect::<HashMap<_, _>>();
        let writers = processes
            .iter()
            .filter(|process| {
                process.uid == prodex.uid
                    && process.state.live()
                    && prodex_core::same_path(&process.cwd, workspace_root)
                    && process.pid != prodex.pid
                    && is_codex_writer(process)
                    && is_descendant_of(process.pid, prodex.pid, &by_pid)
            })
            .cloned()
            .collect::<Vec<_>>();
        let [writer] = writers.as_slice() else {
            return if writers.is_empty() {
                Err(SessionPromptWriteError::NoCodexWriter)
            } else {
                Err(SessionPromptWriteError::AmbiguousCodexWriter)
            };
        };
        if writer.birth_identity.is_none() || prodex.birth_identity.is_none() {
            return Err(SessionPromptWriteError::VerificationInconclusive);
        }
        let Some(details) = self.process.inspect(writer.pid)? else {
            return Err(SessionPromptWriteError::NoCodexWriter);
        };
        let thread_id = resolve_thread_identity(&details.open_files)?;
        let queue_db = exact_open_database(&details.open_files, DatabaseKind::Queue)?
            .ok_or(SessionPromptWriteError::QueueDbUnavailable)?;
        let state_db = exact_open_database(&details.open_files, DatabaseKind::State)?
            .ok_or(SessionPromptWriteError::SessionNotQueueAddressable)?;
        let environment = TargetEnvironment::from_details(&details, workspace_root)?;
        let expected_queue_db = environment.codex_sqlite_home.join("queue_1.sqlite");
        if expected_queue_db.canonicalize().ok().as_ref() != Some(&queue_db) {
            return Err(SessionPromptWriteError::QueueDbUnavailable);
        }
        let remote_endpoint = remote_endpoint(
            &details.record,
            &details.open_files,
            &environment.codex_home,
        );
        let target = ResolvedTarget {
            prodex,
            writer: details.record,
            thread_id,
            queue_db,
            state_db,
            environment,
            remote_endpoint,
        };
        if !self.target_session_is_addressable(&target)? {
            return Err(SessionPromptWriteError::SessionNotQueueAddressable);
        }
        Ok(target)
    }

    fn revalidate(
        &self,
        target: &ResolvedTarget,
        workspace_root: &Path,
    ) -> std::result::Result<ResolvedTarget, SessionPromptWriteError> {
        if target.prodex.birth_identity.is_none() || target.writer.birth_identity.is_none() {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        let uid = self.process.current_uid()?;
        let processes = self.process.list()?;
        let by_pid = processes
            .iter()
            .map(|process| (process.pid, process))
            .collect::<HashMap<_, _>>();
        let Some(prodex) = processes.iter().find(|process| {
            process.pid == target.prodex.pid
                && process.uid == uid
                && process.state.live()
                && prodex_core::same_path(&process.cwd, workspace_root)
                && is_plain_prodex_session(process)
                && same_process_identity(process, &target.prodex)
        }) else {
            return Err(SessionPromptWriteError::StaleTarget);
        };
        let Some(writer) = processes.iter().find(|process| {
            process.pid == target.writer.pid
                && process.uid == uid
                && process.state.live()
                && prodex_core::same_path(&process.cwd, workspace_root)
                && is_codex_writer(process)
                && same_process_identity(process, &target.writer)
                && is_descendant_of(process.pid, prodex.pid, &by_pid)
        }) else {
            return Err(SessionPromptWriteError::StaleTarget);
        };
        let Some(details) = self.process.inspect(writer.pid)? else {
            return Err(SessionPromptWriteError::StaleTarget);
        };
        let current_thread = resolve_thread_identity(&details.open_files)
            .map_err(|_| SessionPromptWriteError::StaleTarget)?;
        if current_thread != target.thread_id {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        let current_queue_db = exact_open_database(&details.open_files, DatabaseKind::Queue)
            .map_err(|_| SessionPromptWriteError::StaleTarget)?
            .ok_or(SessionPromptWriteError::StaleTarget)?;
        if current_queue_db != target.queue_db {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        let current_state_db = exact_open_database(&details.open_files, DatabaseKind::State)
            .map_err(|_| SessionPromptWriteError::StaleTarget)?
            .ok_or(SessionPromptWriteError::StaleTarget)?;
        if current_state_db != target.state_db {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        let environment = TargetEnvironment::from_details(&details, workspace_root)
            .map_err(|_| SessionPromptWriteError::StaleTarget)?;
        if environment
            .codex_sqlite_home
            .join("queue_1.sqlite")
            .canonicalize()
            .ok()
            != Some(current_queue_db.clone())
        {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        if environment != target.environment {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        let current_endpoint = remote_endpoint(
            &details.record,
            &details.open_files,
            &environment.codex_home,
        );
        if current_endpoint != target.remote_endpoint {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        let current_target = ResolvedTarget {
            prodex: prodex.clone(),
            writer: details.record,
            thread_id: current_thread,
            queue_db: current_queue_db,
            state_db: current_state_db,
            environment,
            remote_endpoint: current_endpoint,
        };
        if !self
            .target_session_is_addressable(&current_target)
            .map_err(|_| SessionPromptWriteError::StaleTarget)?
        {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        Ok(current_target)
    }

    fn output_source(
        &self,
        target: &ResolvedTarget,
    ) -> std::result::Result<PathBuf, SessionPromptWriteError> {
        let stored = self
            .queue
            .rollout_path(&target.state_db, &target.thread_id)?;
        let roots = [
            target.environment.codex_home.clone(),
            target.environment.codex_sqlite_home.clone(),
        ];
        if let Some(stored) = stored.as_deref() {
            if let Some(path) = valid_rollout_path_in_roots(stored, &roots, &target.thread_id) {
                return Ok(path);
            }
            if let Some(details) = self.process.inspect(target.writer.pid).ok().flatten()
                && let Some(path) = valid_rollout_path_in_authoritative_open_files(
                    stored,
                    &roots,
                    &details.open_files,
                    &target.thread_id,
                )
            {
                return Ok(path);
            }
            return Err(if rollout_path_exists_in_roots(stored, &roots) {
                SessionPromptWriteError::OutputSourceChanged
            } else {
                SessionPromptWriteError::OutputSourceUnavailable
            });
        }
        let mut candidates = Vec::new();
        for codex_home in roots {
            let codex_home = codex_home
                .canonicalize()
                .map_err(|_| SessionPromptWriteError::OutputSourceUnavailable)?;
            for root in [
                codex_home.join("sessions"),
                codex_home.join("archived_sessions"),
            ] {
                collect_exact_rollouts(&root, &target.thread_id, &mut candidates, 0)?;
            }
        }
        candidates.sort();
        candidates.dedup();
        match candidates.as_slice() {
            [path] => Ok(path.clone()),
            [] => Err(SessionPromptWriteError::OutputSourceUnavailable),
            _ => Err(SessionPromptWriteError::OutputSourceAmbiguous),
        }
    }
}

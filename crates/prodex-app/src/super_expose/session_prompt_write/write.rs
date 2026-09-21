use super::{
    QUEUE_COMMAND_TIMEOUT, QueueControl, QueueInvocation, QueueRequestOutcome, ResolvedTarget,
    SessionBinding, SessionPromptWriteError, SessionPromptWriteRequest, SessionPromptWriteService,
    output_source_id, rollout_contains_exact_user_message,
};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

impl<P, Q> SessionPromptWriteService<P, Q>
where
    P: super::ProcessInspector,
    Q: QueueControl,
{
    pub(super) fn resolve_session_prompt_write_target(
        &self,
        request: &SessionPromptWriteRequest,
        workspace_root: &Path,
        binding: Option<&SessionBinding>,
    ) -> std::result::Result<ResolvedTarget, SessionPromptWriteError> {
        self.resolve_session_target(
            workspace_root,
            request.prodex_pid,
            request.thread_id.as_deref(),
            binding,
        )
    }

    pub(super) fn resolve_session_preempt_target(
        &self,
        requested_pid: Option<u32>,
        requested_thread_id: Option<&str>,
        workspace_root: &Path,
        binding: Option<&SessionBinding>,
    ) -> std::result::Result<ResolvedTarget, SessionPromptWriteError> {
        self.resolve_session_target(workspace_root, requested_pid, requested_thread_id, binding)
    }

    fn resolve_session_target(
        &self,
        workspace_root: &Path,
        requested_pid: Option<u32>,
        requested_thread_id: Option<&str>,
        binding: Option<&SessionBinding>,
    ) -> std::result::Result<ResolvedTarget, SessionPromptWriteError> {
        let requested_pid =
            requested_pid.or_else(|| binding.map(|binding| binding.target.prodex.pid));
        let deadline = Instant::now() + QUEUE_COMMAND_TIMEOUT;
        loop {
            let result = self
                .resolve_target_for_request(
                    workspace_root,
                    requested_pid,
                    requested_thread_id,
                    binding.is_some(),
                )
                .map_err(|error| {
                    session_target_resolution_error(
                        binding,
                        requested_pid,
                        requested_thread_id,
                        error,
                    )
                });
            match result {
                Ok(target) => {
                    self.verify_binding(binding, &target)?;
                    self.verify_session_target_identity(requested_thread_id, binding, &target)?;
                    self.queue
                        .check_capability(&target)
                        .map_err(|_| SessionPromptWriteError::QueueUnsupported)?;
                    return Ok(target);
                }
                Err(error)
                    if session_prompt_write_resolution_retryable(error)
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn verify_session_target_identity(
        &self,
        requested_thread_id: Option<&str>,
        binding: Option<&SessionBinding>,
        target: &ResolvedTarget,
    ) -> std::result::Result<(), SessionPromptWriteError> {
        if let Some(bound_source) = binding.and_then(|binding| binding.source_id.as_deref()) {
            let path = self.output_source(target)?;
            if output_source_id(&path, &target.thread_id)?.as_str() != bound_source {
                return Err(SessionPromptWriteError::StaleTarget);
            }
        }
        if requested_thread_id.is_some_and(|thread_id| thread_id != target.thread_id) {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        Ok(())
    }

    pub(super) fn verify_queue_invocation(
        &self,
        request: &SessionPromptWriteRequest,
        workspace_root: &Path,
        target: &ResolvedTarget,
        rollout_before: Option<&(PathBuf, u64, String)>,
        invocation: &QueueInvocation,
    ) -> std::result::Result<&'static str, SessionPromptWriteError> {
        match invocation.outcome {
            QueueRequestOutcome::Rejected => Err(SessionPromptWriteError::QueueFailed),
            QueueRequestOutcome::Preflight => {
                Err(SessionPromptWriteError::SessionNotQueueAddressable)
            }
            QueueRequestOutcome::Ambiguous => Err(SessionPromptWriteError::WriteAmbiguous),
            QueueRequestOutcome::Accepted if invocation.queued => Ok("queue_pending_observed"),
            QueueRequestOutcome::Accepted => {
                self.wait_for_rollout_user_message(
                    request,
                    workspace_root,
                    target,
                    rollout_before,
                )?;
                Ok("rollout_user_event_observed")
            }
        }
    }

    fn wait_for_rollout_user_message(
        &self,
        request: &SessionPromptWriteRequest,
        workspace_root: &Path,
        target: &ResolvedTarget,
        rollout_before: Option<&(PathBuf, u64, String)>,
    ) -> std::result::Result<(), SessionPromptWriteError> {
        let deadline = Instant::now() + QUEUE_COMMAND_TIMEOUT;
        loop {
            let current_target = self.revalidate(target, workspace_root)?;
            if self.rollout_user_message_visible(
                &current_target,
                &target.thread_id,
                rollout_before,
                &request.message,
            )? {
                self.revalidate_persisted(&current_target, workspace_root)?;
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(SessionPromptWriteError::VerificationInconclusive);
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn rollout_user_message_visible(
        &self,
        target: &ResolvedTarget,
        thread_id: &str,
        rollout_before: Option<&(PathBuf, u64, String)>,
        expected: &str,
    ) -> std::result::Result<bool, SessionPromptWriteError> {
        let path = match self.output_source(target) {
            Ok(path) => path,
            Err(SessionPromptWriteError::OutputSourceUnavailable) => return Ok(false),
            Err(error) => return Err(error),
        };
        let offset = rollout_before_offset(rollout_before, &path, thread_id)?;
        match rollout_contains_exact_user_message(&path, offset, expected) {
            Ok(visible) => Ok(visible),
            Err(SessionPromptWriteError::OutputSourceUnavailable) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

fn rollout_before_offset(
    rollout_before: Option<&(PathBuf, u64, String)>,
    path: &Path,
    thread_id: &str,
) -> std::result::Result<u64, SessionPromptWriteError> {
    rollout_before.map_or(Ok(0), |(before_path, offset, source_id)| {
        if before_path.as_path() != path || output_source_id(path, thread_id)? != *source_id {
            return Err(SessionPromptWriteError::OutputSourceChanged);
        }
        Ok(*offset)
    })
}

pub(super) fn canonical_session_prompt_write_workspace(
    request: &SessionPromptWriteRequest,
) -> std::result::Result<PathBuf, SessionPromptWriteError> {
    canonical_session_workspace(&request.workspace_root, request.cwd.as_deref())
}

pub(super) fn canonical_session_workspace(
    workspace_root: &Path,
    cwd: Option<&str>,
) -> std::result::Result<PathBuf, SessionPromptWriteError> {
    let workspace_root = workspace_root
        .canonicalize()
        .map_err(|_| SessionPromptWriteError::VerificationInconclusive)?;
    if let Some(cwd) = cwd {
        let cwd = Path::new(cwd)
            .canonicalize()
            .map_err(|_| SessionPromptWriteError::StaleTarget)?;
        if cwd != workspace_root {
            return Err(SessionPromptWriteError::StaleTarget);
        }
    }
    Ok(workspace_root)
}

fn session_target_resolution_error(
    binding: Option<&SessionBinding>,
    requested_pid: Option<u32>,
    requested_thread_id: Option<&str>,
    error: SessionPromptWriteError,
) -> SessionPromptWriteError {
    if error == SessionPromptWriteError::NoSession
        && (binding.is_some() || requested_pid.is_some() || requested_thread_id.is_some())
    {
        SessionPromptWriteError::StaleTarget
    } else {
        error
    }
}

pub(super) fn session_prompt_write_resolution_retryable(error: SessionPromptWriteError) -> bool {
    matches!(
        error,
        SessionPromptWriteError::NoCodexWriter
            | SessionPromptWriteError::ThreadIdentityUnavailable
            | SessionPromptWriteError::SessionNotQueueAddressable
            | SessionPromptWriteError::TargetEnvironmentUnavailable
            | SessionPromptWriteError::VerificationInconclusive
    )
}

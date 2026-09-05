use super::{
    QUEUE_COMMAND_TIMEOUT, QueueControl, QueueInvocation, ResolvedTarget, SessionBinding,
    SessionPromptWriteError, SessionPromptWriteRequest, SessionPromptWriteService,
    output_source_id, read_output_events,
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
        let requested_pid = request
            .prodex_pid
            .or_else(|| binding.map(|binding| binding.target.prodex.pid));
        let deadline = Instant::now() + QUEUE_COMMAND_TIMEOUT;
        loop {
            let result = self
                .resolve_target(workspace_root, requested_pid)
                .map_err(|error| session_prompt_write_resolution_error(binding, error))
                .and_then(|target| self.resolve_writer(target, workspace_root));
            match result {
                Ok(target) => {
                    self.verify_binding(binding, &target)?;
                    self.verify_session_prompt_write_identity(request, binding, &target)?;
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

    fn verify_session_prompt_write_identity(
        &self,
        request: &SessionPromptWriteRequest,
        binding: Option<&SessionBinding>,
        target: &ResolvedTarget,
    ) -> std::result::Result<(), SessionPromptWriteError> {
        if let Some(bound_source) = binding.and_then(|binding| binding.source_id.as_deref()) {
            let path = self.output_source(target)?;
            if output_source_id(&path, &target.thread_id)?.as_str() != bound_source {
                return Err(SessionPromptWriteError::StaleTarget);
            }
        }
        if request
            .thread_id
            .as_deref()
            .is_some_and(|thread_id| thread_id != target.thread_id)
        {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        Ok(())
    }

    pub(super) fn verify_queue_invocation(
        &self,
        request: &SessionPromptWriteRequest,
        workspace_root: &Path,
        target: &ResolvedTarget,
        rollout_before: Option<&(PathBuf, u64)>,
        invocation: &QueueInvocation,
    ) -> std::result::Result<&'static str, SessionPromptWriteError> {
        if !invocation.succeeded {
            return Err(SessionPromptWriteError::QueueFailed);
        }
        self.wait_for_rollout_user_message(request, workspace_root, target, rollout_before)?;
        Ok("rollout_user_event_observed")
    }

    fn wait_for_rollout_user_message(
        &self,
        request: &SessionPromptWriteRequest,
        workspace_root: &Path,
        target: &ResolvedTarget,
        rollout_before: Option<&(PathBuf, u64)>,
    ) -> std::result::Result<(), SessionPromptWriteError> {
        let deadline = Instant::now() + QUEUE_COMMAND_TIMEOUT;
        loop {
            let current_target = self.revalidate(target, workspace_root)?;
            match self.output_source(&current_target) {
                Ok(path) => {
                    let offset = rollout_before.map_or(Ok(0), |(before_path, offset)| {
                        (before_path == &path)
                            .then_some(*offset)
                            .ok_or(SessionPromptWriteError::OutputSourceChanged)
                    })?;
                    let visible = match read_output_events(&path, offset, 0, 64) {
                        Ok(read) => read
                            .events
                            .iter()
                            .any(|event| event.kind == "user" && event.text == request.message),
                        Err(SessionPromptWriteError::OutputSourceUnavailable) => false,
                        Err(error) => return Err(error),
                    };
                    if visible {
                        self.revalidate_persisted(&current_target, workspace_root)?;
                        return Ok(());
                    }
                }
                Err(SessionPromptWriteError::OutputSourceUnavailable) => {}
                Err(error) => return Err(error),
            }
            if Instant::now() >= deadline {
                return Err(SessionPromptWriteError::VerificationInconclusive);
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

pub(super) fn canonical_session_prompt_write_workspace(
    request: &SessionPromptWriteRequest,
) -> std::result::Result<PathBuf, SessionPromptWriteError> {
    let workspace_root = request
        .workspace_root
        .canonicalize()
        .map_err(|_| SessionPromptWriteError::VerificationInconclusive)?;
    if let Some(cwd) = request.cwd.as_deref() {
        let cwd = Path::new(cwd)
            .canonicalize()
            .map_err(|_| SessionPromptWriteError::StaleTarget)?;
        if cwd != workspace_root {
            return Err(SessionPromptWriteError::StaleTarget);
        }
    }
    Ok(workspace_root)
}

fn session_prompt_write_resolution_error(
    binding: Option<&SessionBinding>,
    error: SessionPromptWriteError,
) -> SessionPromptWriteError {
    if binding.is_some() && error == SessionPromptWriteError::NoSession {
        SessionPromptWriteError::StaleTarget
    } else {
        error
    }
}

fn session_prompt_write_resolution_retryable(error: SessionPromptWriteError) -> bool {
    matches!(
        error,
        SessionPromptWriteError::NoCodexWriter
            | SessionPromptWriteError::ThreadIdentityUnavailable
            | SessionPromptWriteError::SessionNotQueueAddressable
            | SessionPromptWriteError::TargetEnvironmentUnavailable
            | SessionPromptWriteError::VerificationInconclusive
    )
}

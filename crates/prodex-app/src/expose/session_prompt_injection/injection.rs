use super::{
    PromptInjectionError, PromptInjectionRequest, QueueControl, QueueInvocation, QueueSnapshot,
    ResolvedTarget, SessionBinding, SessionPromptInjectionService, output_source_id,
    read_output_events,
};
use std::path::{Path, PathBuf};

impl<P, Q> SessionPromptInjectionService<P, Q>
where
    P: super::ProcessInspector,
    Q: QueueControl,
{
    pub(super) fn resolve_injection_target(
        &self,
        request: &PromptInjectionRequest,
        workspace_root: &Path,
        binding: Option<&SessionBinding>,
    ) -> std::result::Result<ResolvedTarget, PromptInjectionError> {
        let requested_pid = request
            .prodex_pid
            .or_else(|| binding.map(|binding| binding.target.prodex.pid));
        let target = self
            .resolve_target(workspace_root, requested_pid)
            .map_err(|error| injection_resolution_error(binding, error))?;
        let target = self.resolve_writer(target, workspace_root)?;
        self.verify_binding(binding, &target)?;
        self.verify_injection_identity(request, binding, &target)?;
        self.queue
            .check_capability(&target)
            .map_err(|_| PromptInjectionError::QueueUnsupported)?;
        Ok(target)
    }

    fn verify_injection_identity(
        &self,
        request: &PromptInjectionRequest,
        binding: Option<&SessionBinding>,
        target: &ResolvedTarget,
    ) -> std::result::Result<(), PromptInjectionError> {
        if let Some(bound_source) = binding.and_then(|binding| binding.source_id.as_deref()) {
            let path = self.output_source(target)?;
            if output_source_id(&path, &target.thread_id)?.as_str() != bound_source {
                return Err(PromptInjectionError::StaleTarget);
            }
        }
        if request
            .thread_id
            .as_deref()
            .is_some_and(|thread_id| thread_id != target.thread_id)
        {
            return Err(PromptInjectionError::StaleTarget);
        }
        Ok(())
    }

    pub(super) fn verify_queue_invocation(
        &self,
        request: &PromptInjectionRequest,
        workspace_root: &Path,
        target: &ResolvedTarget,
        rollout_before: Option<&(PathBuf, u64)>,
        invocation: &QueueInvocation,
        after: &QueueSnapshot,
    ) -> std::result::Result<&'static str, PromptInjectionError> {
        if !invocation.succeeded {
            return Err(PromptInjectionError::QueueFailed);
        }
        if invocation
            .message_id
            .as_deref()
            .is_some_and(|id| after.item_ids.contains(id))
        {
            self.revalidate_persisted(target, workspace_root)?;
            return Ok("queued_item_present");
        }
        if self.rollout_was_consumed(request, workspace_root, target, rollout_before) {
            return Ok("consumed_rollout");
        }
        if invocation.message_id.is_some()
            && self.revalidate_persisted(target, workspace_root).is_ok()
        {
            return Ok("queue_acknowledged");
        }
        Err(PromptInjectionError::VerificationInconclusive)
    }

    fn rollout_was_consumed(
        &self,
        request: &PromptInjectionRequest,
        workspace_root: &Path,
        target: &ResolvedTarget,
        rollout_before: Option<&(PathBuf, u64)>,
    ) -> bool {
        rollout_before.is_some_and(|(path, offset)| {
            read_output_events(path, *offset, 0, 64).is_ok_and(|read| {
                read.events
                    .iter()
                    .any(|event| event.kind == "user" && event.text == request.message)
            })
        }) && self.revalidate_persisted(target, workspace_root).is_ok()
    }
}

pub(super) fn canonical_injection_workspace(
    request: &PromptInjectionRequest,
) -> std::result::Result<PathBuf, PromptInjectionError> {
    let workspace_root = request
        .workspace_root
        .canonicalize()
        .map_err(|_| PromptInjectionError::NoSession)?;
    if let Some(cwd) = request.cwd.as_deref() {
        let cwd = Path::new(cwd)
            .canonicalize()
            .map_err(|_| PromptInjectionError::NoSession)?;
        if cwd != workspace_root {
            return Err(PromptInjectionError::NoSession);
        }
    }
    Ok(workspace_root)
}

fn injection_resolution_error(
    binding: Option<&SessionBinding>,
    error: PromptInjectionError,
) -> PromptInjectionError {
    if binding.is_some() && error == PromptInjectionError::NoSession {
        PromptInjectionError::StaleTarget
    } else {
        error
    }
}

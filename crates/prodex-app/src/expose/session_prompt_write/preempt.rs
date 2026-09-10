#[cfg(unix)]
use super::queue::{QueuePreemptResult, app_server_socket, app_server_thread_activity};
use super::write::canonical_session_workspace;
use super::{
    QueueControl, SessionPreemptRequest, SessionPreemptSuccess, SessionPromptWriteError,
    SessionPromptWriteService,
};
#[cfg(unix)]
use crate::app_server_control::{AppServerRequestOutcome, UnixAppServerSocket, request_result};
#[cfg(unix)]
use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

impl<P, Q> SessionPromptWriteService<P, Q>
where
    P: super::ProcessInspector,
    Q: QueueControl,
{
    pub(super) fn preempt_session(
        &self,
        request: SessionPreemptRequest,
    ) -> std::result::Result<SessionPreemptSuccess, SessionPromptWriteError> {
        let _operation_guard = self
            .operation_lock
            .lock()
            .map_err(|_| SessionPromptWriteError::VerificationInconclusive)?;
        let workspace_root =
            canonical_session_workspace(&request.workspace_root, request.cwd.as_deref())?;
        let binding = self.binding(&request.binding_key)?;
        let target = self.resolve_session_preempt_target(
            request.prodex_pid,
            request.thread_id.as_deref(),
            &workspace_root,
            binding.as_ref(),
        )?;
        let target = self.revalidate(&target, &workspace_root)?;
        let result = self.queue.preempt(&target)?;
        let generation = self
            .preempt_generation
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);
        self.remember_binding(&request.binding_key, target.clone(), None)?;
        Ok(SessionPreemptSuccess {
            prodex_pid: target.prodex.pid,
            codex_pid: target.writer.pid,
            thread_id: target.thread_id,
            current_turn_id: result.current_turn_id,
            current_turn_interrupted: result.current_turn_interrupted,
            cancelled_submission_ids: result.cancelled_submission_ids,
            remaining_submission_ids: result.remaining_submission_ids,
            queue_empty_at_boundary: result.queue_empty_at_boundary,
            session_ready: result.session_ready,
            generation,
        })
    }
}

#[cfg(unix)]
pub(super) fn app_server_preempt(
    target: &super::ResolvedTarget,
) -> std::result::Result<QueuePreemptResult, SessionPromptWriteError> {
    let Some(mut socket) = app_server_socket(target)? else {
        return Err(SessionPromptWriteError::QueueUnsupported);
    };
    let mut request_id = 1;
    let activity = app_server_thread_activity(&mut socket, target, true, &mut request_id)?
        .ok_or(SessionPromptWriteError::SessionNotQueueAddressable)?;
    if activity.active && activity.active_turn_id.is_none() {
        return Err(SessionPromptWriteError::VerificationInconclusive);
    }

    let (cancelled_submission_ids, queue_empty_at_boundary, _had_pending_submissions) =
        drain_queue(&mut socket, target, &mut request_id)?;
    if !queue_empty_at_boundary {
        return Err(SessionPromptWriteError::VerificationInconclusive);
    }

    let activity_at_boundary =
        app_server_thread_activity(&mut socket, target, true, &mut request_id)?
            .ok_or(SessionPromptWriteError::SessionNotQueueAddressable)?;
    if activity_at_boundary.active && activity_at_boundary.active_turn_id.is_none() {
        return Err(SessionPromptWriteError::VerificationInconclusive);
    }

    let turn_id_to_interrupt = match (
        activity.active_turn_id.as_deref(),
        activity_at_boundary.active_turn_id.as_deref(),
    ) {
        (Some(initial), Some(current)) if initial == current => Some(current.to_string()),
        (Some(_), None) | (None, None) => None,
        (Some(_), Some(_)) | (None, Some(_)) => {
            return Err(SessionPromptWriteError::VerificationInconclusive);
        }
    };
    let current_turn_interrupted = interrupt_turn(
        &mut socket,
        target,
        &mut request_id,
        turn_id_to_interrupt.as_deref(),
    )?;

    let final_activity = app_server_thread_activity(&mut socket, target, true, &mut request_id)?
        .ok_or(SessionPromptWriteError::SessionNotQueueAddressable)?;
    if final_activity.active && final_activity.active_turn_id.is_none() {
        return Err(SessionPromptWriteError::VerificationInconclusive);
    }
    if final_activity.active_turn_id.is_some() {
        return Err(SessionPromptWriteError::VerificationInconclusive);
    }
    let remaining_submission_ids = app_server_queue_list(&mut socket, target, &mut request_id)?;
    if !remaining_submission_ids.is_empty() {
        return Err(SessionPromptWriteError::VerificationInconclusive);
    }
    let session_ready = !final_activity.active
        && final_activity.active_turn_id.is_none()
        && remaining_submission_ids.is_empty();
    Ok(QueuePreemptResult {
        current_turn_id: activity.active_turn_id,
        current_turn_interrupted,
        cancelled_submission_ids,
        remaining_submission_ids,
        queue_empty_at_boundary,
        session_ready,
    })
}

#[cfg(unix)]
fn interrupt_turn(
    socket: &mut UnixAppServerSocket,
    target: &super::ResolvedTarget,
    request_id: &mut u64,
    turn_id: Option<&str>,
) -> std::result::Result<bool, SessionPromptWriteError> {
    let Some(turn_id) = turn_id else {
        return Ok(false);
    };
    match request_result(
        socket,
        next_request_id(request_id),
        "turn/interrupt",
        serde_json::json!({"threadId": target.thread_id, "turnId": turn_id}),
    ) {
        AppServerRequestOutcome::Accepted(_) => Ok(true),
        AppServerRequestOutcome::Rejected => Ok(false),
        AppServerRequestOutcome::Ambiguous => {
            Err(SessionPromptWriteError::VerificationInconclusive)
        }
    }
}

#[cfg(unix)]
fn drain_queue(
    socket: &mut UnixAppServerSocket,
    target: &super::ResolvedTarget,
    request_id: &mut u64,
) -> std::result::Result<(Vec<String>, bool, bool), SessionPromptWriteError> {
    let mut cancelled_submission_ids = Vec::new();
    let mut had_pending_submissions = false;
    for _ in 0..super::PREEMPT_QUEUE_DRAIN_ATTEMPTS {
        let queued = app_server_queue_list(socket, target, request_id)?;
        if queued.is_empty() {
            return Ok((cancelled_submission_ids, true, had_pending_submissions));
        }
        had_pending_submissions = true;
        for submission_id in queued {
            if app_server_queue_delete(socket, target, request_id, &submission_id)? {
                cancelled_submission_ids.push(submission_id);
            }
        }
    }
    Ok((cancelled_submission_ids, false, had_pending_submissions))
}

#[cfg(unix)]
pub(super) fn next_request_id(request_id: &mut u64) -> u64 {
    let current = *request_id;
    *request_id = (*request_id).saturating_add(1);
    current
}

#[cfg(unix)]
fn app_server_queue_list(
    socket: &mut UnixAppServerSocket,
    target: &super::ResolvedTarget,
    request_id: &mut u64,
) -> std::result::Result<Vec<String>, SessionPromptWriteError> {
    let result = match request_result(
        socket,
        next_request_id(request_id),
        "thread/queue/list",
        serde_json::json!({
            "threadId": target.thread_id,
            "limit": super::PREEMPT_QUEUE_LIMIT,
        }),
    ) {
        AppServerRequestOutcome::Accepted(result) => result,
        AppServerRequestOutcome::Rejected => return Err(SessionPromptWriteError::QueueFailed),
        AppServerRequestOutcome::Ambiguous => {
            return Err(SessionPromptWriteError::VerificationInconclusive);
        }
    };
    if result
        .get("nextCursor")
        .is_some_and(|cursor| !cursor.is_null())
    {
        return Err(SessionPromptWriteError::VerificationInconclusive);
    }
    let Some(data) = result.get("data").and_then(serde_json::Value::as_array) else {
        return Err(SessionPromptWriteError::VerificationInconclusive);
    };
    let mut ids = Vec::with_capacity(data.len());
    let mut seen = BTreeSet::new();
    for item in data {
        let Some(id) = item
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control))
        else {
            return Err(SessionPromptWriteError::VerificationInconclusive);
        };
        if !seen.insert(id.to_string()) {
            return Err(SessionPromptWriteError::VerificationInconclusive);
        }
        ids.push(id.to_string());
    }
    Ok(ids)
}

#[cfg(unix)]
fn app_server_queue_delete(
    socket: &mut UnixAppServerSocket,
    target: &super::ResolvedTarget,
    request_id: &mut u64,
    submission_id: &str,
) -> std::result::Result<bool, SessionPromptWriteError> {
    let result = match request_result(
        socket,
        next_request_id(request_id),
        "thread/queue/delete",
        serde_json::json!({
            "threadId": target.thread_id,
            "queuedSubmissionId": submission_id,
        }),
    ) {
        AppServerRequestOutcome::Accepted(result) => result,
        AppServerRequestOutcome::Rejected => return Err(SessionPromptWriteError::QueueFailed),
        AppServerRequestOutcome::Ambiguous => {
            return Err(SessionPromptWriteError::VerificationInconclusive);
        }
    };
    result
        .get("deleted")
        .and_then(serde_json::Value::as_bool)
        .ok_or(SessionPromptWriteError::VerificationInconclusive)
}

use super::output::{
    OutputCursor, decode_output_cursor, encode_output_cursor, output_source_id, read_output_events,
    source_checkpoint_id,
};
use super::{
    OUTPUT_CURSOR_VERSION, ProcessInspector, PromptOutputReadRequest, PromptOutputReadSuccess,
    QueueControl, SessionPromptWriteError, SessionPromptWriteService,
};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

const OUTPUT_READ_MAX_WAIT: Duration = Duration::from_secs(10);
const OUTPUT_READ_POLL_INTERVAL: Duration = Duration::from_millis(100);

impl<P, Q> SessionPromptWriteService<P, Q>
where
    P: ProcessInspector,
    Q: QueueControl,
{
    pub(super) fn read_output(
        &self,
        request: PromptOutputReadRequest,
    ) -> std::result::Result<PromptOutputReadSuccess, SessionPromptWriteError> {
        let workspace_root = request
            .workspace_root
            .canonicalize()
            .map_err(|_| SessionPromptWriteError::NoSession)?;
        let cursor = request
            .cursor
            .as_deref()
            .map(decode_output_cursor)
            .transpose()?;
        let binding = if cursor.is_some() {
            None
        } else {
            self.binding(&request.binding_key)?
        };
        let requested_pid = request
            .prodex_pid
            .or_else(|| cursor.as_ref().map(|cursor| cursor.prodex_pid))
            .or_else(|| binding.as_ref().map(|binding| binding.target.prodex.pid));
        let target = match self.resolve_target_for_request(
            &workspace_root,
            requested_pid,
            request.thread_id.as_deref(),
            cursor.is_some() || binding.is_some(),
        ) {
            Err(SessionPromptWriteError::NoSession) if cursor.is_some() => {
                return Err(SessionPromptWriteError::StaleCursor);
            }
            Err(SessionPromptWriteError::NoSession)
                if cursor.is_some()
                    || binding.is_some()
                    || request.prodex_pid.is_some()
                    || request.thread_id.is_some() =>
            {
                return Err(SessionPromptWriteError::StaleTarget);
            }
            result => result,
        }?;
        let mut target = target;
        self.verify_binding(binding.as_ref(), &target)?;
        if request
            .thread_id
            .as_deref()
            .is_some_and(|thread_id| thread_id != target.thread_id)
        {
            return Err(SessionPromptWriteError::StaleTarget);
        }
        let output_path = self.output_source(&target)?;
        let Some(prodex_birth) = target.prodex.birth_identity.clone() else {
            return Err(SessionPromptWriteError::OutputSourceUnavailable);
        };
        let Some(codex_birth) = target.writer.birth_identity.clone() else {
            return Err(SessionPromptWriteError::OutputSourceUnavailable);
        };
        let source_id = output_source_id(&output_path, &target.thread_id)?;
        if binding
            .as_ref()
            .and_then(|binding| binding.source_id.as_deref())
            .is_some_and(|bound_source| bound_source != source_id)
        {
            return Err(SessionPromptWriteError::OutputSourceChanged);
        }
        let checkpoint_id = cursor
            .as_ref()
            .map(|cursor| {
                source_checkpoint_id(&output_path, cursor.offset)
                    .map(|current| current == cursor.checkpoint_id)
            })
            .transpose()
            .map_err(|_| SessionPromptWriteError::OutputSourceChanged)?;
        if checkpoint_id == Some(false) {
            return Err(SessionPromptWriteError::OutputSourceChanged);
        }
        let (mut offset, mut event_index) = match cursor.as_ref() {
            Some(cursor) if cursor.matches(&target, &source_id) => {
                (cursor.offset, cursor.event_index)
            }
            Some(_) => return Err(SessionPromptWriteError::StaleCursor),
            None => (0, 0),
        };
        let wait = Duration::from_millis(request.wait_ms).min(OUTPUT_READ_MAX_WAIT);
        let deadline = Instant::now() + wait;
        loop {
            if request
                .shutdown
                .as_ref()
                .is_some_and(|shutdown| shutdown.load(Ordering::SeqCst))
            {
                return Err(SessionPromptWriteError::OutputReadFailed);
            }
            let read = read_output_events(&output_path, offset, event_index, request.limit)?;
            offset = read.next_offset;
            event_index = read.next_event_index;
            if !read.events.is_empty() || wait.is_zero() || Instant::now() >= deadline {
                let result = PromptOutputReadSuccess {
                    prodex_pid: target.prodex.pid,
                    codex_pid: target.writer.pid,
                    thread_id: target.thread_id.clone(),
                    source: "codex_rollout",
                    events: read.events,
                    next_cursor: encode_output_cursor(OutputCursor {
                        version: OUTPUT_CURSOR_VERSION,
                        prodex_pid: target.prodex.pid,
                        prodex_birth,
                        codex_pid: target.writer.pid,
                        codex_birth,
                        thread_id: target.thread_id.clone(),
                        source_id: source_id.clone(),
                        offset,
                        event_index,
                        checkpoint_id: source_checkpoint_id(&output_path, offset)?,
                    })?,
                    has_more: read.has_more,
                };
                if cursor.is_none() {
                    self.remember_binding(
                        &request.binding_key,
                        target.clone(),
                        Some(source_id.clone()),
                    )?;
                }
                return Ok(result);
            }
            thread::sleep(
                OUTPUT_READ_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
            );
            target = self.revalidate(&target, &workspace_root)?;
            let current_path = self.output_source(&target)?;
            let current_source_id = output_source_id(&current_path, &target.thread_id)?;
            if current_source_id != source_id || current_path != output_path {
                return Err(SessionPromptWriteError::OutputSourceChanged);
            }
        }
    }
}

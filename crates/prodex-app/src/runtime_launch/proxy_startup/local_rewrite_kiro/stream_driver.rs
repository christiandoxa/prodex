use super::super::provider_sse_events::runtime_provider_sse_event;
use super::stream::{RuntimeKiroStreamingActivityState, runtime_kiro_stream_notification};
use crate::runtime_kiro_acp::{
    RuntimeKiroAcpEnvelope, RuntimeKiroAcpInitializeResult, RuntimeKiroAcpNewSessionResult,
    runtime_kiro_acp_reject_unsupported_server_request, runtime_kiro_acp_session_prompt_request,
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{self, Cursor, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) struct RuntimeKiroStreamingState {
    pub(super) initialize: Option<RuntimeKiroAcpInitializeResult>,
    pub(super) session: Option<RuntimeKiroAcpNewSessionResult>,
    pub(super) prompt_response: Option<RuntimeKiroAcpEnvelope>,
    pub(super) notifications: Vec<RuntimeKiroAcpEnvelope>,
    pub(super) response_id: String,
    pub(super) created_at: u64,
    pub(super) sequence_number: u64,
    pub(super) message_item_id: String,
    pub(super) message_item_open: bool,
    pub(super) assistant_text: String,
    pub(super) tool_activities: RuntimeKiroStreamingActivityState,
    pub(super) prompt_sent: bool,
    pub(super) chat_delta_started: bool,
    pub(super) chat_completion_id: String,
    pub(super) stream_model: String,
    pub(super) cancelled: Arc<AtomicBool>,
}

impl RuntimeKiroStreamingState {
    pub(super) fn new(request_id: u64, requested_model: Option<&str>) -> Self {
        let response_id = format!("resp_kiro_{request_id}");
        Self {
            initialize: None,
            session: None,
            prompt_response: None,
            notifications: Vec::new(),
            response_id: response_id.clone(),
            created_at: runtime_kiro_created_at(),
            sequence_number: 0,
            message_item_id: format!("msg_kiro_{request_id}_0"),
            message_item_open: false,
            assistant_text: String::new(),
            tool_activities: RuntimeKiroStreamingActivityState::default(),
            prompt_sent: false,
            chat_delta_started: false,
            chat_completion_id: format!("chatcmpl_{response_id}"),
            stream_model: requested_model
                .filter(|model| !model.is_empty())
                .unwrap_or("kiro-cli")
                .to_string(),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub(super) fn runtime_kiro_receive_stream(
    child: &mut std::process::Child,
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    stdin: &mut impl Write,
    lines: Receiver<io::Result<String>>,
    prompt: &str,
    state: &mut RuntimeKiroStreamingState,
    chat_completions_route: bool,
) -> Result<()> {
    loop {
        let Some(line) = runtime_kiro_next_stream_line(child, sender, &lines, &state.cancelled)?
        else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        runtime_kiro_process_stream_envelope(
            sender,
            stdin,
            RuntimeKiroAcpEnvelope::parse(line.trim())?,
            prompt,
            state,
            chat_completions_route,
        )?;
        if state.prompt_response.is_some() {
            break;
        }
    }
    Ok(())
}

pub(super) fn runtime_kiro_next_stream_line(
    child: &mut std::process::Child,
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    lines: &Receiver<io::Result<String>>,
    cancelled: &AtomicBool,
) -> Result<Option<String>> {
    let mut last_heartbeat = std::time::Instant::now();
    loop {
        if cancelled.load(Ordering::Acquire) {
            anyhow::bail!("Kiro ACP stream consumer disconnected");
        }
        if child.try_wait()?.is_some() {
            return Ok(None);
        }
        match lines.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(line)) => return Ok(Some(line)),
            Ok(Err(error)) => return Err(error).context("failed to read Kiro ACP stdout"),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if last_heartbeat.elapsed() >= Duration::from_secs(1) {
                    let _ = sender.try_send(RuntimeKiroStreamingChunk::Heartbeat);
                    last_heartbeat = std::time::Instant::now();
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
        }
    }
}

fn runtime_kiro_process_stream_envelope(
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    stdin: &mut impl Write,
    envelope: RuntimeKiroAcpEnvelope,
    prompt: &str,
    state: &mut RuntimeKiroStreamingState,
    chat_completions_route: bool,
) -> Result<()> {
    if runtime_kiro_acp_reject_unsupported_server_request(stdin, &envelope)? {
        state.notifications.push(envelope);
        return Ok(());
    }
    if !state.prompt_sent && matches!(envelope.numeric_id(), Some(1)) && envelope.error.is_none() {
        return runtime_kiro_send_stream_prompt(
            sender,
            stdin,
            envelope.parse_session_new_result()?,
            prompt,
            state,
            chat_completions_route,
        );
    }
    match envelope.numeric_id() {
        Some(0) if envelope.error.is_none() => {
            state.initialize = Some(envelope.parse_initialize_result()?);
        }
        Some(1) if envelope.error.is_none() => {
            state.session = Some(envelope.parse_session_new_result()?);
        }
        Some(2) => state.prompt_response = Some(envelope),
        _ => {
            if let Ok(notification) = envelope.parse_session_notification() {
                runtime_kiro_stream_notification(
                    sender,
                    &notification,
                    &state.response_id,
                    &state.chat_completion_id,
                    &state.stream_model,
                    state.created_at,
                    &state.message_item_id,
                    &mut state.sequence_number,
                    &mut state.message_item_open,
                    &mut state.assistant_text,
                    &mut state.tool_activities,
                    chat_completions_route,
                    &mut state.chat_delta_started,
                )?;
            }
            state.notifications.push(envelope);
        }
    }
    Ok(())
}

fn runtime_kiro_send_stream_prompt(
    sender: &SyncSender<RuntimeKiroStreamingChunk>,
    stdin: &mut impl Write,
    session: RuntimeKiroAcpNewSessionResult,
    prompt: &str,
    state: &mut RuntimeKiroStreamingState,
    chat_completions_route: bool,
) -> Result<()> {
    writeln!(
        stdin,
        "{}",
        runtime_kiro_acp_session_prompt_request(2, &session.session_id, prompt)
    )
    .context("failed to write Kiro ACP session/prompt request")?;
    stdin
        .flush()
        .context("failed to flush Kiro ACP session/prompt request")?;
    state.prompt_sent = true;
    state.session = Some(session);
    if chat_completions_route {
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_kiro_chat_completion_chunk(
                &state.chat_completion_id,
                Some(&state.stream_model),
                prodex_provider_core::kiro_provider_core_chat_completion_role_delta(),
                None,
            )?,
        ))?;
        state.chat_delta_started = true;
    } else {
        sender.send(RuntimeKiroStreamingChunk::Data(
            runtime_provider_sse_event(
                "response.created",
                prodex_provider_core::kiro_provider_core_response_created_event(
                    state.sequence_number,
                    state.created_at,
                    &state.response_id,
                ),
            )
            .into_bytes(),
        ))?;
    }
    Ok(())
}

pub(super) fn runtime_kiro_chat_completion_chunk(
    chat_completion_id: &str,
    model: Option<&str>,
    delta: Value,
    finish_reason: Option<&str>,
) -> Result<Vec<u8>> {
    prodex_provider_core::kiro_provider_core_chat_completion_chunk(
        chat_completion_id,
        model,
        delta,
        finish_reason,
    )
    .context("failed to serialize Kiro chat completion chunk")
}

fn runtime_kiro_created_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub(super) enum RuntimeKiroStreamingChunk {
    Data(Vec<u8>),
    Heartbeat,
    Error(io::Error),
    End,
}

pub(super) struct RuntimeKiroStreamingReader {
    pub(super) receiver: Receiver<RuntimeKiroStreamingChunk>,
    pub(super) pending: Cursor<Vec<u8>>,
    pub(super) finished: bool,
    pub(super) cancelled: Arc<AtomicBool>,
}

impl Drop for RuntimeKiroStreamingReader {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl Read for RuntimeKiroStreamingReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let read = self.pending.read(buf)?;
            if read > 0 {
                return Ok(read);
            }
            if self.finished {
                return Ok(0);
            }
            match self.receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(RuntimeKiroStreamingChunk::Data(bytes)) => {
                    self.pending = Cursor::new(bytes);
                }
                Ok(RuntimeKiroStreamingChunk::Heartbeat) => {}
                Ok(RuntimeKiroStreamingChunk::Error(err)) => return Err(err),
                Ok(RuntimeKiroStreamingChunk::End) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.finished = true;
                    return Ok(0);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
            }
        }
    }
}

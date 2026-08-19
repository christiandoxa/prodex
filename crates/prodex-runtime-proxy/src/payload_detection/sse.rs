use super::{
    RuntimeTokenUsage, extract_runtime_proxy_previous_response_message_from_value,
    extract_runtime_response_ids_from_value, extract_runtime_token_usage_from_value,
    extract_runtime_turn_state_from_value, runtime_proxy_value_is_invalid_previous_response_id,
    runtime_response_event_type_from_value,
};
use crate::{
    RuntimeHttpErrorAction, RuntimeHttpErrorPhase, runtime_stream_error_policy_from_value,
};

const RUNTIME_SSE_INVALID_DATA_MARKER: &str = "\u{0}prodex-invalid-sse-data";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeParsedSseEvent {
    pub quota_blocked: bool,
    pub overloaded: bool,
    pub previous_response_not_found: bool,
    pub invalid_previous_response_id: bool,
    pub response_ids: Vec<String>,
    pub event_type: Option<String>,
    pub turn_state: Option<String>,
    pub token_usage: Option<RuntimeTokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSseInspectionProgress {
    Hold {
        response_ids: Vec<String>,
        turn_state: Option<String>,
    },
    Commit {
        response_ids: Vec<String>,
        turn_state: Option<String>,
    },
    QuotaBlocked,
    Overloaded,
    PreviousResponseNotFound,
}

pub fn runtime_sse_trimmed_line_bytes(line: &[u8]) -> &[u8] {
    let mut end = line.len();
    while end > 0 && matches!(line.get(end - 1), Some(b'\r' | b'\n')) {
        end -= 1;
    }
    &line[..end]
}

fn runtime_sse_event_marked_invalid(data_lines: &[String]) -> bool {
    matches!(
        data_lines.first().map(String::as_str),
        Some(RUNTIME_SSE_INVALID_DATA_MARKER)
    )
}

fn runtime_sse_mark_invalid(data_lines: &mut Vec<String>) {
    data_lines.clear();
    data_lines.push(RUNTIME_SSE_INVALID_DATA_MARKER.to_string());
}

fn runtime_sse_split_field(line: &[u8]) -> (&[u8], Option<&[u8]>) {
    let Some(separator) = line.iter().position(|byte| *byte == b':') else {
        return (line, None);
    };

    let mut value = &line[separator + 1..];
    if value.first() == Some(&b' ') {
        value = &value[1..];
    }
    (&line[..separator], Some(value))
}

type RuntimeSseEventParser = fn(&[String]) -> RuntimeParsedSseEvent;

fn runtime_sse_emit_event<F>(
    data_lines: &mut Vec<String>,
    parse_event: RuntimeSseEventParser,
    on_event: &mut F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    if data_lines.is_empty() {
        return;
    }
    if runtime_sse_event_marked_invalid(data_lines) {
        data_lines.clear();
        return;
    }
    on_event(parse_event(data_lines));
    data_lines.clear();
}

fn runtime_sse_finish_line<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    parse_event: RuntimeSseEventParser,
    on_event: &mut F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    let trimmed = runtime_sse_trimmed_line_bytes(line);
    if trimmed.is_empty() {
        runtime_sse_emit_event(data_lines, parse_event, on_event);
        line.clear();
        return;
    }

    if trimmed.starts_with(b":") {
        line.clear();
        return;
    }

    let (field, value) = runtime_sse_split_field(trimmed);
    if field == b"data" {
        match value {
            Some(bytes) => match std::str::from_utf8(bytes) {
                Ok(text) => {
                    if !runtime_sse_event_marked_invalid(data_lines) {
                        data_lines.push(text.to_owned());
                    }
                }
                Err(_) => runtime_sse_mark_invalid(data_lines),
            },
            None => {
                if !runtime_sse_event_marked_invalid(data_lines) {
                    data_lines.push(String::new());
                }
            }
        }
    }
    line.clear();
}

pub fn runtime_sse_consume_chunk<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    chunk: &[u8],
    mut on_event: F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    for byte in chunk {
        line.push(*byte);
        if *byte == b'\n' {
            runtime_sse_finish_line(line, data_lines, parse_runtime_sse_event, &mut on_event);
        }
    }
}

pub fn runtime_sse_finish_pending<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    mut on_event: F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    runtime_sse_finish_pending_with_parser(
        line,
        data_lines,
        parse_runtime_sse_event,
        &mut on_event,
    );
}

fn runtime_sse_finish_pending_with_parser<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    parse_event: RuntimeSseEventParser,
    on_event: &mut F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    if !line.is_empty() {
        runtime_sse_finish_line(line, data_lines, parse_event, on_event);
    }
    runtime_sse_emit_event(data_lines, parse_event, on_event);
}

fn runtime_sse_consume_inspection_buffer<F>(
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    chunk: &[u8],
    mut on_event: F,
) where
    F: FnMut(RuntimeParsedSseEvent),
{
    for byte in chunk {
        line.push(*byte);
        if *byte == b'\n' {
            runtime_sse_finish_line(
                line,
                data_lines,
                runtime_sse_inspection_event,
                &mut on_event,
            );
        }
    }
}

pub fn inspect_runtime_sse_buffer(buffered: &[u8]) -> RuntimeSseInspectionProgress {
    inspect_runtime_sse_buffer_with_eof(buffered, false)
}

pub fn inspect_runtime_sse_buffer_at_eof(buffered: &[u8]) -> RuntimeSseInspectionProgress {
    inspect_runtime_sse_buffer_with_eof(buffered, true)
}

fn inspect_runtime_sse_buffer_with_eof(
    buffered: &[u8],
    at_eof: bool,
) -> RuntimeSseInspectionProgress {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut state = RuntimeSseInspectionState::default();
    let mut terminal = None;
    runtime_sse_consume_inspection_buffer(&mut line, &mut data_lines, buffered, |event| {
        record_runtime_sse_inspection_event(&mut state, &mut terminal, event);
    });
    if at_eof && terminal.is_none() {
        runtime_sse_finish_pending_with_parser(
            &mut line,
            &mut data_lines,
            runtime_sse_inspection_event,
            &mut |event| {
                record_runtime_sse_inspection_event(&mut state, &mut terminal, event);
            },
        );
    }
    terminal.unwrap_or_else(|| state.progress())
}

#[derive(Default)]
struct RuntimeSseInspectionState {
    response_ids: std::collections::BTreeSet<String>,
    saw_commit_ready_event: bool,
    turn_state: Option<String>,
}

impl RuntimeSseInspectionState {
    fn observe(&mut self, event: RuntimeParsedSseEvent) -> Option<RuntimeSseInspectionProgress> {
        if event.quota_blocked {
            return Some(RuntimeSseInspectionProgress::QuotaBlocked);
        }
        if event.overloaded {
            return Some(RuntimeSseInspectionProgress::Overloaded);
        }
        if event.previous_response_not_found {
            return Some(RuntimeSseInspectionProgress::PreviousResponseNotFound);
        }
        self.response_ids.extend(event.response_ids);
        if event.turn_state.is_some() {
            self.turn_state = event.turn_state;
        }
        if !event
            .event_type
            .as_deref()
            .is_some_and(crate::runtime_proxy_precommit_hold_event_kind)
        {
            self.saw_commit_ready_event = true;
        }
        None
    }

    fn progress(self) -> RuntimeSseInspectionProgress {
        if self.saw_commit_ready_event {
            RuntimeSseInspectionProgress::Commit {
                response_ids: self.response_ids.into_iter().collect(),
                turn_state: self.turn_state,
            }
        } else {
            RuntimeSseInspectionProgress::Hold {
                response_ids: self.response_ids.into_iter().collect(),
                turn_state: self.turn_state,
            }
        }
    }
}

fn record_runtime_sse_inspection_event(
    state: &mut RuntimeSseInspectionState,
    terminal: &mut Option<RuntimeSseInspectionProgress>,
    event: RuntimeParsedSseEvent,
) {
    if terminal.is_none() {
        *terminal = state.observe(event);
    }
}

fn runtime_sse_inspection_event(data_lines: &[String]) -> RuntimeParsedSseEvent {
    parse_runtime_sse_event(data_lines)
}

pub fn parse_runtime_sse_payload(data_lines: &[String]) -> Option<serde_json::Value> {
    if data_lines.is_empty() || runtime_sse_event_marked_invalid(data_lines) {
        return None;
    }

    let payload = data_lines.join("\n");
    let payload = payload.trim_start_matches('\u{feff}');
    serde_json::from_str::<serde_json::Value>(payload).ok()
}

pub fn parse_runtime_sse_event(data_lines: &[String]) -> RuntimeParsedSseEvent {
    let Some(value) = parse_runtime_sse_payload(data_lines) else {
        return RuntimeParsedSseEvent::default();
    };

    let error_policy =
        runtime_stream_error_policy_from_value(&value, RuntimeHttpErrorPhase::PreCommit);
    RuntimeParsedSseEvent {
        quota_blocked: error_policy.action == RuntimeHttpErrorAction::RotateProfile,
        overloaded: error_policy.action == RuntimeHttpErrorAction::RetryProfile,
        previous_response_not_found: extract_runtime_proxy_previous_response_message_from_value(
            &value,
        )
        .is_some(),
        invalid_previous_response_id: runtime_proxy_value_is_invalid_previous_response_id(&value),
        response_ids: extract_runtime_response_ids_from_value(&value),
        event_type: runtime_response_event_type_from_value(&value),
        turn_state: extract_runtime_turn_state_from_value(&value),
        token_usage: extract_runtime_token_usage_from_value(&value),
    }
}

/// Detects the exact invalid incremental-response error in an SSE payload.
///
/// The ordinary body classifier intentionally parses JSON bodies only. Responses failures can
/// also arrive as `data:` SSE events, where treating this as the older generic
/// `previous_response_not_found` signal would incorrectly re-enter profile rotation.
pub fn runtime_sse_body_is_invalid_previous_response_id(body: &[u8]) -> bool {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut invalid = false;
    runtime_sse_consume_chunk(&mut line, &mut data_lines, body, |event| {
        invalid |= event.invalid_previous_response_id;
    });
    runtime_sse_finish_pending(&mut line, &mut data_lines, |event| {
        invalid |= event.invalid_previous_response_id;
    });
    invalid
}

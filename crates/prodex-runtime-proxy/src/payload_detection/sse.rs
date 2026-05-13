use super::{
    RuntimeTokenUsage, extract_runtime_proxy_previous_response_message_from_value,
    extract_runtime_proxy_quota_message_from_value, extract_runtime_response_ids_from_value,
    extract_runtime_token_usage_from_value, extract_runtime_turn_state_from_value,
    runtime_response_event_type_from_value,
};

const RUNTIME_SSE_INVALID_DATA_MARKER: &str = "\u{0}prodex-invalid-sse-data";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeParsedSseEvent {
    pub quota_blocked: bool,
    pub previous_response_not_found: bool,
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

fn runtime_sse_emit_event<F>(data_lines: &mut Vec<String>, on_event: &mut F)
where
    F: FnMut(RuntimeParsedSseEvent),
{
    if data_lines.is_empty() {
        return;
    }
    if runtime_sse_event_marked_invalid(data_lines) {
        data_lines.clear();
        return;
    }
    on_event(parse_runtime_sse_event(data_lines));
    data_lines.clear();
}

fn runtime_sse_finish_line<F>(line: &mut Vec<u8>, data_lines: &mut Vec<String>, on_event: &mut F)
where
    F: FnMut(RuntimeParsedSseEvent),
{
    let trimmed = runtime_sse_trimmed_line_bytes(line);
    if trimmed.is_empty() {
        runtime_sse_emit_event(data_lines, on_event);
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
            runtime_sse_finish_line(line, data_lines, &mut on_event);
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
    if !line.is_empty() {
        runtime_sse_finish_line(line, data_lines, &mut on_event);
    }
    runtime_sse_emit_event(data_lines, &mut on_event);
}

pub fn inspect_runtime_sse_buffer(buffered: &[u8]) -> RuntimeSseInspectionProgress {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut response_ids = std::collections::BTreeSet::new();
    let mut saw_commit_ready_event = false;
    let mut turn_state = None::<String>;
    let mut process_event = |event: RuntimeParsedSseEvent| {
        if event.quota_blocked {
            return Some(RuntimeSseInspectionProgress::QuotaBlocked);
        }
        if event.previous_response_not_found {
            return Some(RuntimeSseInspectionProgress::PreviousResponseNotFound);
        }
        response_ids.extend(event.response_ids);
        if event.turn_state.is_some() {
            turn_state = event.turn_state;
        }
        if !event
            .event_type
            .as_deref()
            .is_some_and(crate::runtime_proxy_precommit_hold_event_kind)
        {
            saw_commit_ready_event = true;
        }
        None
    };
    let mut terminal = None;
    runtime_sse_consume_chunk(&mut line, &mut data_lines, buffered, |event| {
        if terminal.is_none() {
            terminal = process_event(event);
        }
    });
    if terminal.is_none() {
        runtime_sse_finish_pending(&mut line, &mut data_lines, |event| {
            if terminal.is_none() {
                terminal = process_event(event);
            }
        });
    }
    if let Some(progress) = terminal {
        return progress;
    }

    if saw_commit_ready_event {
        RuntimeSseInspectionProgress::Commit {
            response_ids: response_ids.into_iter().collect(),
            turn_state,
        }
    } else {
        RuntimeSseInspectionProgress::Hold {
            response_ids: response_ids.into_iter().collect(),
            turn_state,
        }
    }
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

    RuntimeParsedSseEvent {
        quota_blocked: extract_runtime_proxy_quota_message_from_value(&value).is_some(),
        previous_response_not_found: extract_runtime_proxy_previous_response_message_from_value(
            &value,
        )
        .is_some(),
        response_ids: extract_runtime_response_ids_from_value(&value),
        event_type: runtime_response_event_type_from_value(&value),
        turn_state: extract_runtime_turn_state_from_value(&value),
        token_usage: extract_runtime_token_usage_from_value(&value),
    }
}

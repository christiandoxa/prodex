use crate::{
    RuntimeHttpErrorClass, RuntimeHttpErrorPhase, runtime_error_signal_message_from_text,
    runtime_error_signal_message_from_value, runtime_http_error_policy,
    runtime_overload_text_message, runtime_usage_limit_text_message,
};

const RUNTIME_PROXY_JSON_SCAN_LIMIT: usize = 2_048;
const RUNTIME_SSE_INVALID_DATA_MARKER: &str = "\u{0}prodex-invalid-sse-data";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeTokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

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
pub enum RuntimeWebsocketErrorPayload {
    Text(String),
    Binary(Vec<u8>),
    Empty,
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

fn runtime_proxy_utf8_text(body: &[u8]) -> Option<&str> {
    std::str::from_utf8(body).ok().map(str::trim)
}

fn runtime_json_find<T, F>(root: &serde_json::Value, mut candidate: F) -> Option<T>
where
    F: FnMut(&serde_json::Value) -> Option<T>,
{
    let mut stack = vec![root];
    let mut visited = 0usize;

    while let Some(value) = stack.pop() {
        if let Some(result) = candidate(value) {
            return Some(result);
        }

        visited += 1;
        if visited >= RUNTIME_PROXY_JSON_SCAN_LIMIT {
            break;
        }

        match value {
            serde_json::Value::Array(values) => stack.extend(values.iter().rev()),
            serde_json::Value::Object(map) => stack.extend(map.values().rev()),
            _ => {}
        }
    }

    None
}

pub fn extract_runtime_proxy_quota_message(body: &[u8]) -> Option<String> {
    let policy = runtime_http_error_policy(429, body, RuntimeHttpErrorPhase::PreCommit);
    (policy.class == RuntimeHttpErrorClass::Quota)
        .then_some(policy.message)
        .flatten()
}

pub fn extract_runtime_proxy_quota_message_from_websocket_payload(
    payload: &RuntimeWebsocketErrorPayload,
) -> Option<String> {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => {
            extract_runtime_proxy_quota_message_from_text(text)
        }
        RuntimeWebsocketErrorPayload::Binary(bytes) => extract_runtime_proxy_quota_message(bytes),
        RuntimeWebsocketErrorPayload::Empty => None,
    }
}

pub fn extract_runtime_proxy_overload_message_from_websocket_payload(
    payload: &RuntimeWebsocketErrorPayload,
) -> Option<String> {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(text)
                && let Some(message) = extract_runtime_proxy_overload_message_from_value(&value)
            {
                return Some(message);
            }
            extract_runtime_proxy_overload_message_from_text(text)
        }
        RuntimeWebsocketErrorPayload::Binary(bytes) => {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes)
                && let Some(message) = extract_runtime_proxy_overload_message_from_value(&value)
            {
                return Some(message);
            }
            runtime_proxy_utf8_text(bytes)
                .and_then(extract_runtime_proxy_overload_message_from_text)
        }
        RuntimeWebsocketErrorPayload::Empty => None,
    }
}

pub fn extract_runtime_proxy_previous_response_message(body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_previous_response_message_from_value(&value)
    {
        return Some(message);
    }

    runtime_proxy_utf8_text(body)
        .and_then(extract_runtime_proxy_previous_response_message_from_text)
}

pub fn extract_runtime_proxy_overload_message(status: u16, body: &[u8]) -> Option<String> {
    let policy = runtime_http_error_policy(status, body, RuntimeHttpErrorPhase::PreCommit);
    matches!(
        policy.class,
        RuntimeHttpErrorClass::Overload | RuntimeHttpErrorClass::TransientServer
    )
    .then_some(policy.message)
    .flatten()
}

pub fn extract_runtime_proxy_overload_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    runtime_error_signal_message_from_value(value, RuntimeHttpErrorClass::Overload)
}

pub fn extract_runtime_proxy_overload_message_from_text(text: &str) -> Option<String> {
    runtime_error_signal_message_from_text(text, RuntimeHttpErrorClass::Overload)
}

pub fn extract_runtime_proxy_quota_message_from_value(value: &serde_json::Value) -> Option<String> {
    runtime_error_signal_message_from_value(value, RuntimeHttpErrorClass::Quota)
}

pub fn extract_runtime_proxy_quota_message_candidate(value: &serde_json::Value) -> Option<String> {
    runtime_error_signal_message_from_value(value, RuntimeHttpErrorClass::Quota)
}

pub fn extract_runtime_proxy_quota_message_from_text(text: &str) -> Option<String> {
    runtime_error_signal_message_from_text(text, RuntimeHttpErrorClass::Quota)
}

pub fn runtime_proxy_usage_limit_message(message: &str) -> bool {
    runtime_usage_limit_text_message(message)
}

pub fn runtime_proxy_overload_message(message: &str) -> bool {
    runtime_overload_text_message(message)
}

pub fn runtime_proxy_body_snippet(body: &[u8], max_chars: usize) -> String {
    let normalized = String::from_utf8_lossy(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return "-".to_string();
    }

    let snippet = normalized.chars().take(max_chars).collect::<String>();
    if normalized.chars().count() > max_chars {
        format!("{snippet}...")
    } else {
        snippet
    }
}

pub fn extract_runtime_proxy_previous_response_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    runtime_json_find(
        value,
        extract_runtime_proxy_previous_response_message_candidate,
    )
}

fn extract_runtime_proxy_previous_response_message_candidate(
    value: &serde_json::Value,
) -> Option<String> {
    match value {
        serde_json::Value::String(message) => {
            (runtime_proxy_previous_response_missing_text_signature(message)
                || runtime_proxy_tool_context_missing_text_signature(message))
            .then(|| message.to_string())
        }
        serde_json::Value::Object(map) => {
            let message = map
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| map.get("detail").and_then(serde_json::Value::as_str))
                .or_else(|| map.get("error").and_then(serde_json::Value::as_str));
            let code = map.get("code").and_then(serde_json::Value::as_str);
            let error_type = map.get("type").and_then(serde_json::Value::as_str);
            let param = map.get("param").and_then(serde_json::Value::as_str);

            if code == Some("previous_response_not_found")
                || message.is_some_and(runtime_proxy_previous_response_missing_message)
            {
                return Some(
                    message
                        .unwrap_or(
                            "Previous response could not be found on the selected Codex account.",
                        )
                        .to_string(),
                );
            }

            if let Some(message) = message
                && runtime_proxy_tool_context_missing_message(message)
                && (matches!(error_type, Some("invalid_request_error"))
                    || matches!(param, Some("input")))
            {
                return Some(message.to_string());
            }

            None
        }
        _ => None,
    }
}

pub fn extract_runtime_proxy_previous_response_message_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if runtime_proxy_previous_response_missing_text_signature(trimmed)
        || runtime_proxy_tool_context_missing_text_signature(trimmed)
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn runtime_proxy_previous_response_missing_text_signature(message: &str) -> bool {
    let lower = message.trim().to_ascii_lowercase();
    lower.starts_with("previous_response_not_found")
        || (lower.starts_with("previous response") && lower.contains("not found"))
}

fn runtime_proxy_previous_response_missing_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("previous_response_not_found")
        || (lower.contains("previous response") && lower.contains("not found"))
}

fn runtime_proxy_tool_context_missing_text_signature(message: &str) -> bool {
    let lower = message.trim().to_ascii_lowercase();
    (lower.starts_with("invalid_request_error:")
        || lower.starts_with("no tool call found")
        || lower.starts_with("no function call found"))
        && runtime_proxy_tool_context_missing_message(message)
}

pub fn runtime_proxy_tool_context_missing_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("no tool call found") || lower.contains("no function call found")
}

#[cfg(test)]
pub fn extract_runtime_response_ids_from_payload(payload: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub fn extract_runtime_response_ids_from_body_bytes(body: &[u8]) -> Vec<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub fn extract_runtime_turn_state_from_body_bytes(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_turn_state_from_value(&value))
}

pub fn extract_runtime_token_usage_from_body_bytes(body: &[u8]) -> Option<RuntimeTokenUsage> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_token_usage_from_value(&value))
}

pub fn push_runtime_response_id(response_ids: &mut Vec<String>, id: Option<&str>) {
    if let Some(id) = id
        && !response_ids.iter().any(|existing| existing == id)
    {
        response_ids.push(id.to_string());
    }
}

pub fn extract_runtime_response_ids_from_value(value: &serde_json::Value) -> Vec<String> {
    let mut response_ids = Vec::new();

    push_runtime_response_id(
        &mut response_ids,
        value
            .get("response")
            .and_then(|response| response.get("id"))
            .and_then(serde_json::Value::as_str),
    );
    push_runtime_response_id(
        &mut response_ids,
        value.get("response_id").and_then(serde_json::Value::as_str),
    );

    if value
        .get("object")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|object| object == "response" || object.ends_with(".response"))
    {
        push_runtime_response_id(
            &mut response_ids,
            value.get("id").and_then(serde_json::Value::as_str),
        );
    }

    response_ids
}

pub fn extract_runtime_turn_state_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("response")
        .and_then(|response| response.get("headers"))
        .and_then(extract_runtime_turn_state_from_headers_value)
        .or_else(|| {
            value
                .get("headers")
                .and_then(extract_runtime_turn_state_from_headers_value)
        })
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("turn_state"))
                .and_then(runtime_json_string)
        })
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("turnState"))
                .and_then(runtime_json_string)
        })
        .or_else(|| value.get("turn_state").and_then(runtime_json_string))
        .or_else(|| value.get("turnState").and_then(runtime_json_string))
}

pub fn extract_runtime_token_usage_from_value(
    value: &serde_json::Value,
) -> Option<RuntimeTokenUsage> {
    runtime_json_find(value, extract_runtime_token_usage_candidate)
}

fn extract_runtime_token_usage_candidate(value: &serde_json::Value) -> Option<RuntimeTokenUsage> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(usage) = map.get("usage")
                && let Some(token_usage) = runtime_token_usage_from_usage_value(usage)
            {
                return Some(token_usage);
            }
            runtime_token_usage_from_usage_value(value)
        }
        _ => None,
    }
}

fn runtime_token_usage_from_usage_value(value: &serde_json::Value) -> Option<RuntimeTokenUsage> {
    let input_tokens = runtime_json_u64_at(value, &["input_tokens"])
        .or_else(|| runtime_json_u64_at(value, &["prompt_tokens"]));
    let cached_input_tokens = runtime_json_u64_at(value, &["cached_input_tokens"])
        .or_else(|| runtime_json_u64_at(value, &["input_tokens_details", "cached_tokens"]))
        .or_else(|| runtime_json_u64_at(value, &["input_tokens_details", "cached_input_tokens"]))
        .or_else(|| runtime_json_u64_at(value, &["prompt_tokens_details", "cached_tokens"]));
    let output_tokens = runtime_json_u64_at(value, &["output_tokens"])
        .or_else(|| runtime_json_u64_at(value, &["completion_tokens"]));
    let reasoning_tokens = runtime_json_u64_at(value, &["reasoning_tokens"])
        .or_else(|| runtime_json_u64_at(value, &["output_tokens_details", "reasoning_tokens"]))
        .or_else(|| runtime_json_u64_at(value, &["completion_tokens_details", "reasoning_tokens"]));

    if input_tokens.is_none()
        && cached_input_tokens.is_none()
        && output_tokens.is_none()
        && reasoning_tokens.is_none()
    {
        return None;
    }

    Some(RuntimeTokenUsage {
        input_tokens: input_tokens.unwrap_or_default(),
        cached_input_tokens: cached_input_tokens.unwrap_or_default(),
        output_tokens: output_tokens.unwrap_or_default(),
        reasoning_tokens: reasoning_tokens.unwrap_or_default(),
    })
}

fn runtime_json_u64_at(value: &serde_json::Value, path: &[&str]) -> Option<u64> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    runtime_json_u64(current)
}

fn runtime_json_u64(value: &serde_json::Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_i64()
            .and_then(|value| (value >= 0).then_some(value as u64))
    })
}

fn runtime_json_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn extract_runtime_turn_state_from_headers_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(headers) => headers.iter().find_map(|(name, value)| {
            if name.eq_ignore_ascii_case("x-codex-turn-state") {
                extract_runtime_turn_state_header_value(value)
            } else {
                None
            }
        }),
        serde_json::Value::Array(headers) => headers
            .iter()
            .find_map(extract_runtime_turn_state_from_header_entry),
        _ => None,
    }
}

fn extract_runtime_turn_state_from_header_entry(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(items) => {
            let name = items.first()?.as_str()?;
            if !name.eq_ignore_ascii_case("x-codex-turn-state") {
                return None;
            }
            items
                .get(1)
                .and_then(extract_runtime_turn_state_header_value)
        }
        serde_json::Value::Object(entry) => {
            let name = entry
                .get("name")
                .or_else(|| entry.get("key"))
                .and_then(serde_json::Value::as_str)?;
            if !name.eq_ignore_ascii_case("x-codex-turn-state") {
                return None;
            }
            entry
                .get("value")
                .or_else(|| entry.get("values"))
                .and_then(extract_runtime_turn_state_header_value)
        }
        _ => None,
    }
}

fn extract_runtime_turn_state_header_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(extract_runtime_turn_state_header_value),
        _ => None,
    }
}

pub fn runtime_response_event_type_from_value(value: &serde_json::Value) -> Option<String> {
    value.get("type").and_then(runtime_json_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_runtime_sse_events(chunks: &[&[u8]]) -> Vec<RuntimeParsedSseEvent> {
        let mut line = Vec::new();
        let mut data_lines = Vec::new();
        let mut events = Vec::new();

        for chunk in chunks {
            runtime_sse_consume_chunk(&mut line, &mut data_lines, chunk, |event| {
                events.push(event)
            });
        }
        runtime_sse_finish_pending(&mut line, &mut data_lines, |event| events.push(event));

        events
    }

    type RuntimeSseEventSignature = (bool, bool, Vec<String>, Option<String>, Option<String>);

    fn runtime_sse_event_signatures(
        events: &[RuntimeParsedSseEvent],
    ) -> Vec<RuntimeSseEventSignature> {
        events
            .iter()
            .map(|event| {
                (
                    event.quota_blocked,
                    event.previous_response_not_found,
                    event.response_ids.clone(),
                    event.event_type.clone(),
                    event.turn_state.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn runtime_sse_helpers_ignore_comments_and_handle_crlf_boundaries() {
        let mut line = Vec::new();
        let mut data_lines = Vec::new();
        let mut events = Vec::new();

        runtime_sse_consume_chunk(
            &mut line,
            &mut data_lines,
            concat!(
                ": keep-alive\r\n",
                "event: response.created\r\n",
                "data: {\"type\":\"response.created\",\"response_id\":\"resp-1\",\"turn_state\":\"ts-1\"}\r\n",
                "\r\n",
            )
            .as_bytes(),
            |event| events.push(event),
        );

        assert!(line.is_empty());
        assert!(data_lines.is_empty());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].response_ids, vec!["resp-1".to_string()]);
        assert_eq!(events[0].turn_state.as_deref(), Some("ts-1"));
        assert_eq!(events[0].event_type.as_deref(), Some("response.created"));
    }

    #[test]
    fn runtime_sse_helpers_flush_partial_event_at_finish() {
        let mut line = Vec::new();
        let mut data_lines = Vec::new();
        let mut events = Vec::new();

        runtime_sse_consume_chunk(
            &mut line,
            &mut data_lines,
            b"data: {\"type\":\"response.in_progress\",\"response_id\":\"resp-2\"}",
            |event| events.push(event),
        );
        assert!(events.is_empty());

        runtime_sse_finish_pending(&mut line, &mut data_lines, |event| events.push(event));

        assert!(line.is_empty());
        assert!(data_lines.is_empty());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].response_ids, vec!["resp-2".to_string()]);
        assert_eq!(
            events[0].event_type.as_deref(),
            Some("response.in_progress")
        );
    }

    #[test]
    fn runtime_sse_event_extracts_response_token_usage() {
        let events = collect_runtime_sse_events(&[concat!(
            "data: {",
            "\"type\":\"response.completed\",",
            "\"response\":{",
            "\"id\":\"resp-token\",",
            "\"usage\":{",
            "\"input_tokens\":120,",
            "\"input_tokens_details\":{\"cached_tokens\":40},",
            "\"output_tokens\":32,",
            "\"output_tokens_details\":{\"reasoning_tokens\":7}",
            "}",
            "}",
            "}\n\n"
        )
        .as_bytes()]);

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].token_usage,
            Some(RuntimeTokenUsage {
                input_tokens: 120,
                cached_input_tokens: 40,
                output_tokens: 32,
                reasoning_tokens: 7,
            })
        );
    }

    #[test]
    fn runtime_sse_helpers_preserve_events_across_split_boundaries() {
        let body = concat!(
            ": keep-alive\r\n",
            "data: {\"type\":\"response.created\",\"response_id\":\"resp-1\"}\r\n",
            "\r\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-2\",\"headers\":{\"x-codex-turn-state\":\"ts-2\"}}}\n",
            "\n",
        )
        .as_bytes();
        let expected = runtime_sse_event_signatures(&collect_runtime_sse_events(&[body]));

        for first in 0..=body.len() {
            for second in first..=body.len() {
                let actual = collect_runtime_sse_events(&[
                    &body[..first],
                    &body[first..second],
                    &body[second..],
                ]);
                assert_eq!(
                    runtime_sse_event_signatures(&actual),
                    expected,
                    "unexpected SSE parse for split {first}/{second}"
                );
            }
        }
    }

    #[test]
    fn runtime_sse_helpers_drop_invalid_utf8_event_and_recover_next_event() {
        let body = &b"data: {\"type\":\"response.created\",\"response_id\":\"resp-\xff\"}\n\n\
data: {\"type\":\"response.completed\",\"response_id\":\"resp-2\"}\n\n"[..];

        let events = collect_runtime_sse_events(&[body]);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].response_ids, vec!["resp-2".to_string()]);
        assert_eq!(events[0].event_type.as_deref(), Some("response.completed"));
    }

    #[test]
    fn previous_response_message_detects_top_level_json_shape() {
        let payload = serde_json::json!({
            "type": "invalid_request_error",
            "code": "previous_response_not_found",
            "message": "Previous response with id 'resp-123' not found.",
        });

        assert_eq!(
            extract_runtime_proxy_previous_response_message_from_value(&payload),
            Some("Previous response with id 'resp-123' not found.".to_string())
        );
    }

    #[test]
    fn previous_response_message_detects_plain_text_tool_context_signature() {
        let body = b"invalid_request_error: No tool call found for output item call_123";

        assert_eq!(
            extract_runtime_proxy_previous_response_message(body),
            Some("invalid_request_error: No tool call found for output item call_123".to_string())
        );
    }

    #[test]
    fn previous_response_message_ignores_non_error_content_text() {
        let payload = serde_json::json!({
            "type": "response.output_text.delta",
            "delta": "The docs mention previous_response_not_found and No tool call found as examples.",
        });

        assert_eq!(
            extract_runtime_proxy_previous_response_message_from_value(&payload),
            None
        );
    }

    #[test]
    fn quota_message_detects_nested_error_payloads() {
        let payload = serde_json::json!({
            "items": [
                {
                    "wrapper": {
                        "error": {
                            "code": "rate_limit_exceeded",
                            "message": "You've hit your usage limit. Try again at 8pm."
                        }
                    }
                }
            ]
        });

        assert_eq!(
            extract_runtime_proxy_quota_message_from_value(&payload),
            Some("You've hit your usage limit. Try again at 8pm.".to_string())
        );
    }

    #[test]
    fn quota_http_body_detection_requires_explicit_error_code() {
        assert_eq!(
            extract_runtime_proxy_quota_message(br#"{"error":{"message":"Too Many Requests"}}"#),
            None
        );
        assert_eq!(
            extract_runtime_proxy_quota_message(
                br#"{"error":{"message":"The usage limit has been reached"}}"#
            ),
            None
        );

        for code in ["insufficient_quota", "rate_limit_exceeded"] {
            let body = serde_json::to_vec(&serde_json::json!({
                "error": {
                    "code": code,
                    "message": "Quota exhausted"
                }
            }))
            .expect("quota body should serialize");

            assert_eq!(
                extract_runtime_proxy_quota_message(&body),
                Some("Quota exhausted".to_string()),
                "{code}"
            );
        }
    }

    #[test]
    fn overload_message_detects_top_level_and_nested_error_payloads() {
        let top_level = serde_json::json!({
            "code": "server_is_overloaded",
            "message": "Upstream Codex backend is currently overloaded.",
        });
        let nested = serde_json::json!({
            "meta": [
                {
                    "response": {
                        "error": {
                            "message": "Selected model is at capacity. Please try again."
                        }
                    }
                }
            ]
        });

        assert_eq!(
            extract_runtime_proxy_overload_message_from_value(&top_level),
            Some("Upstream Codex backend is currently overloaded.".to_string())
        );
        assert_eq!(
            extract_runtime_proxy_overload_message_from_value(&nested),
            Some("Selected model is at capacity. Please try again.".to_string())
        );
    }

    #[test]
    fn invalid_utf8_bodies_do_not_trigger_lossy_retry_classification() {
        assert_eq!(
            extract_runtime_proxy_quota_message(
                b"\xffYou've hit your usage limit. Try again at 8pm."
            ),
            None
        );
        assert_eq!(
            extract_runtime_proxy_previous_response_message(
                b"\xffprevious_response_not_found: missing"
            ),
            None
        );
        assert_eq!(
            extract_runtime_proxy_overload_message(
                503,
                b"\xffSelected model is at capacity. Please try again."
            ),
            Some("Upstream Codex backend returned transient HTTP 503.".to_string())
        );
        assert_eq!(
            extract_runtime_proxy_overload_message(500, b"\xff"),
            Some("Upstream Codex backend is currently experiencing high demand.".to_string())
        );
    }

    #[test]
    fn body_snippet_normalizes_whitespace_and_truncates() {
        assert_eq!(
            runtime_proxy_body_snippet(b" one\n two\tthree ", 7),
            "one two..."
        );
        assert_eq!(runtime_proxy_body_snippet(b"  \n\t  ", 7), "-");
    }
}

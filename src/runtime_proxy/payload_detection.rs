use super::*;

const RUNTIME_PROXY_JSON_SCAN_LIMIT: usize = 2_048;
const RUNTIME_SSE_INVALID_DATA_MARKER: &str = "\u{0}prodex-invalid-sse-data";

pub(crate) fn runtime_sse_trimmed_line_bytes(line: &[u8]) -> &[u8] {
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

pub(crate) fn runtime_sse_consume_chunk<F>(
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

pub(crate) fn runtime_sse_finish_pending<F>(
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

pub(crate) fn parse_runtime_sse_payload(data_lines: &[String]) -> Option<serde_json::Value> {
    if data_lines.is_empty() || runtime_sse_event_marked_invalid(data_lines) {
        return None;
    }

    let payload = data_lines.join("\n");
    let payload = payload.trim_start_matches('\u{feff}');
    serde_json::from_str::<serde_json::Value>(payload).ok()
}

pub(crate) fn parse_runtime_sse_event(data_lines: &[String]) -> RuntimeParsedSseEvent {
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

pub(crate) fn extract_runtime_proxy_quota_message(body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_quota_message_from_value(&value)
    {
        return Some(message);
    }

    runtime_proxy_utf8_text(body).and_then(extract_runtime_proxy_quota_message_from_text)
}

pub(crate) fn extract_runtime_proxy_quota_message_from_response_reply(
    response: &RuntimeResponsesReply,
) -> Option<String> {
    match response {
        RuntimeResponsesReply::Buffered(parts) => extract_runtime_proxy_quota_message(&parts.body),
        RuntimeResponsesReply::Streaming(_) => None,
    }
}

pub(crate) fn extract_runtime_proxy_quota_message_from_websocket_payload(
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

pub(crate) fn extract_runtime_proxy_overload_message_from_websocket_payload(
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

pub(crate) fn extract_runtime_proxy_previous_response_message(body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_previous_response_message_from_value(&value)
    {
        return Some(message);
    }

    runtime_proxy_utf8_text(body)
        .and_then(extract_runtime_proxy_previous_response_message_from_text)
}

pub(crate) fn extract_runtime_proxy_overload_message(status: u16, body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_overload_message_from_value(&value)
    {
        return Some(message);
    }

    let body_text = runtime_proxy_utf8_text(body)
        .unwrap_or_default()
        .to_string();
    if matches!(status, 429 | 500 | 502 | 503 | 504 | 529)
        && let Some(message) = extract_runtime_proxy_overload_message_from_text(&body_text)
    {
        return Some(message);
    }

    (status == 500).then(|| {
        if body_text.is_empty() {
            "Upstream Codex backend is currently experiencing high demand.".to_string()
        } else {
            body_text
        }
    })
}

pub(crate) fn extract_runtime_proxy_overload_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    runtime_json_find(value, extract_runtime_proxy_overload_message_candidate)
}

fn extract_runtime_proxy_overload_message_candidate(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(message) => {
            runtime_proxy_overload_message(message).then(|| message.to_string())
        }
        serde_json::Value::Object(map) => {
            let message = map
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| map.get("detail").and_then(serde_json::Value::as_str));
            let code = map.get("code").and_then(serde_json::Value::as_str);

            if matches!(code, Some("server_is_overloaded" | "slow_down")) {
                return Some(
                    message
                        .unwrap_or("Upstream Codex backend is currently overloaded.")
                        .to_string(),
                );
            }

            message
                .filter(|message| runtime_proxy_overload_message(message))
                .map(str::to_string)
        }
        _ => None,
    }
}

pub(crate) fn extract_runtime_proxy_overload_message_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    runtime_proxy_overload_message(trimmed).then(|| trimmed.to_string())
}

pub(crate) fn extract_runtime_proxy_quota_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    runtime_json_find(value, extract_runtime_proxy_quota_message_candidate)
}

pub(crate) fn extract_runtime_proxy_quota_message_candidate(
    value: &serde_json::Value,
) -> Option<String> {
    match value {
        serde_json::Value::String(message) => {
            runtime_proxy_usage_limit_message(message).then(|| message.to_string())
        }
        serde_json::Value::Object(map) => {
            let message = map
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| map.get("detail").and_then(serde_json::Value::as_str))
                .or_else(|| map.get("error").and_then(serde_json::Value::as_str));
            let code = map.get("code").and_then(serde_json::Value::as_str);
            let error_type = map.get("type").and_then(serde_json::Value::as_str);
            let code_matches = matches!(code, Some("insufficient_quota" | "rate_limit_exceeded"));
            let type_matches = matches!(error_type, Some("usage_limit_reached"));
            let message_matches = message.is_some_and(runtime_proxy_usage_limit_message);
            if !(code_matches || type_matches || message_matches) {
                return None;
            }

            Some(
                message
                    .unwrap_or("Upstream Codex account quota was exhausted.")
                    .to_string(),
            )
        }
        _ => None,
    }
}

pub(crate) fn extract_runtime_proxy_quota_message_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if runtime_proxy_usage_limit_message(trimmed)
        || lower.contains("usage_limit_reached")
        || lower.contains("insufficient_quota")
        || lower.contains("rate_limit_exceeded")
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

pub(crate) fn runtime_proxy_usage_limit_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("you've hit your usage limit")
        || lower.contains("you have hit your usage limit")
        || lower.contains("the usage limit has been reached")
        || lower.contains("usage limit has been reached")
        || lower.contains("usage limit")
            && (lower.contains("try again at")
                || lower.contains("request to your admin")
                || lower.contains("more access now"))
}

pub(crate) fn runtime_proxy_overload_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("selected model is at capacity")
        || (lower.contains("model is at capacity")
            && (lower.contains("try a different model") || lower.contains("please try again")))
        || lower.contains("backend under high demand")
        || lower.contains("experiencing high demand")
        || lower.contains("server is overloaded")
        || lower.contains("currently overloaded")
}

pub(crate) fn runtime_proxy_body_snippet(body: &[u8], max_chars: usize) -> String {
    redaction_text_snippet(String::from_utf8_lossy(body).as_ref(), max_chars)
}

pub(crate) fn runtime_proxy_redacted_body_snippet(body: &[u8], max_chars: usize) -> String {
    redaction_redacted_body_snippet(body, max_chars)
}

pub(crate) fn runtime_proxy_redacted_headers_debug(headers: &[(String, String)]) -> String {
    redaction_redacted_headers_debug(headers)
}

pub(crate) fn extract_runtime_proxy_previous_response_message_from_value(
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

            // Upstream regressions sometimes surface broken tool-call continuity as
            // `invalid_request_error: No tool call found ...` instead of
            // `previous_response_not_found`. Treat that as the same pre-commit
            // continuity failure so session-replayable requests can recover before
            // the user sees a spurious tool-context error.
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

fn extract_runtime_proxy_previous_response_message_from_text(text: &str) -> Option<String> {
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

pub(crate) fn runtime_proxy_tool_context_missing_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("no tool call found") || lower.contains("no function call found")
}

#[cfg(test)]
pub(crate) fn extract_runtime_response_ids_from_payload(payload: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub(crate) fn extract_runtime_response_ids_from_body_bytes(body: &[u8]) -> Vec<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub(crate) fn extract_runtime_turn_state_from_body_bytes(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_turn_state_from_value(&value))
}

pub(crate) fn push_runtime_response_id(response_ids: &mut Vec<String>, id: Option<&str>) {
    if let Some(id) = id
        && !response_ids.iter().any(|existing| existing == id)
    {
        response_ids.push(id.to_string());
    }
}

pub(crate) fn extract_runtime_response_ids_from_value(value: &serde_json::Value) -> Vec<String> {
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

pub(crate) fn extract_runtime_turn_state_from_value(value: &serde_json::Value) -> Option<String> {
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

fn runtime_json_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn extract_runtime_turn_state_from_headers_value(
    value: &serde_json::Value,
) -> Option<String> {
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

    fn fake_secret(parts: &[&str]) -> String {
        parts.concat()
    }

    fn fake_named_secret(name: &str) -> String {
        fake_secret(&["fixture_", name, "_notreal_", "12345"])
    }

    fn fake_api_key(prefix: &str, name: &str) -> String {
        fake_secret(&[prefix, "fixture-", name, "-notreal-", "123456789"])
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
    fn redacted_headers_debug_removes_sensitive_header_values() {
        let authorization_token = fake_named_secret("authorization");
        let api_key = fake_api_key("sk-ant-", "header");
        let cookie_value = fake_named_secret("cookie");
        let forwarded_token = fake_named_secret("forwarded");
        let nested_bearer_token = fake_named_secret("nested_bearer");
        let headers = vec![
            (
                "authorization".to_string(),
                format!("Bearer {authorization_token}"),
            ),
            ("x-api-key".to_string(), api_key.clone()),
            ("cookie".to_string(), format!("session={cookie_value}")),
            ("x-forwarded-token".to_string(), forwarded_token.clone()),
            (
                "x-observed-value".to_string(),
                format!("Bearer {nested_bearer_token}"),
            ),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ];

        let redacted = runtime_proxy_redacted_headers_debug(&headers);

        assert!(redacted.contains("authorization"));
        assert!(redacted.contains("Bearer <redacted>"));
        assert!(redacted.contains("anthropic-version"));
        assert!(redacted.contains("2023-06-01"));
        assert!(!redacted.contains(authorization_token.as_str()));
        assert!(!redacted.contains(api_key.as_str()));
        assert!(!redacted.contains(cookie_value.as_str()));
        assert!(!redacted.contains(forwarded_token.as_str()));
        assert!(!redacted.contains(nested_bearer_token.as_str()));
    }

    #[test]
    fn redacted_body_snippet_removes_json_secret_fields_and_token_values() {
        let api_key = fake_api_key("sk-ant-", "json");
        let access_token = fake_named_secret("access_token");
        let refresh_token = fake_named_secret("refresh_token");
        let client_secret = fake_named_secret("client_secret");
        let password = fake_named_secret("password");
        let bearer_token = fake_named_secret("body_bearer");
        let free_text_key = fake_api_key("sk-proj-", "free_text");
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "api_key": api_key.clone(),
            "auth": {
                "access_token": access_token.clone(),
                "refreshToken": refresh_token.clone(),
                "client_secret": client_secret.clone(),
                "password": password.clone()
            },
            "messages": [
                {
                    "role": "user",
                    "content": format!("Use Bearer {bearer_token} and {free_text_key}")
                }
            ]
        }))
        .expect("test body should serialize");

        let redacted = runtime_proxy_redacted_body_snippet(&body, 4096);

        assert!(redacted.contains("claude-sonnet-4-6"));
        assert!(redacted.contains("max_tokens"));
        assert!(redacted.contains("\"api_key\":\"<redacted>\""));
        assert!(redacted.contains("\"access_token\":\"<redacted>\""));
        assert!(redacted.contains("\"refreshToken\":\"<redacted>\""));
        assert!(redacted.contains("\"client_secret\":\"<redacted>\""));
        assert!(redacted.contains("\"password\":\"<redacted>\""));
        assert!(redacted.contains("Bearer <redacted>"));
        assert!(redacted.contains("sk-proj-<redacted>"));
        assert!(!redacted.contains(api_key.as_str()));
        assert!(!redacted.contains(access_token.as_str()));
        assert!(!redacted.contains(refresh_token.as_str()));
        assert!(!redacted.contains(client_secret.as_str()));
        assert!(!redacted.contains(password.as_str()));
        assert!(!redacted.contains(bearer_token.as_str()));
        assert!(!redacted.contains(free_text_key.as_str()));
    }

    #[test]
    fn redacted_body_snippet_removes_plain_text_secret_assignments() {
        let api_key = fake_named_secret("plain_api_key");
        let access_token = fake_named_secret("plain_access_token");
        let bearer_token = fake_named_secret("plain_bearer");
        let prefixed_key = fake_api_key("sk-live-", "plain");
        let body = format!(
            "api_key={api_key} access_token: {access_token} \
             Bearer {bearer_token} x={prefixed_key}"
        );

        let redacted = runtime_proxy_redacted_body_snippet(body.as_bytes(), 4096);

        assert!(redacted.contains("api_key=<redacted>"));
        assert!(redacted.contains("access_token: <redacted>"));
        assert!(redacted.contains("Bearer <redacted>"));
        assert!(redacted.contains("sk-live-<redacted>"));
        assert!(!redacted.contains(api_key.as_str()));
        assert!(!redacted.contains(access_token.as_str()));
        assert!(!redacted.contains(bearer_token.as_str()));
        assert!(!redacted.contains(prefixed_key.as_str()));
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
    fn previous_response_message_detects_plain_text_signature() {
        let body = b"previous_response_not_found: Previous response with id 'resp-123' not found.";

        assert_eq!(
            extract_runtime_proxy_previous_response_message(body),
            Some(
                "previous_response_not_found: Previous response with id 'resp-123' not found."
                    .to_string()
            )
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
    fn previous_response_message_detects_nested_tool_context_payloads() {
        let payload = serde_json::json!({
            "items": [
                {
                    "response": {
                        "error": {
                            "type": "invalid_request_error",
                            "message": "No tool call found for output item call_123"
                        }
                    }
                }
            ]
        });

        assert_eq!(
            extract_runtime_proxy_previous_response_message_from_value(&payload),
            Some("No tool call found for output item call_123".to_string())
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
            None
        );
        assert_eq!(
            extract_runtime_proxy_overload_message(500, b"\xff"),
            Some("Upstream Codex backend is currently experiencing high demand.".to_string())
        );
    }
}

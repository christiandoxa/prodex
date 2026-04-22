use super::*;

pub(crate) fn parse_runtime_sse_payload(data_lines: &[String]) -> Option<serde_json::Value> {
    if data_lines.is_empty() {
        return None;
    }

    let payload = data_lines.join("\n");
    serde_json::from_str::<serde_json::Value>(&payload).ok()
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

pub(crate) fn extract_runtime_proxy_quota_message(body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_quota_message_from_value(&value)
    {
        return Some(message);
    }

    extract_runtime_proxy_quota_message_from_text(&String::from_utf8_lossy(body))
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
            extract_runtime_proxy_overload_message_from_text(&String::from_utf8_lossy(bytes))
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

    extract_runtime_proxy_previous_response_message_from_text(&String::from_utf8_lossy(body))
}

pub(crate) fn extract_runtime_proxy_overload_message(status: u16, body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_overload_message_from_value(&value)
    {
        return Some(message);
    }

    let body_text = String::from_utf8_lossy(body).trim().to_string();
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
    let direct_error = value.get("error");
    let response_error = value
        .get("response")
        .and_then(|response| response.get("error"));
    for error in [direct_error, response_error].into_iter().flatten() {
        let code = error.get("code").and_then(serde_json::Value::as_str);
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .or_else(|| error.get("detail").and_then(serde_json::Value::as_str));
        if matches!(code, Some("server_is_overloaded" | "slow_down")) {
            return Some(
                message
                    .unwrap_or("Upstream Codex backend is currently overloaded.")
                    .to_string(),
            );
        }
        if let Some(message) = message.filter(|message| runtime_proxy_overload_message(message)) {
            return Some(message.to_string());
        }
    }

    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(extract_runtime_proxy_overload_message_from_value),
        serde_json::Value::Object(map) => map
            .values()
            .find_map(extract_runtime_proxy_overload_message_from_value),
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
    if let Some(message) = extract_runtime_proxy_quota_message_candidate(value) {
        return Some(message);
    }

    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(extract_runtime_proxy_quota_message_from_value),
        serde_json::Value::Object(map) => map
            .values()
            .find_map(extract_runtime_proxy_quota_message_from_value),
        _ => None,
    }
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

pub(crate) fn extract_runtime_proxy_previous_response_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    if let Some(message) = extract_runtime_proxy_previous_response_message_candidate(value) {
        return Some(message);
    }

    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(extract_runtime_proxy_previous_response_message_from_value),
        serde_json::Value::Object(map) => map
            .values()
            .find_map(extract_runtime_proxy_previous_response_message_from_value),
        _ => None,
    }
}

fn extract_runtime_proxy_previous_response_message_candidate(
    value: &serde_json::Value,
) -> Option<String> {
    match value {
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

    if runtime_proxy_previous_response_missing_message(trimmed)
        || runtime_proxy_tool_context_missing_message(trimmed)
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn runtime_proxy_previous_response_missing_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("previous_response_not_found")
        || (lower.contains("previous response") && lower.contains("not found"))
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
}

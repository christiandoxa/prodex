use super::json_utils::{runtime_json_find, runtime_proxy_utf8_text};
use crate::{
    RuntimeHttpErrorClass, RuntimeHttpErrorPhase, runtime_error_signal_message_from_text,
    runtime_error_signal_message_from_value, runtime_http_error_policy,
    runtime_overload_text_message, runtime_usage_limit_text_message,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeWebsocketErrorPayload {
    Text(String),
    Binary(Vec<u8>),
    Empty,
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

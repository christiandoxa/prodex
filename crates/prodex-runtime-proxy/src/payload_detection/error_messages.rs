use super::json_utils::{runtime_json_find, runtime_proxy_utf8_text};
use crate::{
    RuntimeHttpErrorClass, RuntimeHttpErrorPhase, RuntimeHttpErrorPolicy,
    runtime_http_error_policy, runtime_stream_error_policy,
};
#[cfg(test)]
use crate::{
    runtime_error_signal_message_from_value, runtime_workspace_credit_exhausted_text_message,
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
    let policy = runtime_websocket_error_policy(payload, RuntimeHttpErrorPhase::PreCommit);
    (policy.class == RuntimeHttpErrorClass::Quota)
        .then_some(policy.message)
        .flatten()
}

pub fn extract_runtime_proxy_overload_message_from_websocket_payload(
    payload: &RuntimeWebsocketErrorPayload,
) -> Option<String> {
    let policy = runtime_websocket_error_policy(payload, RuntimeHttpErrorPhase::PreCommit);
    matches!(
        policy.class,
        RuntimeHttpErrorClass::RateLimited | RuntimeHttpErrorClass::Overload
    )
    .then_some(policy.message)
    .flatten()
}

pub fn runtime_websocket_error_policy(
    payload: &RuntimeWebsocketErrorPayload,
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => {
            runtime_stream_error_policy(text.as_bytes(), phase)
        }
        RuntimeWebsocketErrorPayload::Binary(bytes) => runtime_stream_error_policy(bytes, phase),
        RuntimeWebsocketErrorPayload::Empty => RuntimeHttpErrorPolicy::pass_through(),
    }
}

#[cfg(test)]
pub(crate) fn runtime_websocket_workspace_credit_exhausted(
    payload: &RuntimeWebsocketErrorPayload,
) -> bool {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => {
            runtime_workspace_credit_exhausted_text_message(text)
        }
        RuntimeWebsocketErrorPayload::Binary(bytes) => runtime_proxy_utf8_text(bytes)
            .is_some_and(runtime_workspace_credit_exhausted_text_message),
        RuntimeWebsocketErrorPayload::Empty => false,
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

#[cfg(test)]
pub(crate) fn extract_runtime_proxy_overload_message(status: u16, body: &[u8]) -> Option<String> {
    let policy = runtime_http_error_policy(status, body, RuntimeHttpErrorPhase::PreCommit);
    matches!(
        policy.class,
        RuntimeHttpErrorClass::Overload | RuntimeHttpErrorClass::TransientServer
    )
    .then_some(policy.message)
    .flatten()
}

#[cfg(test)]
pub(crate) fn extract_runtime_proxy_overload_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    runtime_error_signal_message_from_value(value, RuntimeHttpErrorClass::Overload)
}

#[cfg(test)]
pub(crate) fn extract_runtime_proxy_quota_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    runtime_error_signal_message_from_value(value, RuntimeHttpErrorClass::Quota)
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

pub fn runtime_proxy_value_is_invalid_previous_response_id(value: &serde_json::Value) -> bool {
    runtime_json_find(value, |candidate| {
        let serde_json::Value::Object(map) = candidate else {
            return None;
        };
        let error_type = map.get("type").and_then(serde_json::Value::as_str);
        let code = map.get("code").and_then(serde_json::Value::as_str);
        let param = map.get("param").and_then(serde_json::Value::as_str);
        let message = map
            .get("message")
            .and_then(serde_json::Value::as_str)
            .or_else(|| map.get("detail").and_then(serde_json::Value::as_str))
            .or_else(|| map.get("error").and_then(serde_json::Value::as_str));
        (runtime_previous_response_class(true, error_type, code, param, message)
            == PREVIOUS_RESPONSE_ERROR_CLASS_INVALID_ID)
            .then_some(())
    })
    .is_some()
}

pub fn runtime_proxy_body_is_invalid_previous_response_id(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .is_some_and(|value| runtime_proxy_value_is_invalid_previous_response_id(&value))
}

const PREVIOUS_RESPONSE_ERROR_CLASS_NONE: i64 = 0;
const PREVIOUS_RESPONSE_ERROR_CLASS_NOT_FOUND: i64 = 1;
const PREVIOUS_RESPONSE_ERROR_CLASS_INVALID_ID: i64 = 2;
const PREVIOUS_RESPONSE_ERROR_CLASS_TOOL_CONTEXT: i64 = 3;

fn extract_runtime_proxy_previous_response_message_candidate(
    value: &serde_json::Value,
) -> Option<String> {
    let serde_json::Value::Object(map) = value else {
        return None;
    };
    let message = map
        .get("message")
        .and_then(serde_json::Value::as_str)
        .or_else(|| map.get("detail").and_then(serde_json::Value::as_str))
        .or_else(|| map.get("error").and_then(serde_json::Value::as_str));
    let code = map.get("code").and_then(serde_json::Value::as_str);
    let error_type = map.get("type").and_then(serde_json::Value::as_str);
    let param = map.get("param").and_then(serde_json::Value::as_str);

    match runtime_previous_response_class(true, error_type, code, param, message) {
        PREVIOUS_RESPONSE_ERROR_CLASS_NOT_FOUND => Some(
            message
                .unwrap_or("Previous response could not be found on the selected Codex account.")
                .to_string(),
        ),
        PREVIOUS_RESPONSE_ERROR_CLASS_INVALID_ID | PREVIOUS_RESPONSE_ERROR_CLASS_TOOL_CONTEXT => {
            message.map(str::to_string)
        }
        PREVIOUS_RESPONSE_ERROR_CLASS_NONE => None,
        _ => unreachable!("validated previous-response error class"),
    }
}

pub fn extract_runtime_proxy_previous_response_message_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    (runtime_previous_response_class(false, None, None, None, Some(trimmed))
        != PREVIOUS_RESPONSE_ERROR_CLASS_NONE)
        .then(|| trimmed.to_string())
}

fn runtime_previous_response_class(
    structured: bool,
    error_type: Option<&str>,
    code: Option<&str>,
    param: Option<&str>,
    message: Option<&str>,
) -> i64 {
    #[cfg(feature = "mojo")]
    {
        let mode = if structured {
            prodex_mojo_core::rich::PREVIOUS_RESPONSE_ERROR_MODE_STRUCTURED
        } else {
            prodex_mojo_core::rich::PREVIOUS_RESPONSE_ERROR_MODE_TEXT
        };
        prodex_mojo_core::rich::previous_response_error_class(
            mode, error_type, code, param, message,
        )
        .expect("Mojo previous-response classifier returned an invalid result")
    }
    #[cfg(not(feature = "mojo"))]
    {
        if structured {
            rust_oracle::structured_class(error_type, code, param, message)
        } else {
            rust_oracle::text_class(message.unwrap_or_default())
        }
    }
}

#[cfg(any(not(feature = "mojo"), test))]
mod rust_oracle {
    use super::*;

    pub(super) fn structured_class(
        error_type: Option<&str>,
        code: Option<&str>,
        param: Option<&str>,
        message: Option<&str>,
    ) -> i64 {
        if code == Some("previous_response_not_found") {
            return PREVIOUS_RESPONSE_ERROR_CLASS_NOT_FOUND;
        }
        if error_type == Some("invalid_request_error")
            && message.is_some_and(invalid_previous_response_id_message)
        {
            return PREVIOUS_RESPONSE_ERROR_CLASS_INVALID_ID;
        }
        if message.is_some_and(tool_context_missing_message)
            && (error_type == Some("invalid_request_error") || param == Some("input"))
        {
            return PREVIOUS_RESPONSE_ERROR_CLASS_TOOL_CONTEXT;
        }
        PREVIOUS_RESPONSE_ERROR_CLASS_NONE
    }

    pub(super) fn text_class(text: &str) -> i64 {
        let lower = text.to_ascii_lowercase();
        if invalid_previous_response_id_message(text) {
            PREVIOUS_RESPONSE_ERROR_CLASS_INVALID_ID
        } else if lower.starts_with("previous_response_not_found")
            || (lower.starts_with("previous response") && lower.contains("not found"))
        {
            PREVIOUS_RESPONSE_ERROR_CLASS_NOT_FOUND
        } else if (lower.starts_with("invalid_request_error:")
            || lower.starts_with("no tool call found")
            || lower.starts_with("no function call found"))
            && tool_context_missing_message(text)
        {
            PREVIOUS_RESPONSE_ERROR_CLASS_TOOL_CONTEXT
        } else {
            PREVIOUS_RESPONSE_ERROR_CLASS_NONE
        }
    }

    fn tool_context_missing_message(message: &str) -> bool {
        let lower = message.to_ascii_lowercase();
        lower.contains("no tool call found") || lower.contains("no function call found")
    }

    fn invalid_previous_response_id_message(message: &str) -> bool {
        message
            .trim()
            .eq_ignore_ascii_case("Invalid `previous_response_id`.")
    }
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_parity_tests {
    use super::*;

    #[test]
    fn previous_response_error_classifier_matches_rust_oracle() {
        let structured = [
            (
                Some("invalid_request_error"),
                None,
                None,
                Some("Invalid `previous_response_id`."),
            ),
            (
                Some("invalid_request_error"),
                None,
                None,
                Some("  INVALID `PREVIOUS_RESPONSE_ID`.  "),
            ),
            (
                None,
                Some("previous_response_not_found"),
                None,
                Some("missing"),
            ),
            (None, Some("previous_response_not_found"), None, None),
            (
                Some("invalid_request_error"),
                None,
                None,
                Some("No tool call found for call_123"),
            ),
            (
                None,
                None,
                Some("input"),
                Some("No function call found for call_123"),
            ),
            (
                None,
                None,
                None,
                Some("The docs mention no tool call found"),
            ),
        ];
        for (error_type, code, param, message) in structured {
            assert_eq!(
                runtime_previous_response_class(true, error_type, code, param, message),
                rust_oracle::structured_class(error_type, code, param, message),
                "structured={error_type:?}/{code:?}/{param:?}/{message:?}"
            );
        }

        for text in [
            "previous_response_not_found: missing",
            "Previous response with id 'resp_123' not found.",
            "Invalid `previous_response_id`.",
            "invalid_request_error: No tool call found for call_123",
            "No function call found for call_123",
            "The docs mention previous_response_not_found",
            "No tool call found is documented here but not at the prefix",
        ] {
            assert_eq!(
                runtime_previous_response_class(false, None, None, None, Some(text)),
                rust_oracle::text_class(text),
                "text={text:?}"
            );
        }
    }
}

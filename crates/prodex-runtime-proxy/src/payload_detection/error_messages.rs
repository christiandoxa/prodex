use super::json_utils::{runtime_json_borrowed_shape, runtime_proxy_utf8_text};
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
    // This UTF-8 lossy boundary is host-side. Unicode whitespace decisions,
    // clipping and ellipsis bytes are exclusively compiled Mojo semantics.
    let body = String::from_utf8_lossy(body);
    prodex_mojo_core::rich::runtime_proxy_body_snippet(&body, max_chars)
        .expect("Mojo runtime proxy body snippet returned invalid output")
}

// Serde's map access is host acquisition only. First-string field
// precedence and error classification are exclusively compiled Mojo policy.
fn previous_response_json_fields<'a>(
    originals: &[&'a serde_json::Value],
) -> Vec<prodex_mojo_core::rich::PreviousResponseJsonObjectFields<'a>> {
    originals
        .iter()
        .map(|value| {
            let string = |key: &str| value.get(key).and_then(serde_json::Value::as_str);
            prodex_mojo_core::rich::PreviousResponseJsonObjectFields {
                error_type: string("type"),
                code: string("code"),
                param: string("param"),
                message: string("message"),
                detail: string("detail"),
                error: string("error"),
            }
        })
        .collect()
}

pub fn extract_runtime_proxy_previous_response_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    use prodex_mojo_core::rich::{PreviousResponseJsonOperation, PreviousResponseJsonSelection};
    let (nodes, originals) = runtime_json_borrowed_shape(value);
    let fields = previous_response_json_fields(&originals);
    let selection = prodex_mojo_core::rich::previous_response_json_search(
        &nodes,
        &fields,
        PreviousResponseJsonOperation::Message,
    )
    .expect("compiled Mojo previous-response JSON search returned invalid output");
    match selection {
        PreviousResponseJsonSelection::None => None,
        PreviousResponseJsonSelection::Message {
            object_index,
            field,
        } => {
            let key = match field {
                prodex_mojo_core::rich::PreviousResponseJsonTextField::Message => "message",
                prodex_mojo_core::rich::PreviousResponseJsonTextField::Detail => "detail",
                prodex_mojo_core::rich::PreviousResponseJsonTextField::Error => "error",
            };
            Some(
                originals[object_index]
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .expect("Mojo-selected original error field must be text")
                    .to_string(),
            )
        }
        PreviousResponseJsonSelection::DefaultNotFound => {
            Some("Previous response could not be found on the selected Codex account.".to_string())
        }
        PreviousResponseJsonSelection::InvalidId(_) => {
            unreachable!("validated Mojo message operation")
        }
    }
}

pub fn runtime_proxy_value_is_invalid_previous_response_id(value: &serde_json::Value) -> bool {
    use prodex_mojo_core::rich::{PreviousResponseJsonOperation, PreviousResponseJsonSelection};
    let (nodes, originals) = runtime_json_borrowed_shape(value);
    let fields = previous_response_json_fields(&originals);
    matches!(
        prodex_mojo_core::rich::previous_response_json_search(
            &nodes,
            &fields,
            PreviousResponseJsonOperation::InvalidId,
        )
        .expect("compiled Mojo previous-response ID detection returned invalid output"),
        PreviousResponseJsonSelection::InvalidId(_),
    )
}

pub fn runtime_proxy_body_is_invalid_previous_response_id(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .is_some_and(|value| runtime_proxy_value_is_invalid_previous_response_id(&value))
}

const PREVIOUS_RESPONSE_ERROR_CLASS_NONE: i64 = 0;

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
    let mode = if structured {
        prodex_mojo_core::rich::PREVIOUS_RESPONSE_ERROR_MODE_STRUCTURED
    } else {
        prodex_mojo_core::rich::PREVIOUS_RESPONSE_ERROR_MODE_TEXT
    };
    prodex_mojo_core::rich::previous_response_error_class(mode, error_type, code, param, message)
        .expect("Mojo previous-response classifier returned an invalid result")
}

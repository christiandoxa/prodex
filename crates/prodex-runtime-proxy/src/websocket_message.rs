use crate::{
    RuntimeHttpErrorAction, RuntimeHttpErrorClass, RuntimeHttpErrorPhase, RuntimeTokenUsage,
    RuntimeWebsocketErrorPayload, extract_runtime_proxy_previous_response_message,
    extract_runtime_proxy_previous_response_message_from_value,
    extract_runtime_response_ids_from_value, extract_runtime_token_usage_from_value,
    extract_runtime_turn_state_from_value, runtime_proxy_stale_continuation_message,
    runtime_response_event_type_from_value, runtime_stream_error_policy_from_value,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBufferedWebsocketTextFrame {
    pub text: String,
    pub response_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWebsocketRetryInspectionKind {
    ConnectionLimitReached,
    QuotaBlocked,
    Overloaded,
    PreviousResponseNotFound,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeInspectedWebsocketTextFrame {
    pub event_type: Option<String>,
    pub turn_state: Option<String>,
    pub response_ids: Vec<String>,
    pub token_usage: Option<RuntimeTokenUsage>,
    pub retry_kind: Option<RuntimeWebsocketRetryInspectionKind>,
    pub precommit_hold: bool,
    pub terminal_event: bool,
}

pub fn runtime_websocket_should_promote_committed_profile(
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    compact_followup_profile: Option<&(String, &'static str)>,
    _request_session_id_header_present: bool,
    bound_session_profile: Option<&str>,
) -> bool {
    previous_response_id.is_none()
        && bound_profile.is_none()
        && request_turn_state.is_none()
        && turn_state_profile.is_none()
        && compact_followup_profile.is_none()
        && bound_session_profile.is_none()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWebsocketMessageLoopAction {
    Continue,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWebsocketDirectCurrentFallbackReason {
    PrecommitBudgetExhausted,
    CandidateExhausted,
}

impl RuntimeWebsocketDirectCurrentFallbackReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrecommitBudgetExhausted => "precommit_budget_exhausted",
            Self::CandidateExhausted => "candidate_exhausted",
        }
    }

    pub fn reset_previous_response_retry_index_on_local_block(self) -> bool {
        matches!(self, Self::PrecommitBudgetExhausted)
    }
}

pub fn runtime_websocket_error_payload_from_http_body(body: &[u8]) -> RuntimeWebsocketErrorPayload {
    if body.is_empty() {
        return RuntimeWebsocketErrorPayload::Empty;
    }

    match std::str::from_utf8(body) {
        Ok(text) => RuntimeWebsocketErrorPayload::Text(text.to_string()),
        Err(_) => RuntimeWebsocketErrorPayload::Binary(body.to_vec()),
    }
}

pub fn runtime_proxy_websocket_error_payload_text(
    status: u16,
    code: &str,
    message: &str,
) -> String {
    serde_json::json!({
        "type": "error",
        "status": status,
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string()
}

pub fn runtime_translate_previous_response_websocket_text_frame(payload: &str) -> String {
    payload.to_string()
}

pub fn runtime_translate_precommit_previous_response_websocket_text_frame(payload: &str) -> String {
    if extract_runtime_proxy_previous_response_message(payload.as_bytes()).is_none() {
        return payload.to_string();
    }

    let message = runtime_proxy_stale_continuation_message();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return runtime_proxy_websocket_error_payload_text(409, "stale_continuation", message);
    };

    let event_type = runtime_response_event_type_from_value(&value);
    if event_type.as_deref() == Some("response.failed") {
        if value.get("response").is_some() {
            return serde_json::json!({
                "type": "response.failed",
                "status": 409,
                "response": {
                    "error": {
                        "code": "stale_continuation",
                        "message": message,
                    }
                }
            })
            .to_string();
        }

        return serde_json::json!({
            "type": "response.failed",
            "status": 409,
            "error": {
                "code": "stale_continuation",
                "message": message,
            }
        })
        .to_string();
    }

    runtime_proxy_websocket_error_payload_text(409, "stale_continuation", message)
}

pub fn inspect_runtime_websocket_text_frame(payload: &str) -> RuntimeInspectedWebsocketTextFrame {
    inspect_runtime_websocket_text_frame_with_phase(payload, RuntimeHttpErrorPhase::PreCommit)
}

pub fn inspect_runtime_websocket_text_frame_with_phase(
    payload: &str,
    phase: RuntimeHttpErrorPhase,
) -> RuntimeInspectedWebsocketTextFrame {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return RuntimeInspectedWebsocketTextFrame::default();
    };

    let event_type = runtime_response_event_type_from_value(&value);
    let error_policy = runtime_stream_error_policy_from_value(&value, phase);
    let retry_kind = if runtime_websocket_connection_limit_reached(&value) {
        Some(RuntimeWebsocketRetryInspectionKind::ConnectionLimitReached)
    } else if extract_runtime_proxy_previous_response_message_from_value(&value).is_some() {
        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound)
    } else if error_policy.action == RuntimeHttpErrorAction::RetryProfile
        && matches!(
            error_policy.class,
            RuntimeHttpErrorClass::Overload | RuntimeHttpErrorClass::TransientServer
        )
    {
        Some(RuntimeWebsocketRetryInspectionKind::Overloaded)
    } else if error_policy.action == RuntimeHttpErrorAction::RotateProfile
        && matches!(
            error_policy.class,
            RuntimeHttpErrorClass::Quota | RuntimeHttpErrorClass::ProfileUnavailable
        )
    {
        Some(RuntimeWebsocketRetryInspectionKind::QuotaBlocked)
    } else {
        None
    };
    let precommit_hold = event_type
        .as_deref()
        .is_some_and(runtime_proxy_precommit_hold_event_kind);
    let terminal_event = event_type
        .as_deref()
        .is_some_and(runtime_responses_websocket_terminal_event_kind)
        || runtime_websocket_wrapped_error_is_terminal(&value);

    RuntimeInspectedWebsocketTextFrame {
        event_type,
        turn_state: extract_runtime_turn_state_from_value(&value),
        response_ids: extract_runtime_response_ids_from_value(&value),
        token_usage: extract_runtime_token_usage_from_value(&value),
        retry_kind,
        precommit_hold,
        terminal_event,
    }
}

fn runtime_websocket_connection_limit_reached(value: &serde_json::Value) -> bool {
    const CODE: &str = "websocket_connection_limit_reached";
    value
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|code| code.eq_ignore_ascii_case(CODE))
        || value
            .get("code")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|code| code.eq_ignore_ascii_case(CODE))
}

fn runtime_websocket_wrapped_error_is_terminal(value: &serde_json::Value) -> bool {
    runtime_response_event_type_from_value(value).as_deref() == Some("error")
        && (runtime_websocket_connection_limit_reached(value)
            || runtime_websocket_wrapped_error_status(value).is_some_and(|status| status >= 400))
}

fn runtime_websocket_wrapped_error_status(value: &serde_json::Value) -> Option<u16> {
    value
        .get("status")
        .or_else(|| value.get("status_code"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|status| u16::try_from(status).ok())
}

pub fn runtime_response_event_type(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|value| runtime_response_event_type_from_value(&value))
}

pub fn runtime_proxy_precommit_hold_event_kind(kind: &str) -> bool {
    matches!(
        kind,
        "codex.rate_limits"
            | "codex.response.metadata"
            | "response.metadata"
            | "response.created"
            | "response.in_progress"
            | "response.queued"
            | "response.output_item.added"
            | "response.content_part.added"
            | "response.reasoning_summary_part.added"
    )
}

pub fn runtime_realtime_websocket_terminal_event_kind(kind: &str) -> bool {
    matches!(
        kind,
        "session.started"
            | "session.updated"
            | "conversation.item.added"
            | "conversation.item.done"
            | "delegation.created"
            | "response.cancelled"
            | "response.done"
            | "turn.done"
            | "error"
    )
}

fn runtime_responses_websocket_terminal_event_kind(kind: &str) -> bool {
    matches!(
        kind,
        "response.completed" | "response.failed" | "response.incomplete"
    )
}

pub fn is_runtime_terminal_event(payload: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return false;
    };
    runtime_response_event_type_from_value(&value)
        .as_deref()
        .is_some_and(runtime_responses_websocket_terminal_event_kind)
        || runtime_websocket_wrapped_error_is_terminal(&value)
}

#[cfg(test)]
#[path = "../tests/src/websocket_message.rs"]
mod tests;

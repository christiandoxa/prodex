use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeWorkflowRecoveryClass {
    UsageLimit,
    RateLimit,
    Overload,
    Auth,
    ProfileUnavailable,
    Transport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeWorkflowEvidence {
    pub(crate) acceptance_state: &'static str,
    pub(crate) stream_committed: Option<bool>,
    pub(crate) side_effect_state: &'static str,
}

impl Default for RuntimeWorkflowEvidence {
    fn default() -> Self {
        Self {
            acceptance_state: "acceptance_ambiguous",
            stream_committed: None,
            side_effect_state: "unknown",
        }
    }
}

impl RuntimeWorkflowEvidence {
    pub(super) fn safe_to_resume(self) -> bool {
        matches!(
            self.acceptance_state,
            "accepted_but_uncommitted" | "committed" | "side_effect_observed"
        )
    }
}

impl RuntimeWorkflowRecoveryClass {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::UsageLimit => "usage_limit",
            Self::RateLimit => "rate_limit",
            Self::Overload => "overload",
            Self::Auth => "auth",
            Self::ProfileUnavailable => "profile_unavailable",
            Self::Transport => "transport",
        }
    }

    pub(super) const fn retries_after_pool_round(self) -> bool {
        matches!(self, Self::RateLimit | Self::Overload | Self::Transport)
    }
}

pub(super) fn runtime_workflow_recovery_class(
    value: &Value,
    session_id: &str,
) -> Option<RuntimeWorkflowRecoveryClass> {
    if !record_matches_session(value, session_id) {
        return None;
    }
    let (error, outer_status) = terminal_error(value)?;
    let info = error
        .get("codex_error_info")
        .or_else(|| error.get("codexErrorInfo"))
        .or_else(|| error.get("error")?.get("codex_error_info"))
        .or_else(|| error.get("error")?.get("codexErrorInfo"))?;
    classify_codex_error(info, outer_status, error)
}

pub(super) fn runtime_workflow_effective_model(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    let model = match object.get("type").and_then(Value::as_str) {
        Some("turn_context") => object.get("payload")?.get("model")?.as_str(),
        Some("event_msg")
            if object.get("payload")?.get("type")?.as_str() == Some("model_reroute") =>
        {
            object.get("payload")?.get("to_model")?.as_str()
        }
        _ if object.get("method").and_then(Value::as_str) == Some("model/rerouted") => {
            object.get("params")?.get("toModel")?.as_str()
        }
        _ => None,
    }?;
    let model = model.trim();
    (!model.is_empty() && model.len() <= 128 && !model.chars().any(char::is_control))
        .then(|| model.to_string())
}

pub(super) fn observe_runtime_workflow_evidence(
    value: &Value,
    evidence: &mut RuntimeWorkflowEvidence,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    let outer = object.get("type").and_then(Value::as_str);
    let payload = object.get("payload").and_then(Value::as_object);
    let payload_type = payload
        .and_then(|payload| payload.get("type"))
        .and_then(Value::as_str);
    let role = payload
        .and_then(|payload| payload.get("role"))
        .and_then(Value::as_str);

    let turn_started = outer == Some("event_msg") && payload_type == Some("turn_started")
        || object.get("method").and_then(Value::as_str) == Some("turn/started");
    if turn_started {
        *evidence = RuntimeWorkflowEvidence::default();
        return;
    }

    let user = matches!(outer, Some("response_item" | "message")) && role == Some("user")
        || outer == Some("event_msg") && payload_type == Some("user_message");
    if user {
        *evidence = RuntimeWorkflowEvidence {
            acceptance_state: "accepted_but_uncommitted",
            stream_committed: Some(false),
            side_effect_state: "none",
        };
        return;
    }

    let assistant = matches!(outer, Some("response_item" | "message")) && role == Some("assistant")
        || outer == Some("event_msg") && payload_type == Some("agent_message");
    if assistant {
        evidence.acceptance_state = "committed";
        evidence.stream_committed = Some(true);
    }

    let side_effect = outer == Some("response_item")
        && matches!(
            payload_type,
            Some("function_call_output" | "custom_tool_call_output")
        )
        || matches!(
            outer,
            Some("exec_command_end" | "patch_apply_end" | "mcp_tool_call_end" | "tool_completed")
        )
        || outer == Some("event_msg")
            && matches!(
                payload_type,
                Some(
                    "exec_command_end" | "patch_apply_end" | "mcp_tool_call_end" | "tool_completed"
                )
            );
    if side_effect {
        evidence.acceptance_state = "side_effect_observed";
        evidence.stream_committed = Some(true);
        evidence.side_effect_state = "observed";
    }
}

fn terminal_error(value: &Value) -> Option<(&Map<String, Value>, Option<u16>)> {
    let object = value.as_object()?;
    let kind = object.get("type").and_then(Value::as_str);
    if kind == Some("event_msg") {
        let payload = object.get("payload")?.as_object()?;
        return (payload.get("type").and_then(Value::as_str) == Some("error"))
            .then_some((payload, http_status(payload)));
    }
    if kind.is_some_and(|kind| matches!(kind, "error" | "turn.failed" | "turn_failed")) {
        let error = object
            .get("error")
            .and_then(Value::as_object)
            .or_else(|| object.get("payload").and_then(Value::as_object))
            .unwrap_or(object);
        return Some((error, http_status(object).or_else(|| http_status(error))));
    }
    if kind
        .is_some_and(|kind| matches!(kind, "turn.completed" | "turn_completed" | "task_complete"))
    {
        return failed_turn(object.get("turn").unwrap_or(value).as_object()?);
    }
    if object.get("method").and_then(Value::as_str) == Some("turn/completed") {
        return failed_turn(object.get("params")?.get("turn")?.as_object()?);
    }
    None
}

fn failed_turn(turn: &Map<String, Value>) -> Option<(&Map<String, Value>, Option<u16>)> {
    if turn.get("status").and_then(Value::as_str) != Some("failed") {
        return None;
    }
    let error = turn.get("error")?.as_object()?;
    Some((error, http_status(turn).or_else(|| http_status(error))))
}

fn classify_codex_error(
    info: &Value,
    outer_status: Option<u16>,
    error: &Map<String, Value>,
) -> Option<RuntimeWorkflowRecoveryClass> {
    let name = info
        .as_str()
        .or_else(|| info.as_object()?.keys().next().map(String::as_str))?;
    let variant_status = info
        .as_object()
        .and_then(|object| object.values().find_map(Value::as_object))
        .and_then(http_status);
    let status = outer_status.or(variant_status);
    match name {
        "usage_limit_exceeded" | "usageLimitExceeded" => {
            Some(RuntimeWorkflowRecoveryClass::UsageLimit)
        }
        "rate_limit_exceeded" | "rateLimitExceeded" => {
            Some(RuntimeWorkflowRecoveryClass::RateLimit)
        }
        "server_overloaded" | "serverOverloaded" => Some(RuntimeWorkflowRecoveryClass::Overload),
        "unauthorized" => Some(RuntimeWorkflowRecoveryClass::Auth),
        "internal_server_error"
        | "internalServerError"
        | "http_connection_failed"
        | "httpConnectionFailed"
        | "response_stream_connection_failed"
        | "responseStreamConnectionFailed"
        | "response_stream_disconnected"
        | "responseStreamDisconnected"
        | "response_too_many_failed_attempts"
        | "responseTooManyFailedAttempts"
            if transient_status(status) =>
        {
            Some(RuntimeWorkflowRecoveryClass::Transport)
        }
        "other" => classify_unexpected_status(error),
        _ => None,
    }
}

fn classify_unexpected_status(error: &Map<String, Value>) -> Option<RuntimeWorkflowRecoveryClass> {
    let message = error.get("message").and_then(Value::as_str)?;
    let message = message
        .strip_prefix("Error running remote compact task: ")
        .unwrap_or(message);
    let value = message.strip_prefix("unexpected status ")?;
    let (status, detail) = value.split_once(": ")?;
    let status = status.split_whitespace().next()?.parse::<u16>().ok()?;
    match status {
        401 => Some(RuntimeWorkflowRecoveryClass::Auth),
        500 | 502 | 503 | 504 | 529 => Some(RuntimeWorkflowRecoveryClass::Transport),
        402 | 403
            if runtime_proxy_crate::runtime_authoritative_usage_limit_text_message(detail) =>
        {
            Some(RuntimeWorkflowRecoveryClass::UsageLimit)
        }
        402 | 403 => classify_unexpected_profile_body(status, detail),
        _ => None,
    }
}

fn classify_unexpected_profile_body(
    status: u16,
    detail: &str,
) -> Option<RuntimeWorkflowRecoveryClass> {
    let mut values = serde_json::Deserializer::from_str(detail).into_iter::<Value>();
    values.next()?.ok()?;
    let body = &detail.as_bytes()[..values.byte_offset()];
    let policy = runtime_proxy_crate::runtime_http_error_policy(
        status,
        body,
        runtime_proxy_crate::RuntimeHttpErrorPhase::PreCommit,
    );
    match policy.class {
        runtime_proxy_crate::RuntimeHttpErrorClass::Quota => {
            Some(RuntimeWorkflowRecoveryClass::UsageLimit)
        }
        runtime_proxy_crate::RuntimeHttpErrorClass::ProfileUnavailable => {
            Some(RuntimeWorkflowRecoveryClass::ProfileUnavailable)
        }
        _ => None,
    }
}

fn transient_status(status: Option<u16>) -> bool {
    matches!(status, None | Some(401 | 429 | 500 | 502 | 503 | 504 | 529))
}

fn http_status(object: &Map<String, Value>) -> Option<u16> {
    [
        "http_status_code",
        "httpStatusCode",
        "status_code",
        "statusCode",
        "status",
    ]
    .into_iter()
    .find_map(|key| object.get(key).and_then(Value::as_u64))
    .and_then(|status| u16::try_from(status).ok())
}

fn record_matches_session(value: &Value, session_id: &str) -> bool {
    fn visit(value: &Value, session_id: &str, depth: usize) -> bool {
        if depth > 5 {
            return true;
        }
        let Some(object) = value.as_object() else {
            return true;
        };
        for key in ["session_id", "sessionId", "thread_id", "threadId"] {
            if let Some(identity) = object.get(key).and_then(Value::as_str)
                && identity != session_id
            {
                return false;
            }
        }
        object
            .iter()
            .filter(|(key, _)| !matches!(key.as_str(), "message" | "content" | "text" | "delta"))
            .all(|(_, value)| visit(value, session_id, depth + 1))
    }
    visit(value, session_id, 0)
}

#[cfg(test)]
#[path = "workflow/tests.rs"]
mod tests;

use super::ValidationFailure;
use serde_json::Value;

#[derive(Default)]
pub(super) struct LifecyclePayloadValidation;

impl LifecyclePayloadValidation {
    pub(super) fn observe_preview_and_frame(
        &mut self,
        preview: &Value,
        frame: Option<&Value>,
    ) -> Option<ValidationFailure> {
        if !preview["preview"]["parse_ok"].as_bool().unwrap_or_default() {
            return None;
        }
        match preview["preview"]["summary"]["lifecycle_stage"].as_str()? {
            "thread_started_notification"
            | "thread_resume_request"
            | "thread_fork_request"
            | "turn_start_request"
                if preview_thread_id(preview).is_none() =>
            {
                return Some(payload_failure(preview, "lifecycle_missing_thread_id"));
            }
            _ => {}
        }
        if preview["preview"]["summary"]["lifecycle_stage"].as_str()?
            == "thread_started_notification"
            && frame
                .and_then(|frame| frame_string(frame, &["params", "thread", "id"]))
                .is_none()
        {
            return Some(payload_failure(
                preview,
                "lifecycle_missing_thread_object_id",
            ));
        }
        if preview["preview"]["summary"]["lifecycle_stage"].as_str()?
            == "thread_started_notification"
        {
            if let Some(reason) = frame
                .map(|frame| thread_status_failure_reason(frame, &["params", "thread", "status"]))
                .unwrap_or(Some("lifecycle_missing_thread_status"))
            {
                return Some(payload_failure(preview, reason));
            }
            if !frame.is_some_and(|frame| thread_object_has_context(frame, &["params", "thread"])) {
                return Some(payload_failure(preview, "lifecycle_missing_thread_context"));
            }
        }
        if preview["preview"]["summary"]["lifecycle_stage"].as_str()? == "turn_start_request"
            && !frame
                .and_then(|frame| frame.get("params"))
                .and_then(|params| params.get("input"))
                .is_some_and(Value::is_array)
        {
            return Some(payload_failure(preview, "lifecycle_missing_turn_input"));
        }
        if matches!(
            preview["preview"]["summary"]["lifecycle_stage"].as_str(),
            Some("turn_started_notification" | "turn_completed_notification")
        ) {
            let turn_status = frame
                .and_then(|frame| frame.get("params"))
                .and_then(|params| params.get("turn"))
                .and_then(|turn| turn.get("status"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|status| !status.is_empty());
            match turn_status {
                Some(status) if is_valid_turn_status(status) => {}
                Some(_) => {
                    return Some(payload_failure(preview, "lifecycle_invalid_turn_status"));
                }
                None => {
                    return Some(payload_failure(preview, "lifecycle_missing_turn_status"));
                }
            }
            if !frame
                .and_then(|frame| frame.get("params"))
                .and_then(|params| params.get("turn"))
                .and_then(|turn| turn.get("items"))
                .is_some_and(Value::is_array)
            {
                return Some(payload_failure(preview, "lifecycle_missing_turn_items"));
            }
        }
        None
    }
}

pub(super) fn preview_thread_id(preview: &Value) -> Option<String> {
    preview["preview"]["summary"]["metadata"]["thread_id"]
        .as_str()
        .map(str::trim)
        .filter(|thread_id| !thread_id.is_empty())
        .map(str::to_string)
}

pub(super) fn preview_turn_id(preview: &Value) -> Option<String> {
    preview["preview"]["summary"]["metadata"]["turn_id"]
        .as_str()
        .map(str::trim)
        .filter(|turn_id| !turn_id.is_empty())
        .map(str::to_string)
}

pub(super) fn preview_id_key(preview: &Value) -> Option<String> {
    let id = &preview["preview"]["summary"]["id"];
    (!id.is_null()).then(|| id.to_string())
}

pub(super) fn frame_string(frame: &Value, path: &[&str]) -> Option<String> {
    frame_value(frame, path)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn frame_value<'a>(frame: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = frame;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

pub(super) fn thread_response_has_context(frame: &Value) -> bool {
    ["cwd", "model", "modelProvider"]
        .iter()
        .all(|field| frame_string(frame, &["result", field]).is_some())
        && ["approvalPolicy", "approvalsReviewer", "sandbox"]
            .iter()
            .all(|field| {
                frame_value(frame, &["result", field]).is_some_and(|value| !value.is_null())
            })
}

pub(super) fn thread_response_has_valid_context(frame: &Value) -> bool {
    frame_value(frame, &["result", "approvalPolicy"]).is_some_and(is_valid_approval_policy)
        && frame_string(frame, &["result", "approvalsReviewer"])
            .is_some_and(|value| is_valid_approvals_reviewer(&value))
        && frame_string(frame, &["result", "sandbox", "type"])
            .is_some_and(|value| is_valid_sandbox_type(&value))
}

pub(super) fn thread_object_has_context(frame: &Value, path: &[&str]) -> bool {
    let Some(thread) = frame_value(frame, path) else {
        return false;
    };
    ["cliVersion", "cwd", "modelProvider", "sessionId"]
        .iter()
        .all(|field| {
            thread
                .get(*field)
                .and_then(Value::as_str)
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
        })
        && ["createdAt", "updatedAt"]
            .iter()
            .all(|field| thread.get(*field).is_some_and(Value::is_number))
        && thread.get("source").is_some_and(is_valid_session_source)
        && thread.get("preview").is_some_and(Value::is_string)
        && thread.get("ephemeral").is_some_and(Value::is_boolean)
        && thread.get("turns").is_some_and(Value::is_array)
}

fn is_valid_session_source(value: &Value) -> bool {
    match value {
        Value::String(value) => matches!(
            value.trim(),
            "cli" | "vscode" | "exec" | "appServer" | "unknown"
        ),
        Value::Object(source) => {
            source.get("custom").is_some_and(Value::is_string)
                || source.get("subAgent").is_some_and(|value| !value.is_null())
        }
        _ => false,
    }
}

fn is_valid_approval_policy(value: &Value) -> bool {
    match value {
        Value::String(value) => matches!(value.trim(), "untrusted" | "on-request" | "never"),
        Value::Object(_) => true,
        _ => false,
    }
}

fn is_valid_approvals_reviewer(value: &str) -> bool {
    matches!(value, "user" | "auto_review" | "guardian_subagent")
}

fn is_valid_sandbox_type(value: &str) -> bool {
    matches!(
        value,
        "dangerFullAccess" | "readOnly" | "externalSandbox" | "workspaceWrite"
    )
}

pub(super) fn is_valid_turn_status(status: &str) -> bool {
    matches!(
        status,
        "completed" | "interrupted" | "failed" | "inProgress"
    )
}

fn is_valid_thread_status(status: &str) -> bool {
    matches!(status, "notLoaded" | "idle" | "systemError" | "active")
}

pub(super) fn thread_status_failure_reason(frame: &Value, path: &[&str]) -> Option<&'static str> {
    let Some(status) = frame_value(frame, path) else {
        return Some("lifecycle_missing_thread_status");
    };
    let Some(status_type) = status
        .get("type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Some("lifecycle_missing_thread_status");
    };
    if !is_valid_thread_status(status_type) {
        return Some("lifecycle_invalid_thread_status");
    }
    if status_type == "active" {
        let Some(active_flags) = status.get("activeFlags").and_then(Value::as_array) else {
            return Some("lifecycle_invalid_thread_status");
        };
        if !active_flags
            .iter()
            .all(|flag| flag.as_str().is_some_and(is_valid_thread_active_flag))
        {
            return Some("lifecycle_invalid_thread_status");
        }
    }
    None
}

fn is_valid_thread_active_flag(flag: &str) -> bool {
    matches!(flag, "waitingOnApproval" | "waitingOnUserInput")
}

fn payload_failure(preview: &Value, reason: &'static str) -> ValidationFailure {
    let stage = preview["preview"]["summary"]["lifecycle_stage"]
        .as_str()
        .unwrap_or("unknown");
    ValidationFailure::from_preview(preview, reason).lifecycle_stage(stage)
}

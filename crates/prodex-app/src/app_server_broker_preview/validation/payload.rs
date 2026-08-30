use super::ValidationFailure;
use serde_json::Value;

#[cfg(feature = "mojo-core")]
use super::super::super::app_server_broker_protocol::app_server_broker_mojo_validation_reason;
#[cfg(feature = "mojo-core")]
use prodex_mojo_core::rich::AppServerBrokerValidationInput;

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
        let stage = preview["preview"]["summary"]["lifecycle_stage"].as_str()?;
        #[cfg(feature = "mojo-core")]
        {
            app_server_broker_mojo_validation_reason(AppServerBrokerValidationInput {
                response: false,
                stage,
                thread_id_present: preview_thread_id(preview).is_some(),
                thread_object_id_present: frame
                    .and_then(|frame| frame_string(frame, &["params", "thread", "id"]))
                    .is_some(),
                thread_status: frame_thread_status_type(frame, &["params", "thread", "status"]),
                thread_active_flags_valid: frame_active_flags_valid(
                    frame,
                    &["params", "thread", "status"],
                ),
                thread_object_context: frame
                    .is_some_and(|frame| thread_object_has_context(frame, &["params", "thread"])),
                response_thread_context: false,
                response_thread_context_valid: false,
                response_thread_object_context: false,
                turn_input: frame
                    .and_then(|frame| frame_value(frame, &["params", "input"]))
                    .is_some_and(Value::is_array),
                turn_id_present: preview_turn_id(preview).is_some(),
                turn_status: frame_turn_status(frame, &["params", "turn", "status"]),
                turn_items: frame
                    .and_then(|frame| frame_value(frame, &["params", "turn", "items"]))
                    .is_some_and(Value::is_array),
            })
            .map(|reason| payload_failure(preview, reason))
        }
        #[cfg(not(feature = "mojo-core"))]
        {
            if let Some(failure) = validate_thread_id(preview, stage) {
                return Some(failure);
            }
            if let Some(failure) = validate_thread_started(preview, frame, stage) {
                return Some(failure);
            }
            if let Some(failure) = validate_turn_input(preview, frame, stage) {
                return Some(failure);
            }
            if let Some(failure) = validate_turn_status(preview, frame, stage) {
                return Some(failure);
            }
            None
        }
    }
}

#[cfg(not(feature = "mojo-core"))]
fn validate_thread_id(preview: &Value, stage: &str) -> Option<ValidationFailure> {
    let requires_thread_id = matches!(
        stage,
        "thread_started_notification"
            | "thread_resume_request"
            | "thread_fork_request"
            | "turn_start_request"
    );
    (requires_thread_id && preview_thread_id(preview).is_none())
        .then(|| payload_failure(preview, "lifecycle_missing_thread_id"))
}

#[cfg(not(feature = "mojo-core"))]
fn validate_thread_started(
    preview: &Value,
    frame: Option<&Value>,
    stage: &str,
) -> Option<ValidationFailure> {
    if stage != "thread_started_notification" {
        return None;
    }
    if frame
        .and_then(|frame| frame_string(frame, &["params", "thread", "id"]))
        .is_none()
    {
        return Some(payload_failure(
            preview,
            "lifecycle_missing_thread_object_id",
        ));
    }
    if let Some(reason) = frame
        .map(|frame| thread_status_failure_reason(frame, &["params", "thread", "status"]))
        .unwrap_or(Some("lifecycle_missing_thread_status"))
    {
        return Some(payload_failure(preview, reason));
    }
    (!frame.is_some_and(|frame| thread_object_has_context(frame, &["params", "thread"])))
        .then(|| payload_failure(preview, "lifecycle_missing_thread_context"))
}

#[cfg(not(feature = "mojo-core"))]
fn validate_turn_input(
    preview: &Value,
    frame: Option<&Value>,
    stage: &str,
) -> Option<ValidationFailure> {
    if stage != "turn_start_request" {
        return None;
    }
    let has_input = frame
        .and_then(|frame| frame.get("params"))
        .and_then(|params| params.get("input"))
        .is_some_and(Value::is_array);
    (!has_input).then(|| payload_failure(preview, "lifecycle_missing_turn_input"))
}

#[cfg(not(feature = "mojo-core"))]
fn validate_turn_status(
    preview: &Value,
    frame: Option<&Value>,
    stage: &str,
) -> Option<ValidationFailure> {
    if !matches!(
        stage,
        "turn_started_notification" | "turn_completed_notification"
    ) {
        return None;
    }
    let turn_status = frame
        .and_then(|frame| frame.get("params"))
        .and_then(|params| params.get("turn"))
        .and_then(|turn| turn.get("status"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|status| !status.is_empty());
    match turn_status {
        Some(status) if is_valid_turn_status(status) => {}
        Some(_) => return Some(payload_failure(preview, "lifecycle_invalid_turn_status")),
        None => return Some(payload_failure(preview, "lifecycle_missing_turn_status")),
    }
    let has_items = frame
        .and_then(|frame| frame.get("params"))
        .and_then(|params| params.get("turn"))
        .and_then(|turn| turn.get("items"))
        .is_some_and(Value::is_array);
    (!has_items).then(|| payload_failure(preview, "lifecycle_missing_turn_items"))
}

#[cfg(feature = "mojo-core")]
pub(super) fn frame_thread_status_type<'a>(
    frame: Option<&'a Value>,
    path: &[&str],
) -> Option<&'a str> {
    frame
        .and_then(|frame| frame_value(frame, path))
        .and_then(|status| status.get("type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|status| !status.is_empty())
}

#[cfg(feature = "mojo-core")]
pub(super) fn frame_turn_status<'a>(frame: Option<&'a Value>, path: &[&str]) -> Option<&'a str> {
    frame
        .and_then(|frame| frame_value(frame, path))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|status| !status.is_empty())
}

#[cfg(feature = "mojo-core")]
pub(super) fn frame_active_flags_valid(frame: Option<&Value>, path: &[&str]) -> bool {
    let Some(status) = frame.and_then(|frame| frame_value(frame, path)) else {
        return true;
    };
    let Some(status_type) = status.get("type").and_then(Value::as_str) else {
        return true;
    };
    status_type.trim() != "active"
        || status
            .get("activeFlags")
            .and_then(Value::as_array)
            .is_some_and(|flags| {
                flags.iter().all(|flag| {
                    matches!(
                        flag.as_str(),
                        Some("waitingOnApproval" | "waitingOnUserInput")
                    )
                })
            })
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

#[cfg(not(feature = "mojo-core"))]
pub(super) fn is_valid_turn_status(status: &str) -> bool {
    matches!(
        status,
        "completed" | "interrupted" | "failed" | "inProgress"
    )
}

#[cfg(not(feature = "mojo-core"))]
fn is_valid_thread_status(status: &str) -> bool {
    matches!(status, "notLoaded" | "idle" | "systemError" | "active")
}

#[cfg(not(feature = "mojo-core"))]
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

#[cfg(not(feature = "mojo-core"))]
fn is_valid_thread_active_flag(flag: &str) -> bool {
    matches!(flag, "waitingOnApproval" | "waitingOnUserInput")
}

fn payload_failure(preview: &Value, reason: &'static str) -> ValidationFailure {
    let stage = preview["preview"]["summary"]["lifecycle_stage"]
        .as_str()
        .unwrap_or("unknown");
    ValidationFailure::from_preview(preview, reason).lifecycle_stage(stage)
}

use super::super::super::app_server_broker_protocol::app_server_broker_lifecycle_response_schema_file;
#[cfg(feature = "mojo-core")]
use super::super::super::app_server_broker_protocol::app_server_broker_mojo_validation_reason;
#[cfg(feature = "mojo-core")]
use super::payload::{
    frame_active_flags_valid, frame_thread_status_type, frame_turn_status,
    thread_object_has_context, thread_response_has_context, thread_response_has_valid_context,
};
use super::payload::{frame_string, frame_value, preview_id_key};
#[cfg(not(feature = "mojo-core"))]
use super::payload::{
    is_valid_turn_status, thread_object_has_context, thread_response_has_context,
    thread_response_has_valid_context, thread_status_failure_reason,
};
use super::{APP_SERVER_BROKER_MAX_ACTIVE_VALIDATION_ITEMS, ProtocolDirection, ValidationFailure};
#[cfg(feature = "mojo-core")]
use prodex_mojo_core::rich::AppServerBrokerValidationInput;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct RequestResponseValidation {
    pending_requests: HashMap<(ProtocolDirection, String), Option<String>>,
}

impl RequestResponseValidation {
    pub(super) fn annotate_response_schema(
        &self,
        preview: &mut Value,
        frame: Option<&Value>,
        direction: ProtocolDirection,
    ) {
        if preview["preview"]["summary"]["frame_kind"].as_str() != Some("response") {
            return;
        }
        if !frame.is_some_and(|frame| frame.get("result").is_some() && frame.get("error").is_none())
        {
            return;
        }
        let Some(id) = preview_id_key(preview) else {
            return;
        };
        let key = (direction.requester_for_response(), id);
        let Some(Some(lifecycle_stage)) = self.pending_requests.get(&key) else {
            return;
        };
        if let Some(schema_file) = app_server_broker_lifecycle_response_schema_file(lifecycle_stage)
        {
            preview["preview"]["summary"]["lifecycle_schema_file"] =
                Value::String(schema_file.to_string());
        }
    }

    pub(super) fn observe_for_schema_tracking(&mut self, preview: &Value) {
        if !preview["preview"]["parse_ok"].as_bool().unwrap_or_default() {
            return;
        }
        match preview["preview"]["summary"]["frame_kind"].as_str() {
            Some("request") => {
                if let Some(id) = preview_id_key(preview) {
                    let lifecycle_stage = preview["preview"]["summary"]["lifecycle_stage"]
                        .as_str()
                        .map(str::to_string);
                    if self.pending_requests.len() < APP_SERVER_BROKER_MAX_ACTIVE_VALIDATION_ITEMS
                        || self
                            .pending_requests
                            .contains_key(&(ProtocolDirection::SingleStream, id.clone()))
                    {
                        self.pending_requests
                            .insert((ProtocolDirection::SingleStream, id), lifecycle_stage);
                    }
                }
            }
            Some("response") => {
                if let Some(id) = preview_id_key(preview) {
                    self.pending_requests
                        .remove(&(ProtocolDirection::SingleStream, id));
                }
            }
            _ => {}
        }
    }

    pub(super) fn observe_preview_and_frame(
        &mut self,
        preview: &Value,
        frame: Option<&Value>,
        direction: ProtocolDirection,
    ) -> Option<ValidationFailure> {
        if !preview["preview"]["parse_ok"].as_bool().unwrap_or_default() {
            return None;
        }
        match preview["preview"]["summary"]["frame_kind"].as_str()? {
            "request" => self.observe_request(preview, direction),
            "response" => self.observe_response(preview, frame, direction),
            _ => None,
        }
    }

    fn observe_request(
        &mut self,
        preview: &Value,
        direction: ProtocolDirection,
    ) -> Option<ValidationFailure> {
        let Some(id) = preview_id_key(preview) else {
            return Some(ValidationFailure::from_preview(
                preview,
                "request_missing_id",
            ));
        };
        let lifecycle_stage = preview["preview"]["summary"]["lifecycle_stage"]
            .as_str()
            .map(str::to_string);
        let key = (direction, id.clone());
        if self.pending_requests.contains_key(&key) {
            return Some(
                ValidationFailure::from_preview(preview, "duplicate_pending_request_id")
                    .request_id(id),
            );
        }
        if self.pending_requests.len() >= APP_SERVER_BROKER_MAX_ACTIVE_VALIDATION_ITEMS {
            return Some(
                ValidationFailure::from_preview(preview, "pending_request_limit_exceeded")
                    .request_id(id),
            );
        }
        self.pending_requests.insert(key, lifecycle_stage);
        None
    }

    fn observe_response(
        &mut self,
        preview: &Value,
        frame: Option<&Value>,
        direction: ProtocolDirection,
    ) -> Option<ValidationFailure> {
        let Some(id) = preview_id_key(preview) else {
            return Some(ValidationFailure::from_preview(
                preview,
                "response_missing_id",
            ));
        };
        let key = (direction.requester_for_response(), id.clone());
        let Some(lifecycle_stage) = self.pending_requests.remove(&key) else {
            return Some(
                ValidationFailure::from_preview(preview, "response_without_request").request_id(id),
            );
        };
        self.validate_lifecycle_response(preview, frame, lifecycle_stage.as_deref())
    }

    fn validate_lifecycle_response(
        &self,
        preview: &Value,
        frame: Option<&Value>,
        lifecycle_stage: Option<&str>,
    ) -> Option<ValidationFailure> {
        let frame = frame?;
        if frame.get("error").is_some() {
            return None;
        }
        #[cfg(feature = "mojo-core")]
        {
            let stage = lifecycle_stage?;
            return app_server_broker_mojo_validation_reason(AppServerBrokerValidationInput {
                response: true,
                stage,
                thread_id_present: frame_string(frame, &["result", "thread", "id"]).is_some(),
                thread_object_id_present: false,
                thread_status: frame_thread_status_type(
                    Some(frame),
                    &["result", "thread", "status"],
                ),
                thread_active_flags_valid: frame_active_flags_valid(
                    Some(frame),
                    &["result", "thread", "status"],
                ),
                thread_object_context: false,
                response_thread_context: thread_response_has_context(frame),
                response_thread_context_valid: thread_response_has_valid_context(frame),
                response_thread_object_context: thread_object_has_context(
                    frame,
                    &["result", "thread"],
                ),
                turn_input: false,
                turn_id_present: frame_string(frame, &["result", "turn", "id"]).is_some(),
                turn_status: frame_turn_status(Some(frame), &["result", "turn", "status"]),
                turn_items: frame_value(frame, &["result", "turn", "items"])
                    .is_some_and(Value::is_array),
            })
            .map(|reason| ValidationFailure::from_preview(preview, reason));
        }
        #[cfg(not(feature = "mojo-core"))]
        {
            match lifecycle_stage {
                Some("thread_start_request" | "thread_resume_request" | "thread_fork_request") => {
                    validate_thread_response(preview, frame)
                }
                Some("turn_start_request") => validate_turn_response(preview, frame),
                _ => None,
            }
        }
    }

    pub(super) fn finish(&self, line_index: usize) -> Option<ValidationFailure> {
        let (_, id) = self.pending_requests.keys().min()?;
        Some(
            ValidationFailure::at_eof(line_index, "pending_request_without_response")
                .request_id(id),
        )
    }
}

#[cfg(not(feature = "mojo-core"))]
fn validate_thread_response(preview: &Value, frame: &Value) -> Option<ValidationFailure> {
    if frame_string(frame, &["result", "thread", "id"]).is_none() {
        return Some(ValidationFailure::from_preview(
            preview,
            "lifecycle_response_missing_thread_id",
        ));
    }
    if let Some(reason) = thread_status_failure_reason(frame, &["result", "thread", "status"]) {
        return Some(ValidationFailure::from_preview(
            preview,
            match reason {
                "lifecycle_invalid_thread_status" => "lifecycle_response_invalid_thread_status",
                _ => "lifecycle_response_missing_thread_status",
            },
        ));
    }
    for (valid, reason) in [
        (
            thread_response_has_context(frame),
            "lifecycle_response_missing_thread_context",
        ),
        (
            thread_response_has_valid_context(frame),
            "lifecycle_response_invalid_thread_context",
        ),
        (
            thread_object_has_context(frame, &["result", "thread"]),
            "lifecycle_response_missing_thread_object_context",
        ),
    ] {
        if !valid {
            return Some(ValidationFailure::from_preview(preview, reason));
        }
    }
    None
}

#[cfg(not(feature = "mojo-core"))]
fn validate_turn_response(preview: &Value, frame: &Value) -> Option<ValidationFailure> {
    if frame_string(frame, &["result", "turn", "id"]).is_none() {
        return Some(ValidationFailure::from_preview(
            preview,
            "lifecycle_response_missing_turn_id",
        ));
    }
    match frame_string(frame, &["result", "turn", "status"]).as_deref() {
        Some(status) if is_valid_turn_status(status) => {}
        Some(_) => {
            return Some(ValidationFailure::from_preview(
                preview,
                "lifecycle_response_invalid_turn_status",
            ));
        }
        None => {
            return Some(ValidationFailure::from_preview(
                preview,
                "lifecycle_response_missing_turn_status",
            ));
        }
    }
    (!frame_value(frame, &["result", "turn", "items"]).is_some_and(Value::is_array))
        .then(|| ValidationFailure::from_preview(preview, "lifecycle_response_missing_turn_items"))
}

use super::super::super::app_server_broker_protocol::app_server_broker_lifecycle_response_schema_file;
use super::ValidationFailure;
use super::payload::{
    frame_string, frame_value, is_valid_turn_status, preview_id_key, thread_object_has_context,
    thread_response_has_context, thread_response_has_valid_context, thread_status_failure_reason,
};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct RequestResponseValidation {
    pending_requests: HashMap<String, Option<String>>,
}

impl RequestResponseValidation {
    pub(super) fn annotate_response_schema(&self, preview: &mut Value, frame: Option<&Value>) {
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
        let Some(Some(lifecycle_stage)) = self.pending_requests.get(&id) else {
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
                    self.pending_requests.insert(id, lifecycle_stage);
                }
            }
            Some("response") => {
                if let Some(id) = preview_id_key(preview) {
                    self.pending_requests.remove(&id);
                }
            }
            _ => {}
        }
    }

    pub(super) fn observe_preview_and_frame(
        &mut self,
        preview: &Value,
        frame: Option<&Value>,
    ) -> Option<ValidationFailure> {
        if !preview["preview"]["parse_ok"].as_bool().unwrap_or_default() {
            return None;
        }
        match preview["preview"]["summary"]["frame_kind"].as_str()? {
            "request" => self.observe_request(preview),
            "response" => self.observe_response(preview, frame),
            _ => None,
        }
    }

    fn observe_request(&mut self, preview: &Value) -> Option<ValidationFailure> {
        let Some(id) = preview_id_key(preview) else {
            return Some(ValidationFailure::from_preview(
                preview,
                "request_missing_id",
            ));
        };
        let lifecycle_stage = preview["preview"]["summary"]["lifecycle_stage"]
            .as_str()
            .map(str::to_string);
        if self
            .pending_requests
            .insert(id.clone(), lifecycle_stage)
            .is_some()
        {
            return Some(
                ValidationFailure::from_preview(preview, "duplicate_pending_request_id")
                    .request_id(id),
            );
        }
        None
    }

    fn observe_response(
        &mut self,
        preview: &Value,
        frame: Option<&Value>,
    ) -> Option<ValidationFailure> {
        let Some(id) = preview_id_key(preview) else {
            return Some(ValidationFailure::from_preview(
                preview,
                "response_missing_id",
            ));
        };
        let Some(lifecycle_stage) = self.pending_requests.remove(&id) else {
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
        match lifecycle_stage {
            Some("thread_start_request" | "thread_resume_request" | "thread_fork_request") => {
                if frame_string(frame, &["result", "thread", "id"]).is_none() {
                    return Some(ValidationFailure::from_preview(
                        preview,
                        "lifecycle_response_missing_thread_id",
                    ));
                }
                if let Some(reason) =
                    thread_status_failure_reason(frame, &["result", "thread", "status"])
                {
                    return Some(ValidationFailure::from_preview(
                        preview,
                        match reason {
                            "lifecycle_invalid_thread_status" => {
                                "lifecycle_response_invalid_thread_status"
                            }
                            _ => "lifecycle_response_missing_thread_status",
                        },
                    ));
                }
                if !thread_response_has_context(frame) {
                    return Some(ValidationFailure::from_preview(
                        preview,
                        "lifecycle_response_missing_thread_context",
                    ));
                }
                if !thread_response_has_valid_context(frame) {
                    return Some(ValidationFailure::from_preview(
                        preview,
                        "lifecycle_response_invalid_thread_context",
                    ));
                }
                if !thread_object_has_context(frame, &["result", "thread"]) {
                    return Some(ValidationFailure::from_preview(
                        preview,
                        "lifecycle_response_missing_thread_object_context",
                    ));
                }
            }
            Some("turn_start_request") => {
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
                if !frame_value(frame, &["result", "turn", "items"]).is_some_and(Value::is_array) {
                    return Some(ValidationFailure::from_preview(
                        preview,
                        "lifecycle_response_missing_turn_items",
                    ));
                }
            }
            _ => {}
        }
        None
    }

    pub(super) fn finish(&self, line_index: usize) -> Option<ValidationFailure> {
        let id = self.pending_requests.keys().min()?;
        Some(
            ValidationFailure::at_eof(line_index, "pending_request_without_response")
                .request_id(id),
        )
    }
}

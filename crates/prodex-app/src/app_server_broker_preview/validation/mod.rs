mod lifecycle;
mod payload;
mod request_response;

use super::parse::app_server_broker_preview_line;
use super::report::app_server_broker_preview_report_from_previews;
use lifecycle::LifecycleValidation;
use payload::LifecyclePayloadValidation;
use request_response::RequestResponseValidation;
use serde_json::Value;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ValidationFailure {
    pub(super) reason: &'static str,
    pub(super) line: u64,
    pub(super) frame_kind: Option<String>,
    pub(super) request_id: Option<String>,
    pub(super) lifecycle_stage: Option<String>,
    pub(super) thread_id: Option<String>,
    pub(super) active_turn_id: Option<String>,
    pub(super) turn_id: Option<String>,
}

impl ValidationFailure {
    pub(super) fn from_preview(preview: &Value, reason: &'static str) -> Self {
        Self {
            reason,
            line: preview["line"].as_u64().unwrap_or_default(),
            frame_kind: preview["preview"]["summary"]["frame_kind"]
                .as_str()
                .map(str::to_string),
            request_id: None,
            lifecycle_stage: None,
            thread_id: None,
            active_turn_id: None,
            turn_id: None,
        }
    }

    pub(super) fn at_eof(line: usize, reason: &'static str) -> Self {
        Self {
            reason,
            line: line as u64,
            frame_kind: None,
            request_id: None,
            lifecycle_stage: None,
            thread_id: None,
            active_turn_id: None,
            turn_id: None,
        }
    }

    pub(super) fn request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub(super) fn lifecycle_stage(mut self, lifecycle_stage: impl Into<String>) -> Self {
        self.lifecycle_stage = Some(lifecycle_stage.into());
        self
    }

    pub(super) fn thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    pub(super) fn active_turn_id(mut self, active_turn_id: impl Into<String>) -> Self {
        self.active_turn_id = Some(active_turn_id.into());
        self
    }

    pub(super) fn turn_id(mut self, turn_id: impl Into<String>) -> Self {
        self.turn_id = Some(turn_id.into());
        self
    }
}

impl fmt::Display for ValidationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = &self.frame_kind;
        write!(formatter, "line={} reason={}", self.line, self.reason)?;
        if let Some(request_id) = &self.request_id {
            write!(formatter, " id={request_id}")?;
        }
        if let Some(lifecycle_stage) = &self.lifecycle_stage {
            write!(formatter, " lifecycle_stage={lifecycle_stage}")?;
        }
        if let Some(thread_id) = &self.thread_id {
            write!(formatter, " thread_id={thread_id}")?;
        }
        if let Some(active_turn_id) = &self.active_turn_id {
            write!(formatter, " active_turn_id={active_turn_id}")?;
        }
        if let Some(turn_id) = &self.turn_id {
            write!(formatter, " turn_id={turn_id}")?;
        }
        Ok(())
    }
}

pub(super) struct PreviewObservation {
    pub(super) preview: Value,
    pub(super) lifecycle_failure: Option<ValidationFailure>,
    pub(super) request_response_failure: Option<ValidationFailure>,
    pub(super) lifecycle_payload_failure: Option<ValidationFailure>,
}

#[derive(Default)]
pub(super) struct PreviewSession {
    previews: Vec<Value>,
    request_response: RequestResponseValidation,
    lifecycle: LifecycleValidation,
    lifecycle_payload: LifecyclePayloadValidation,
}

impl PreviewSession {
    fn parsed_preview(&self, line_index: usize, line: &str) -> Value {
        serde_json::json!({
            "object": "app_server_broker.preview_event",
            "line": line_index,
            "preview": app_server_broker_preview_line(line),
        })
    }

    pub(super) fn observe_line(&mut self, line_index: usize, line: &str) -> Value {
        let mut preview = self.parsed_preview(line_index, line);
        let frame = serde_json::from_str::<Value>(line).ok();
        self.request_response
            .annotate_response_schema(&mut preview, frame.as_ref());
        self.request_response.observe_for_schema_tracking(&preview);
        self.previews.push(preview.clone());
        preview
    }

    pub(super) fn validate_line(&mut self, line_index: usize, line: &str) -> PreviewObservation {
        let mut preview = self.parsed_preview(line_index, line);
        let frame = serde_json::from_str::<Value>(line).ok();
        self.request_response
            .annotate_response_schema(&mut preview, frame.as_ref());
        let lifecycle_failure = self.lifecycle.observe_preview(&preview);
        let request_response_failure = self
            .request_response
            .observe_preview_and_frame(&preview, frame.as_ref());
        let lifecycle_payload_failure = self
            .lifecycle_payload
            .observe_preview_and_frame(&preview, frame.as_ref());
        self.previews.push(preview.clone());
        PreviewObservation {
            preview,
            lifecycle_failure,
            request_response_failure,
            lifecycle_payload_failure,
        }
    }

    pub(super) fn finish(&self, line_index: usize) -> Option<ValidationFailure> {
        self.request_response.finish(line_index)
    }

    #[cfg(test)]
    pub(super) fn into_previews(self) -> Vec<Value> {
        self.previews
    }

    pub(super) fn into_report_json(self) -> Value {
        serde_json::json!({
            "object": "app_server_broker.preview_report",
            "report": app_server_broker_preview_report_from_previews(self.previews),
        })
    }
}

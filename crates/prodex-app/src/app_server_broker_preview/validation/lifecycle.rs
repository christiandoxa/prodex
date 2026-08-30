#[cfg(feature = "mojo-core")]
use super::super::super::app_server_broker_protocol::app_server_broker_mojo_lifecycle_sequence_reason;
use super::payload::{preview_thread_id, preview_turn_id};
use super::{APP_SERVER_BROKER_MAX_ACTIVE_VALIDATION_ITEMS, ValidationFailure};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Default)]
pub(super) struct LifecycleValidation {
    active_turn_by_thread: HashMap<String, String>,
    started_turns: HashSet<String>,
    completed_turns: HashSet<String>,
    completed_turn_order: VecDeque<String>,
}

impl LifecycleValidation {
    pub(super) fn observe_preview(&mut self, preview: &Value) -> Option<ValidationFailure> {
        if !preview["preview"]["parse_ok"].as_bool().unwrap_or_default() {
            return None;
        }
        match preview["preview"]["summary"]["lifecycle_stage"].as_str()? {
            "turn_started_notification" => self.observe_turn_started(preview),
            "turn_completed_notification" => self.observe_turn_completed(preview),
            "turn_interrupt_request" => self.observe_turn_interrupt(preview),
            _ => None,
        }
    }

    #[cfg(not(feature = "mojo-core"))]
    fn observe_turn_started(&mut self, preview: &Value) -> Option<ValidationFailure> {
        let Some(turn_id) = preview_turn_id(preview) else {
            return Some(ValidationFailure::from_preview(
                preview,
                "turn_started_missing_turn_id",
            ));
        };
        let Some(thread_id) = preview_thread_id(preview) else {
            return Some(
                ValidationFailure::from_preview(preview, "turn_started_missing_thread_id")
                    .turn_id(turn_id),
            );
        };
        if self.completed_turns.contains(&turn_id) {
            return Some(
                ValidationFailure::from_preview(preview, "turn_started_after_completed")
                    .turn_id(turn_id),
            );
        }
        if let Some(active_turn_id) = self.active_turn_by_thread.get(&thread_id)
            && active_turn_id != &turn_id
        {
            return Some(
                ValidationFailure::from_preview(preview, "thread_active_turn_conflict")
                    .thread_id(thread_id)
                    .active_turn_id(active_turn_id)
                    .turn_id(turn_id),
            );
        }
        if !self.started_turns.insert(turn_id.clone()) {
            return Some(
                ValidationFailure::from_preview(preview, "duplicate_turn_started").turn_id(turn_id),
            );
        }
        if self.started_turns.len() > APP_SERVER_BROKER_MAX_ACTIVE_VALIDATION_ITEMS {
            self.started_turns.remove(&turn_id);
            return Some(
                ValidationFailure::from_preview(preview, "active_turn_limit_exceeded")
                    .thread_id(thread_id)
                    .turn_id(turn_id),
            );
        }
        self.active_turn_by_thread.insert(thread_id, turn_id);
        None
    }

    #[cfg(feature = "mojo-core")]
    fn observe_turn_started(&mut self, preview: &Value) -> Option<ValidationFailure> {
        let turn_id = preview_turn_id(preview);
        let thread_id = preview_thread_id(preview);
        let active_turn_id = thread_id
            .as_ref()
            .and_then(|thread_id| self.active_turn_by_thread.get(thread_id))
            .cloned();
        if let Some(reason) = app_server_broker_mojo_lifecycle_sequence_reason(
            "turn_started_notification",
            turn_id.is_some(),
            thread_id.is_some(),
            turn_id
                .as_ref()
                .is_some_and(|turn_id| self.completed_turns.contains(turn_id)),
            active_turn_id.is_some(),
            active_turn_id
                .as_ref()
                .is_some_and(|active| turn_id.as_ref().is_some_and(|turn| active == turn)),
            turn_id
                .as_ref()
                .is_some_and(|turn_id| self.started_turns.contains(turn_id)),
        ) {
            return Some(sequence_failure(
                preview,
                reason,
                turn_id,
                thread_id,
                active_turn_id,
            ));
        }
        let Some(turn_id) = turn_id else {
            return Some(ValidationFailure::from_preview(
                preview,
                "turn_started_missing_turn_id",
            ));
        };
        let Some(thread_id) = thread_id else {
            return Some(
                ValidationFailure::from_preview(preview, "turn_started_missing_thread_id")
                    .turn_id(turn_id),
            );
        };
        let _ = self.started_turns.insert(turn_id.clone());
        if self.started_turns.len() > APP_SERVER_BROKER_MAX_ACTIVE_VALIDATION_ITEMS {
            self.started_turns.remove(&turn_id);
            return Some(
                ValidationFailure::from_preview(preview, "active_turn_limit_exceeded")
                    .thread_id(thread_id)
                    .turn_id(turn_id),
            );
        }
        self.active_turn_by_thread.insert(thread_id, turn_id);
        None
    }

    #[cfg(not(feature = "mojo-core"))]
    fn observe_turn_completed(&mut self, preview: &Value) -> Option<ValidationFailure> {
        let Some(turn_id) = preview_turn_id(preview) else {
            return Some(ValidationFailure::from_preview(
                preview,
                "turn_completed_missing_turn_id",
            ));
        };
        let Some(thread_id) = preview_thread_id(preview) else {
            return Some(
                ValidationFailure::from_preview(preview, "turn_completed_missing_thread_id")
                    .turn_id(turn_id),
            );
        };
        if !self.started_turns.contains(&turn_id) {
            return Some(
                ValidationFailure::from_preview(preview, "turn_completed_without_turn_started")
                    .turn_id(turn_id),
            );
        }
        if self
            .active_turn_by_thread
            .get(&thread_id)
            .is_some_and(|active_turn_id| active_turn_id != &turn_id)
        {
            return Some(
                ValidationFailure::from_preview(preview, "turn_completed_not_active")
                    .thread_id(thread_id)
                    .turn_id(turn_id),
            );
        }
        if !self.completed_turns.insert(turn_id.clone()) {
            return Some(
                ValidationFailure::from_preview(preview, "duplicate_turn_completed")
                    .turn_id(turn_id),
            );
        }
        self.started_turns.remove(&turn_id);
        self.completed_turn_order.push_back(turn_id.clone());
        // ponytail: detect recent replay IDs with bounded memory; persist protocol history if
        // replay detection must span more than one long-running validation window.
        if self.completed_turn_order.len() > APP_SERVER_BROKER_MAX_ACTIVE_VALIDATION_ITEMS
            && let Some(expired) = self.completed_turn_order.pop_front()
        {
            self.completed_turns.remove(&expired);
        }
        self.active_turn_by_thread.remove(&thread_id);
        None
    }

    #[cfg(feature = "mojo-core")]
    fn observe_turn_completed(&mut self, preview: &Value) -> Option<ValidationFailure> {
        let turn_id = preview_turn_id(preview);
        let thread_id = preview_thread_id(preview);
        let active_turn_id = thread_id
            .as_ref()
            .and_then(|thread_id| self.active_turn_by_thread.get(thread_id))
            .cloned();
        if let Some(reason) = app_server_broker_mojo_lifecycle_sequence_reason(
            "turn_completed_notification",
            turn_id.is_some(),
            thread_id.is_some(),
            turn_id
                .as_ref()
                .is_some_and(|turn_id| self.completed_turns.contains(turn_id)),
            active_turn_id.is_some(),
            active_turn_id
                .as_ref()
                .is_some_and(|active| turn_id.as_ref().is_some_and(|turn| active == turn)),
            turn_id
                .as_ref()
                .is_some_and(|turn_id| self.started_turns.contains(turn_id)),
        ) {
            return Some(sequence_failure(
                preview,
                reason,
                turn_id,
                thread_id,
                active_turn_id,
            ));
        }
        let Some(turn_id) = turn_id else {
            return Some(ValidationFailure::from_preview(
                preview,
                "turn_completed_missing_turn_id",
            ));
        };
        let Some(thread_id) = thread_id else {
            return Some(
                ValidationFailure::from_preview(preview, "turn_completed_missing_thread_id")
                    .turn_id(turn_id),
            );
        };
        let _ = self.completed_turns.insert(turn_id.clone());
        self.started_turns.remove(&turn_id);
        self.completed_turn_order.push_back(turn_id.clone());
        if self.completed_turn_order.len() > APP_SERVER_BROKER_MAX_ACTIVE_VALIDATION_ITEMS
            && let Some(expired) = self.completed_turn_order.pop_front()
        {
            self.completed_turns.remove(&expired);
        }
        self.active_turn_by_thread.remove(&thread_id);
        None
    }

    #[cfg(not(feature = "mojo-core"))]
    fn observe_turn_interrupt(&mut self, preview: &Value) -> Option<ValidationFailure> {
        let Some(turn_id) = preview_turn_id(preview) else {
            return Some(ValidationFailure::from_preview(
                preview,
                "turn_interrupt_missing_turn_id",
            ));
        };
        let Some(thread_id) = preview_thread_id(preview) else {
            return Some(
                ValidationFailure::from_preview(preview, "turn_interrupt_missing_thread_id")
                    .turn_id(turn_id),
            );
        };
        if let Some(active_turn_id) = self.active_turn_by_thread.get(&thread_id) {
            if active_turn_id != &turn_id {
                return Some(
                    ValidationFailure::from_preview(preview, "turn_interrupt_active_turn_conflict")
                        .thread_id(thread_id)
                        .active_turn_id(active_turn_id)
                        .turn_id(turn_id),
                );
            }
            self.active_turn_by_thread.remove(&thread_id);
        }
        None
    }

    #[cfg(feature = "mojo-core")]
    fn observe_turn_interrupt(&mut self, preview: &Value) -> Option<ValidationFailure> {
        let turn_id = preview_turn_id(preview);
        let thread_id = preview_thread_id(preview);
        let active_turn_id = thread_id
            .as_ref()
            .and_then(|thread_id| self.active_turn_by_thread.get(thread_id))
            .cloned();
        if let Some(reason) = app_server_broker_mojo_lifecycle_sequence_reason(
            "turn_interrupt_request",
            turn_id.is_some(),
            thread_id.is_some(),
            false,
            active_turn_id.is_some(),
            active_turn_id
                .as_ref()
                .is_some_and(|active| turn_id.as_ref().is_some_and(|turn| active == turn)),
            false,
        ) {
            return Some(sequence_failure(
                preview,
                reason,
                turn_id,
                thread_id,
                active_turn_id,
            ));
        }
        let (Some(turn_id), Some(thread_id)) = (turn_id, thread_id) else {
            return Some(ValidationFailure::from_preview(
                preview,
                "turn_interrupt_missing_turn_id",
            ));
        };
        if active_turn_id
            .as_ref()
            .is_some_and(|active| active == &turn_id)
        {
            self.active_turn_by_thread.remove(&thread_id);
        }
        None
    }
}

#[cfg(feature = "mojo-core")]
fn sequence_failure(
    preview: &Value,
    reason: &'static str,
    turn_id: Option<String>,
    thread_id: Option<String>,
    active_turn_id: Option<String>,
) -> ValidationFailure {
    let mut failure = ValidationFailure::from_preview(preview, reason);
    if matches!(
        reason,
        "turn_started_missing_thread_id"
            | "turn_completed_missing_thread_id"
            | "turn_interrupt_missing_thread_id"
            | "turn_completed_without_turn_started"
            | "turn_started_after_completed"
            | "duplicate_turn_started"
            | "duplicate_turn_completed"
            | "turn_started_missing_turn_id"
            | "turn_completed_missing_turn_id"
            | "turn_interrupt_missing_turn_id"
    ) {
        if let Some(turn_id) = turn_id.as_ref() {
            failure = failure.turn_id(turn_id.clone());
        }
    }
    if matches!(
        reason,
        "thread_active_turn_conflict"
            | "turn_completed_not_active"
            | "turn_interrupt_active_turn_conflict"
    ) {
        if let Some(thread_id) = thread_id.as_ref() {
            failure = failure.thread_id(thread_id.clone());
        }
        if let Some(active_turn_id) = active_turn_id.as_ref() {
            failure = failure.active_turn_id(active_turn_id.clone());
        }
        if let Some(turn_id) = turn_id.as_ref() {
            failure = failure.turn_id(turn_id.clone());
        }
    }
    failure
}

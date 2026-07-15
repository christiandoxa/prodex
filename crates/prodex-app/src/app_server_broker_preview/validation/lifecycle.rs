use super::ValidationFailure;
use super::payload::{preview_thread_id, preview_turn_id};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct LifecycleValidation {
    active_turn_by_thread: HashMap<String, String>,
    started_turns: HashSet<String>,
    completed_turns: HashSet<String>,
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
        self.active_turn_by_thread.insert(thread_id, turn_id);
        None
    }

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
        self.active_turn_by_thread.remove(&thread_id);
        None
    }

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
}

//! Gemini SSE completion and final-response guardrail helpers.

use super::*;

impl RuntimeGeminiSseState {
    fn apply_forced_output_text(&mut self) {
        if let Some(text) = self.forced_output_text.clone() {
            self.output_text = text;
            self.tool_calls.clear();
            self.tool_call_indices_by_id.clear();
        }
    }

    pub(in super::super) fn complete_event(&mut self) -> Option<String> {
        if self.completed {
            return None;
        }
        self.apply_forced_output_text();
        let mut events = self.flush_pending_output_text_events();
        events.extend(self.complete_tool_call_events());
        events.extend(self.complete_output_text_item_events());
        events.extend(self.complete_media_item_events());
        events.extend(self.complete_image_generation_item_events());
        let output = self.output_items();
        if output.is_empty() && self.reasoning_content.is_empty() {
            let suffix = self
                .finish_reason
                .as_deref()
                .map(|reason| format!(" finishReason={reason}"))
                .unwrap_or_default();
            if let Some(event) = self.failed_event(
                "gemini_empty_response",
                &format!("Gemini returned no visible response content.{suffix}"),
            ) {
                events.push(event);
            }
            return Some(events.join(""));
        }
        if self.tool_calls.is_empty()
            && let Some(tool_name) = runtime_gemini_tool_intent_without_call(&self.output_text)
        {
            if let Some(event) = self.failed_event(
                "gemini_tool_intent_without_call",
                &format!(
                    "Gemini stopped after announcing a `{tool_name}` tool call instead of emitting the tool call."
                ),
            ) {
                events.push(event);
            }
            return Some(events.join(""));
        }
        if self.tool_calls.is_empty()
            && let Some(reason) = runtime_gemini_non_actionable_wait_or_poll_text(&self.output_text)
        {
            if let Some(event) = self.failed_event(
                "gemini_non_actionable_wait_or_poll",
                &format!(
                    "Gemini stopped with non-actionable wait/poll narration instead of waiting on the running tool session: {reason}."
                ),
            ) {
                events.push(event);
            }
            return Some(events.join(""));
        }
        if self.tool_calls.is_empty()
            && runtime_gemini_unverified_success_claim(
                &self.output_text,
                &self.conversation_messages,
            )
        {
            if let Some(event) = self.failed_event(
                "gemini_unverified_success_claim",
                "Gemini made a success/up-to-date/no-blocker final claim without a clean verification tool result after the relevant action.",
            ) {
                events.push(event);
            }
            return Some(events.join(""));
        }
        let model = self.model.as_deref().unwrap_or(GEMINI_DEFAULT_MODEL);
        let response = gemini_provider_core_stream_response_value(
            &self.response_id,
            output,
            Some(model),
            self.usage.clone(),
            self.metadata.clone(),
        );
        let sequence_number = self.next_sequence_number();
        events.push(self.event(
            "response.completed",
            gemini_provider_core_response_completed_event(
                sequence_number,
                self.created_at,
                &response,
            ),
        ));
        self.store_conversation_snapshot();
        if let Some(recorder) = &self.binding_recorder {
            recorder(self.response_id.clone(), self.tool_call_ids());
        }
        self.completed = true;
        Some(events.join(""))
    }

    pub(super) fn flush_pending_output_text_events(&mut self) -> Vec<String> {
        if self.pending_output_text.is_empty() {
            return Vec::new();
        }
        let text = std::mem::take(&mut self.pending_output_text);
        if gemini_provider_core_text_echoes_internal_instruction(
            &text,
            &self.internal_instruction_corpus,
        ) {
            Vec::new()
        } else {
            self.observe_output_text(&text)
        }
    }
}

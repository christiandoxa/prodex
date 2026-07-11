//! Gemini SSE event-emission helpers.

use super::*;

impl RuntimeGeminiSseState {
    fn output_text_item_id(&self) -> String {
        gemini_provider_core_stream_output_text_item_id(self.request_id)
    }

    pub(super) fn observe_output_text(&mut self, text: &str) -> Vec<String> {
        let mut events = Vec::new();
        if text.is_empty() {
            return events;
        }
        if let Some(event) = self.output_text_item_added_event() {
            events.push(event);
        }
        self.output_text.push_str(text);
        let sequence_number = self.next_sequence_number();
        let upstream_value = gemini_provider_core_stream_text_delta_source(text);
        if let Some((event_name, data)) = runtime_provider_stream_text_delta_event(
            RuntimeProviderBridgeKind::Gemini,
            &upstream_value,
            sequence_number,
            self.created_at,
            &self.response_id,
        ) {
            events.push(self.event(&event_name, data));
        } else {
            events.push(self.event(
                "response.output_text.delta",
                gemini_provider_core_output_text_delta_event(
                    sequence_number,
                    self.created_at,
                    &self.response_id,
                    text,
                ),
            ));
        }
        events
    }

    pub(super) fn metadata_event(&mut self, metadata: serde_json::Value) -> Option<String> {
        if self.metadata_sent.as_ref() == Some(&metadata) {
            return None;
        }
        self.metadata_sent = Some(metadata.clone());
        let sequence_number = self.next_sequence_number();
        Some(self.event(
            "response.metadata",
            gemini_provider_core_response_metadata_event(
                sequence_number,
                self.created_at,
                &self.response_id,
                metadata,
            ),
        ))
    }

    fn output_text_item_added_event(&mut self) -> Option<String> {
        if self.output_text_item_added {
            return None;
        }
        self.output_text_item_added = true;
        let sequence_number = self.next_sequence_number();
        Some(runtime_provider_sse_output_text_item_added_event(
            sequence_number,
            &self.response_id,
            &self.output_text_item_id(),
        ))
    }

    pub(super) fn reasoning_delta_events(&mut self, text: &str) -> Vec<String> {
        let mut events = Vec::new();
        if let Some(event) = self.output_text_item_added_event() {
            events.push(event);
        }
        if !self.reasoning_summary_part_added {
            self.reasoning_summary_part_added = true;
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.reasoning_summary_part.added",
                gemini_provider_core_reasoning_summary_part_added_event(
                    sequence_number,
                    &self.response_id,
                    0,
                ),
            ));
        }
        let sequence_number = self.next_sequence_number();
        let upstream_value = gemini_provider_core_stream_reasoning_delta_source(text);
        if let Some((event_name, data)) = runtime_provider_stream_reasoning_summary_text_delta_event(
            RuntimeProviderBridgeKind::Gemini,
            &upstream_value,
            sequence_number,
            &self.response_id,
            0,
        ) {
            events.push(self.event(&event_name, data));
        } else {
            events.push(self.event(
                "response.reasoning_summary_text.delta",
                gemini_provider_core_reasoning_summary_text_delta_event(
                    sequence_number,
                    &self.response_id,
                    0,
                    text,
                ),
            ));
        }
        events
    }

    pub(super) fn complete_output_text_item_events(&mut self) -> Vec<String> {
        if self.output_text.is_empty() || self.output_text_item_done {
            return Vec::new();
        }
        let mut events = Vec::new();
        if let Some(event) = self.output_text_item_added_event() {
            events.push(event);
        }
        self.output_text_item_done = true;
        let sequence_number = self.next_sequence_number();
        events.push(runtime_provider_sse_output_text_item_done_event(
            sequence_number,
            &self.response_id,
            &self.output_text_item_id(),
            &self.output_text,
        ));
        events
    }

    fn media_item_id(&self) -> String {
        gemini_provider_core_stream_media_item_id(self.request_id)
    }

    fn citation_item_id(&self) -> String {
        gemini_provider_core_stream_citation_item_id(self.request_id)
    }

    pub(super) fn observe_citation_text(&mut self, text: String) -> Vec<String> {
        if self.citation_item_done || text.is_empty() {
            return Vec::new();
        }
        self.citation_item_done = true;
        self.citation_text = Some(text.clone());
        let sequence_number = self.next_sequence_number();
        let item = gemini_provider_core_stream_message_item(
            &self.citation_item_id(),
            vec![gemini_provider_core_stream_output_text_content(&text)],
        );
        vec![self.event(
            "response.output_item.done",
            gemini_provider_core_output_item_done_event(
                sequence_number,
                Some(&self.response_id),
                &item,
            ),
        )]
    }

    pub(super) fn complete_media_item_events(&mut self) -> Vec<String> {
        if self.media_content_items.is_empty() || self.media_item_done {
            return Vec::new();
        }
        self.media_item_done = true;
        let sequence_number = self.next_sequence_number();
        let item = gemini_provider_core_stream_message_item(
            &self.media_item_id(),
            self.media_content_items.clone(),
        );
        vec![self.event(
            "response.output_item.done",
            gemini_provider_core_output_item_done_event(
                sequence_number,
                Some(&self.response_id),
                &item,
            ),
        )]
    }

    pub(super) fn complete_image_generation_item_events(&mut self) -> Vec<String> {
        if self.image_generation_items.is_empty() || self.image_generation_items_done {
            return Vec::new();
        }
        self.image_generation_items_done = true;
        let pending = self.image_generation_items.clone();
        let mut events = Vec::new();
        for item in pending {
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.output_item.done",
                gemini_provider_core_output_item_done_event(
                    sequence_number,
                    Some(&self.response_id),
                    &item,
                ),
            ));
        }
        events
    }
}

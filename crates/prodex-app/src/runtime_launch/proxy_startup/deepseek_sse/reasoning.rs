//! Preserve the native Responses item lifecycle around streamed reasoning.
use super::super::provider_bridge::runtime_provider_stream_reasoning_summary_text_delta_event;
use super::RuntimeDeepSeekSseState;
use prodex_provider_core::{
    deepseek_provider_core_output_item_added_event, deepseek_provider_core_output_item_done_event,
};

impl RuntimeDeepSeekSseState {
    pub(super) fn item_output_index(&mut self, id: &str) -> usize {
        let next = self.output_indices.len();
        *self.output_indices.entry(id.to_owned()).or_insert(next)
    }

    pub(super) fn output_item_event(
        &mut self,
        event: &str,
        id: &str,
        item: serde_json::Value,
    ) -> String {
        let index = self.item_output_index(id);
        let sequence = self.next_sequence_number();
        let mut data = if event == "response.output_item.added" {
            deepseek_provider_core_output_item_added_event(sequence, &item)
        } else {
            deepseek_provider_core_output_item_done_event(sequence, &item)
        };
        data["output_index"] = index.into();
        data["response_id"] = self.response_id.clone().into();
        self.event(event, data)
    }

    pub(super) fn decorate_text_delta(&self, data: &mut serde_json::Value) {
        let id = self.output_text_item_id();
        data["output_index"] = self.output_indices[&id].into();
        data["item_id"] = id.into();
        data["content_index"] = 0.into();
    }

    fn reasoning_item_id(&self) -> String {
        format!("rs_{}", self.response_id)
    }

    pub(super) fn reasoning_item(&self) -> serde_json::Value {
        serde_json::json!({"id":self.reasoning_item_id(),"type":"reasoning","status":"completed",
            "summary":[{"type":"summary_text","text":self.reasoning_content}]})
    }

    pub(super) fn complete_reasoning_events(&mut self) -> Vec<String> {
        if self.reasoning_content.is_empty() || self.reasoning_summary_done {
            return Vec::new();
        }
        self.reasoning_summary_done = true;
        let id = self.reasoning_item_id();
        let index = self.output_indices[&id];
        let mut events = Vec::new();
        for event in [
            "response.reasoning_summary_text.done",
            "response.reasoning_summary_part.done",
        ] {
            let mut data = serde_json::json!({"type":event,"sequence_number":self.next_sequence_number(),
                "response_id":self.response_id,"item_id":id,"output_index":index,"summary_index":0});
            if event == "response.reasoning_summary_text.done" {
                data["text"] = self.reasoning_content.clone().into();
            } else {
                data["part"] =
                    serde_json::json!({"type":"summary_text","text":self.reasoning_content});
            }
            events.push(self.event(event, data));
        }
        events.push(self.output_item_event(
            "response.output_item.done",
            &id,
            self.reasoning_item(),
        ));
        events
    }

    pub(in crate::runtime_launch::proxy_startup) fn observe_reasoning_delta(
        &mut self,
        upstream_value: &serde_json::Value,
        text: &str,
    ) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }
        self.reasoning_content.push_str(text);
        let mut events = Vec::new();
        if !self.reasoning_summary_part_added {
            self.reasoning_summary_part_added = true;
            let id = self.reasoning_item_id();
            let item =
                serde_json::json!({"id":id,"type":"reasoning","summary":[],"status":"in_progress"});
            events.push(self.output_item_event("response.output_item.added", &id, item));
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.reasoning_summary_part.added",
                serde_json::json!({
                    "type": "response.reasoning_summary_part.added",
                    "sequence_number": sequence_number,
                    "response_id": self.response_id,
                    "item_id": self.reasoning_item_id(),
                    "output_index": self.output_indices[&self.reasoning_item_id()],
                    "summary_index": 0,
                    "part": {"type": "summary_text", "text": ""},
                }),
            ));
        }
        let sequence_number = self.next_sequence_number();
        if let Some((event_name, mut data)) =
            runtime_provider_stream_reasoning_summary_text_delta_event(
                self.provider_kind,
                upstream_value,
                sequence_number,
                &self.response_id,
                0,
            )
        {
            data["item_id"] = self.reasoning_item_id().into();
            data["output_index"] = self.output_indices[&self.reasoning_item_id()].into();
            events.push(self.event(&event_name, data));
        } else {
            events.push(self.event(
                "response.reasoning_summary_text.delta",
                serde_json::json!({
                    "type": "response.reasoning_summary_text.delta",
                    "sequence_number": sequence_number,
                    "response_id": self.response_id,
                    "item_id": self.reasoning_item_id(),
                    "output_index": self.output_indices[&self.reasoning_item_id()],
                    "summary_index": 0,
                    "delta": text,
                }),
            ));
        }
        events
    }
}

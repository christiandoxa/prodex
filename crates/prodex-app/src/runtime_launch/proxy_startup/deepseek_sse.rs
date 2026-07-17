//! Chat-compatible SSE chunk observation for DeepSeek-style providers.
//! Tool-call accumulation and completion assembly live in focused child modules.

use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_label,
    runtime_provider_stream_reasoning_summary_text_delta_event,
    runtime_provider_stream_text_delta_event,
};
use prodex_domain::{CallId, RequestId};
use prodex_provider_core::{
    deepseek_provider_core_chat_stream_error, deepseek_provider_core_output_text_delta_event,
    deepseek_provider_core_response_created_event, deepseek_provider_core_stream_choice_delta,
    deepseek_provider_core_stream_choice_metadata, deepseek_provider_core_stream_chunk_metadata,
    deepseek_provider_core_stream_first_choice,
    deepseek_provider_core_stream_response_id_from_chunk,
    deepseek_provider_core_stream_text_delta_source, provider_core_chat_compatible_created_at,
};
use std::collections::BTreeMap;
use std::fmt;

mod completion;
mod tool_calls;

pub(super) struct RuntimeDeepSeekSseState {
    provider_kind: RuntimeProviderBridgeKind,
    request_id: u64,
    response_id: String,
    created_at: u64,
    sequence_number: u64,
    created: bool,
    completed: bool,
    pub(super) eof: bool,
    output_text_item_added: bool,
    output_text_item_done: bool,
    reasoning_summary_part_added: bool,
    reasoning_summary_done: bool,
    model: Option<String>,
    output_text: String,
    reasoning_content: String,
    refusal: String,
    tool_calls: BTreeMap<usize, RuntimeDeepSeekToolCall>,
    usage: Option<serde_json::Value>,
    logprobs: Option<serde_json::Value>,
    annotations: Vec<serde_json::Value>,
    finish_reason: Option<String>,
    system_fingerprint: Option<String>,
    response_metadata: Option<serde_json::Value>,
    conversation_messages: Vec<serde_json::Value>,
    conversations: RuntimeDeepSeekConversationStore,
}

struct RuntimeDeepSeekToolCall {
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
    thought_signature: Option<String>,
    added: bool,
    done: bool,
}

impl fmt::Debug for RuntimeDeepSeekSseState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeDeepSeekSseState")
            .field("provider_kind", &self.provider_kind)
            .field("request_id", &"<redacted>")
            .field("response_id", &"<redacted>")
            .field("created_at", &"<redacted>")
            .field("sequence_number", &"<redacted>")
            .field("created", &self.created)
            .field("completed", &self.completed)
            .field("eof", &self.eof)
            .field("output_text_item_added", &self.output_text_item_added)
            .field("output_text_item_done", &self.output_text_item_done)
            .field("model", &self.model.as_ref().map(|_| "<redacted>"))
            .field("output_text", &"<redacted>")
            .field("reasoning_content", &"<redacted>")
            .field("refusal", &"<redacted>")
            .field("tool_calls", &redacted_len(self.tool_calls.len()))
            .field("usage", &self.usage.as_ref().map(|_| "<redacted>"))
            .field("logprobs", &self.logprobs.as_ref().map(|_| "<redacted>"))
            .field("annotations", &redacted_len(self.annotations.len()))
            .field(
                "finish_reason",
                &self.finish_reason.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "system_fingerprint",
                &self.system_fingerprint.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "response_metadata",
                &self.response_metadata.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "conversation_messages",
                &redacted_len(self.conversation_messages.len()),
            )
            .field("conversations", &"<redacted>")
            .finish()
    }
}

impl fmt::Debug for RuntimeDeepSeekToolCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeDeepSeekToolCall")
            .field("call_id", &self.call_id.as_ref().map(|_| "<redacted>"))
            .field("name", &self.name.as_ref().map(|_| "<redacted>"))
            .field("arguments", &"<redacted>")
            .field(
                "thought_signature",
                &self.thought_signature.as_ref().map(|_| "<redacted>"),
            )
            .field("added", &self.added)
            .field("done", &self.done)
            .finish()
    }
}

fn redacted_len(len: usize) -> String {
    format!("<redacted:{len}>")
}

impl Default for RuntimeDeepSeekToolCall {
    fn default() -> Self {
        Self {
            call_id: Some(format!("call_deepseek_{}", CallId::new())),
            name: None,
            arguments: String::new(),
            thought_signature: None,
            added: false,
            done: false,
        }
    }
}

impl RuntimeDeepSeekSseState {
    #[allow(dead_code)]
    pub(super) fn new(
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        response_metadata: Option<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
    ) -> Self {
        Self::new_with_provider(
            RuntimeProviderBridgeKind::DeepSeek,
            request_id,
            conversation_messages,
            response_metadata,
            conversations,
        )
    }

    pub(super) fn new_with_provider(
        provider_kind: RuntimeProviderBridgeKind,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        response_metadata: Option<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
    ) -> Self {
        Self {
            provider_kind,
            request_id,
            response_id: format!("resp_deepseek_{}", RequestId::new()),
            created_at: provider_core_chat_compatible_created_at(),
            sequence_number: 0,
            created: false,
            completed: false,
            eof: false,
            output_text_item_added: false,
            output_text_item_done: false,
            reasoning_summary_part_added: false,
            reasoning_summary_done: false,
            model: None,
            output_text: String::new(),
            reasoning_content: String::new(),
            refusal: String::new(),
            tool_calls: BTreeMap::new(),
            usage: None,
            logprobs: None,
            annotations: Vec::new(),
            finish_reason: None,
            system_fingerprint: None,
            response_metadata,
            conversation_messages,
            conversations,
        }
    }

    pub(super) fn observe_chat_chunk(&mut self, value: &serde_json::Value) -> Vec<String> {
        if let Some((code, message)) = deepseek_provider_core_chat_stream_error(value) {
            return self
                .failed_event(&code, &message)
                .into_iter()
                .collect::<Vec<_>>();
        }
        if let Some(id) = deepseek_provider_core_stream_response_id_from_chunk(
            runtime_provider_label(self.provider_kind),
            &self.response_id,
            value,
        ) {
            self.response_id = id;
        }
        let metadata = deepseek_provider_core_stream_chunk_metadata(
            value,
            runtime_provider_label(self.provider_kind),
        );
        if let Some(model) = metadata.model {
            self.model = Some(model);
        }
        if let Some(created) = metadata.created_at {
            self.created_at = created;
        }
        if let Some(system_fingerprint) = metadata.system_fingerprint {
            self.system_fingerprint = Some(system_fingerprint);
        }
        if let Some(usage) = metadata.usage {
            self.usage = Some(usage);
        }
        let mut events = Vec::new();
        if !self.created {
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.created",
                deepseek_provider_core_response_created_event(
                    sequence_number,
                    self.created_at,
                    &self.response_id,
                ),
            ));
            self.created = true;
        }
        let Some(choice) = deepseek_provider_core_stream_first_choice(value) else {
            return events;
        };
        let choice_metadata = deepseek_provider_core_stream_choice_metadata(choice);
        if let Some(logprobs) = choice_metadata.logprobs {
            self.logprobs = Some(logprobs);
        }
        let choice_delta = deepseek_provider_core_stream_choice_delta(choice);
        if let Some(reasoning_content) = choice_delta.reasoning_content.as_deref() {
            events.extend(self.observe_reasoning_delta(value, reasoning_content));
        }
        if let Some(refusal) = choice_delta.refusal.as_deref() {
            self.refusal.push_str(refusal);
        }
        self.annotations
            .extend(choice_delta.annotations.iter().cloned());
        if let Some(text) = choice_delta.content.as_deref() {
            if let Some(event) = self.output_text_item_added_event() {
                events.push(event);
            }
            self.output_text.push_str(text);
            let sequence_number = self.next_sequence_number();
            let upstream_value = deepseek_provider_core_stream_text_delta_source(text);
            if let Some((event_name, data)) = runtime_provider_stream_text_delta_event(
                self.provider_kind,
                &upstream_value,
                sequence_number,
                self.created_at,
                &self.response_id,
            ) {
                events.push(self.event(&event_name, data));
            } else {
                events.push(self.event(
                    "response.output_text.delta",
                    deepseek_provider_core_output_text_delta_event(
                        sequence_number,
                        self.created_at,
                        &self.response_id,
                        text,
                    ),
                ));
            }
        }
        for tool_call in &choice_delta.tool_calls {
            events.extend(self.observe_tool_call_delta(tool_call));
        }
        if let Some(finish_reason) = choice_metadata.finish_reason {
            self.finish_reason = Some(finish_reason);
            if let Err(message) = self.validate_tool_call_arguments() {
                if let Some(event) = self.failed_event("invalid_tool_call_arguments", &message) {
                    events.push(event);
                }
                return events;
            }
            events.extend(self.complete_tool_call_events());
            if !self.tool_calls.is_empty() {
                self.store_conversation_snapshot();
            }
        }
        events
    }

    pub(super) fn observe_reasoning_delta(
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
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.reasoning_summary_part.added",
                serde_json::json!({
                    "type": "response.reasoning_summary_part.added",
                    "sequence_number": sequence_number,
                    "response_id": self.response_id,
                    "output_index": 0,
                    "summary_index": 0,
                    "part": {"type": "summary_text", "text": ""},
                }),
            ));
        }
        let sequence_number = self.next_sequence_number();
        if let Some((event_name, data)) = runtime_provider_stream_reasoning_summary_text_delta_event(
            self.provider_kind,
            upstream_value,
            sequence_number,
            &self.response_id,
            0,
        ) {
            events.push(self.event(&event_name, data));
        } else {
            events.push(self.event(
                "response.reasoning_summary_text.delta",
                serde_json::json!({
                    "type": "response.reasoning_summary_text.delta",
                    "sequence_number": sequence_number,
                    "response_id": self.response_id,
                    "output_index": 0,
                    "summary_index": 0,
                    "delta": text,
                }),
            ));
        }
        events
    }
}

#[cfg(test)]
#[path = "deepseek_sse_tests.rs"]
mod tests;

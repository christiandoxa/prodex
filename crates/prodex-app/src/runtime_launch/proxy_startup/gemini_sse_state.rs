use super::super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_store_conversation,
};
use super::super::gemini_rewrite::runtime_gemini_blocked_tool_call_message_with_config;
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_stream_reasoning_summary_text_delta_event,
    runtime_provider_stream_text_delta_event,
};
use super::super::provider_sse_events::{
    runtime_provider_sse_failed_event, runtime_provider_sse_output_text_item_added_event,
    runtime_provider_sse_output_text_item_done_event,
};
use super::RuntimeGeminiBindingRecorder;
use prodex_domain::RequestId;
use prodex_provider_core::{
    GeminiProviderCoreStreamToolCall, gemini_provider_core_citation_text,
    gemini_provider_core_conversation_requests_command_output_only as runtime_gemini_conversation_requests_command_output_only,
    gemini_provider_core_finish_reason_failure, gemini_provider_core_finish_reason_incomplete,
    gemini_provider_core_forced_command_output as runtime_gemini_forced_command_output,
    gemini_provider_core_image_generation_call_item_from_part,
    gemini_provider_core_internal_instruction_corpus,
    gemini_provider_core_media_content_item_from_part,
    gemini_provider_core_non_actionable_wait_or_poll_text as runtime_gemini_non_actionable_wait_or_poll_text,
    gemini_provider_core_output_item_done_event, gemini_provider_core_output_text_delta_event,
    gemini_provider_core_prompt_feedback_failure,
    gemini_provider_core_reasoning_summary_part_added_event,
    gemini_provider_core_reasoning_summary_text_delta_event,
    gemini_provider_core_response_completed_event, gemini_provider_core_response_created_event,
    gemini_provider_core_response_incomplete_event, gemini_provider_core_response_metadata_event,
    gemini_provider_core_stream_candidate_parts,
    gemini_provider_core_stream_chat_assistant_message, gemini_provider_core_stream_chunk_metadata,
    gemini_provider_core_stream_error as runtime_gemini_stream_error,
    gemini_provider_core_stream_message_item, gemini_provider_core_stream_output_items,
    gemini_provider_core_stream_output_text_content,
    gemini_provider_core_stream_part_function_call,
    gemini_provider_core_stream_part_has_video_metadata,
    gemini_provider_core_stream_part_is_thought, gemini_provider_core_stream_part_text,
    gemini_provider_core_stream_reasoning_delta_source, gemini_provider_core_stream_response_value,
    gemini_provider_core_stream_text_delta_source, gemini_provider_core_stream_tool_call,
    gemini_provider_core_stream_tool_call_ids,
    gemini_provider_core_text_echoes_internal_instruction,
    gemini_provider_core_text_from_special_part, gemini_provider_core_thought_signature,
    gemini_provider_core_tool_intent_without_call as runtime_gemini_tool_intent_without_call,
    gemini_provider_core_unverified_success_claim as runtime_gemini_unverified_success_claim,
    gemini_provider_core_visible_text_from_part,
    gemini_provider_core_web_search_call_from_grounding, provider_core_chat_compatible_created_at,
};
use prodex_runtime_gemini::GEMINI_DEFAULT_MODEL;
use std::collections::BTreeMap;

#[path = "gemini_sse_state/complete.rs"]
mod gemini_sse_state_complete;
#[path = "gemini_sse_state/events.rs"]
mod gemini_sse_state_events;
#[path = "gemini_sse_state/output.rs"]
mod gemini_sse_state_output;
#[path = "gemini_sse_tool_calls.rs"]
mod gemini_sse_tool_calls;
use gemini_sse_tool_calls::RuntimeGeminiToolCall;

pub(super) struct RuntimeGeminiSseState {
    pub(super) gemini_config: crate::RuntimeGeminiConfig,
    harness_mode: prodex_provider_core::EffectiveHarnessMode,
    harness_model: Option<String>,
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
    media_item_done: bool,
    image_generation_items_done: bool,
    citation_item_done: bool,
    metadata_sent: Option<serde_json::Value>,
    model: Option<String>,
    finish_reason: Option<String>,
    output_text: String,
    pending_output_text: String,
    internal_instruction_corpus: String,
    reasoning_content: String,
    media_content_items: Vec<serde_json::Value>,
    native_parts: Vec<serde_json::Value>,
    image_generation_items: Vec<serde_json::Value>,
    citation_text: Option<String>,
    metadata: Option<serde_json::Value>,
    tool_calls: BTreeMap<usize, RuntimeGeminiToolCall>,
    tool_call_indices_by_id: BTreeMap<String, usize>,
    usage: Option<serde_json::Value>,
    web_search_call: Option<serde_json::Value>,
    conversation_messages: Vec<serde_json::Value>,
    conversations: RuntimeDeepSeekConversationStore,
    binding_recorder: Option<RuntimeGeminiBindingRecorder>,
    command_output_only: bool,
    forced_output_text: Option<String>,
}

impl RuntimeGeminiSseState {
    pub(super) fn new(
        _request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
        binding_recorder: Option<RuntimeGeminiBindingRecorder>,
        harness_mode: prodex_provider_core::EffectiveHarnessMode,
        harness_model: Option<String>,
        gemini_config: crate::RuntimeGeminiConfig,
    ) -> Self {
        let command_output_only =
            runtime_gemini_conversation_requests_command_output_only(&conversation_messages);
        let forced_output_text = runtime_gemini_forced_command_output(&conversation_messages);
        let internal_instruction_corpus =
            gemini_provider_core_internal_instruction_corpus(&conversation_messages);
        let response_id = RequestId::new();
        let request_id = (response_id.as_uuid().as_u128() >> 64) as u64;
        Self {
            gemini_config,
            harness_mode,
            harness_model,
            request_id,
            response_id: format!("resp_gemini_{response_id}"),
            created_at: provider_core_chat_compatible_created_at(),
            sequence_number: 0,
            created: false,
            completed: false,
            eof: false,
            output_text_item_added: false,
            output_text_item_done: false,
            reasoning_summary_part_added: false,
            media_item_done: false,
            image_generation_items_done: false,
            citation_item_done: false,
            metadata_sent: None,
            model: None,
            finish_reason: None,
            output_text: String::new(),
            pending_output_text: String::new(),
            internal_instruction_corpus,
            reasoning_content: String::new(),
            media_content_items: Vec::new(),
            native_parts: Vec::new(),
            image_generation_items: Vec::new(),
            citation_text: None,
            metadata: None,
            tool_calls: BTreeMap::new(),
            tool_call_indices_by_id: BTreeMap::new(),
            usage: None,
            web_search_call: None,
            conversation_messages,
            conversations,
            binding_recorder,
            command_output_only,
            forced_output_text,
        }
    }

    pub(super) fn postprocess_harness_events(&self, events: Vec<String>) -> Vec<String> {
        events
            .into_iter()
            .map(|event| self.postprocess_harness_event(event))
            .collect()
    }

    pub(super) fn postprocess_harness_event(&self, event: String) -> String {
        prodex_provider_core::postprocess_harness_provider_stream_event(
            self.harness_mode,
            prodex_provider_core::ProviderId::Gemini,
            self.harness_model.as_deref(),
            prodex_provider_core::ProviderEndpoint::Responses,
            event.as_bytes(),
        )
        .map(|transform| transform.body.into_owned())
        .ok()
        .and_then(|body| String::from_utf8(body).ok())
        .unwrap_or(event)
    }

    pub(super) fn observe_generate_chunk(&mut self, value: &serde_json::Value) -> Vec<String> {
        if let Some((code, message)) = runtime_gemini_stream_error(value) {
            return self
                .failed_event(&code, &message)
                .into_iter()
                .collect::<Vec<_>>();
        }
        let metadata = gemini_provider_core_stream_chunk_metadata(&self.response_id, value);
        if let Some(id) = metadata.response_id {
            self.response_id = id;
        }
        if let Some(model) = metadata.model {
            self.model = Some(model);
        }
        if let Some(usage) = metadata.usage {
            self.usage = Some(usage);
        }
        if let Some(metadata) = metadata.response_metadata {
            self.metadata = Some(runtime_gemini_merge_metadata(
                self.metadata.take(),
                metadata.clone(),
            ));
        }
        if let Some((code, message)) = gemini_provider_core_prompt_feedback_failure(value) {
            return self
                .failed_event(&code, &message)
                .into_iter()
                .collect::<Vec<_>>();
        }
        let finish_reason = metadata.finish_reason;
        if let Some(reason) = finish_reason.clone() {
            self.finish_reason = Some(reason);
        }
        let mut events = Vec::new();
        if !self.created {
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.created",
                gemini_provider_core_response_created_event(
                    sequence_number,
                    self.created_at,
                    &self.response_id,
                ),
            ));
            self.created = true;
        }
        if let Some(metadata) = self.metadata.clone()
            && let Some(event) = self.metadata_event(metadata)
        {
            events.push(event);
        }
        if let Some(event) = self.observe_grounding(value) {
            events.push(event);
        }
        let Some(parts) = gemini_provider_core_stream_candidate_parts(value) else {
            return events;
        };
        for (part_index, part) in parts.iter().enumerate() {
            if let Some(text) = gemini_provider_core_stream_part_text(part) {
                if gemini_provider_core_stream_part_is_thought(part) {
                    self.reasoning_content.push_str(text);
                    events.extend(self.reasoning_delta_events(text));
                } else if let Some(text) = gemini_provider_core_visible_text_from_part(part) {
                    if self.command_output_only || self.forced_output_text.is_some() {
                        continue;
                    }
                    if !gemini_provider_core_text_echoes_internal_instruction(
                        &text,
                        &self.internal_instruction_corpus,
                    ) {
                        events.extend(self.observe_output_text(&text));
                    }
                }
            }
            if let Some(content_item) = gemini_provider_core_media_content_item_from_part(part) {
                if !self.media_content_items.contains(&content_item) {
                    self.media_content_items.push(content_item);
                }
                if !self.native_parts.contains(part) {
                    self.native_parts.push(part.clone());
                }
            }
            if gemini_provider_core_stream_part_has_video_metadata(part)
                && !self.native_parts.contains(part)
            {
                self.native_parts.push(part.clone());
            }
            if let Some(text) = gemini_provider_core_text_from_special_part(part)
                && !self.command_output_only
                && self.forced_output_text.is_none()
            {
                events.extend(self.observe_output_text(&text));
            }
            if let Some(item) = gemini_provider_core_image_generation_call_item_from_part(
                &self.response_id,
                part_index,
                part,
            ) {
                let item_id = item.get("id").and_then(serde_json::Value::as_str);
                if let Some(existing) = self.image_generation_items.iter_mut().find(|existing| {
                    existing.get("id").and_then(serde_json::Value::as_str) == item_id
                }) {
                    *existing = item;
                } else {
                    self.image_generation_items.push(item);
                }
            }
            if let Some(function_call) = gemini_provider_core_stream_part_function_call(part) {
                if self.forced_output_text.is_some() {
                    continue;
                }
                events.extend(self.flush_pending_output_text_events());
                let thought_signature = gemini_provider_core_thought_signature(part)
                    .or_else(|| gemini_provider_core_thought_signature(function_call));
                events.extend(self.observe_function_call(
                    part_index,
                    function_call,
                    thought_signature,
                ));
            }
        }
        if let Some(reason) = finish_reason {
            if let Some(text) = gemini_provider_core_citation_text(value) {
                events.extend(self.observe_citation_text(text));
            }
            if let Some((reason, message)) = gemini_provider_core_finish_reason_incomplete(&reason)
            {
                events.extend(self.flush_pending_output_text_events());
                events.extend(self.complete_tool_call_events());
                events.extend(self.complete_output_text_item_events());
                events.extend(self.complete_media_item_events());
                events.extend(self.complete_image_generation_item_events());
                if let Some(event) = self.incomplete_event(&reason, &message) {
                    events.push(event);
                }
                return events;
            }
            if let Some((code, message)) = gemini_provider_core_finish_reason_failure(&reason) {
                events.extend(self.flush_pending_output_text_events());
                if let Some(event) = self.failed_event(&code, &message) {
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

    fn observe_grounding(&mut self, value: &serde_json::Value) -> Option<String> {
        if self.web_search_call.is_some() {
            return None;
        }
        let item = gemini_provider_core_web_search_call_from_grounding(value, &self.response_id)?;
        self.web_search_call = Some(item.clone());
        let sequence_number = self.next_sequence_number();
        Some(self.event(
            "response.output_item.done",
            gemini_provider_core_output_item_done_event(sequence_number, None, &item),
        ))
    }

    pub(super) fn incomplete_event(&mut self, reason: &str, message: &str) -> Option<String> {
        if self.completed {
            return None;
        }
        let sequence_number = self.next_sequence_number();
        self.completed = true;
        Some(self.event(
            "response.incomplete",
            gemini_provider_core_response_incomplete_event(
                sequence_number,
                self.created_at,
                &self.response_id,
                reason,
                message,
            ),
        ))
    }

    pub(super) fn failed_event(&mut self, code: &str, message: &str) -> Option<String> {
        if self.completed {
            return None;
        }
        let sequence_number = self.next_sequence_number();
        self.completed = true;
        Some(runtime_provider_sse_failed_event(
            sequence_number,
            self.created_at,
            &self.response_id,
            code,
            message,
        ))
    }

    fn event(&self, event: &str, data: serde_json::Value) -> String {
        let data = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
        format!("event: {event}\r\ndata: {data}\r\n\r\n")
    }

    fn next_sequence_number(&mut self) -> u64 {
        let next = self.sequence_number;
        self.sequence_number = self.sequence_number.saturating_add(1);
        next
    }
}

fn runtime_gemini_stream_uuid(request_id: u64, salt: u64) -> String {
    let low = (request_id.rotate_left(23) ^ salt) & 0x3fff_ffff_ffff_ffff | 0x8000_0000_0000_0000;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        request_id >> 32,
        (request_id >> 16) & 0xffff,
        request_id & 0xffff,
        low >> 48,
        low & 0xffff_ffff_ffff,
    )
}

fn gemini_provider_core_stream_output_text_item_id(request_id: u64) -> String {
    format!("msg_gemini_{}", runtime_gemini_stream_uuid(request_id, 1))
}

fn gemini_provider_core_stream_media_item_id(request_id: u64) -> String {
    format!(
        "msg_gemini_media_{}",
        runtime_gemini_stream_uuid(request_id, 2)
    )
}

fn gemini_provider_core_stream_citation_item_id(request_id: u64) -> String {
    format!(
        "msg_gemini_citations_{}",
        runtime_gemini_stream_uuid(request_id, 3)
    )
}

fn runtime_gemini_merge_metadata(
    existing: Option<serde_json::Value>,
    incoming: serde_json::Value,
) -> serde_json::Value {
    let Some(mut existing) = existing else {
        return incoming;
    };
    let Some(existing_object) = existing.as_object_mut() else {
        return incoming;
    };
    let Some(incoming_object) = incoming.as_object() else {
        return existing;
    };
    for (key, value) in incoming_object {
        match (existing_object.get_mut(key), value) {
            (Some(existing_value), serde_json::Value::Object(incoming_nested)) => {
                if let Some(existing_nested) = existing_value.as_object_mut() {
                    for (nested_key, nested_value) in incoming_nested {
                        existing_nested.insert(nested_key.clone(), nested_value.clone());
                    }
                    continue;
                }
                existing_object.insert(key.clone(), value.clone());
            }
            _ => {
                existing_object.insert(key.clone(), value.clone());
            }
        }
    }
    existing
}

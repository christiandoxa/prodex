use super::super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_created_at,
    runtime_deepseek_rtk_wrapped_tool_arguments, runtime_deepseek_store_conversation,
};
use super::super::gemini_rewrite::{
    runtime_gemini_blocked_tool_call_message, runtime_gemini_citation_text,
    runtime_gemini_custom_tool_call_item, runtime_gemini_custom_tool_input_from_arguments,
    runtime_gemini_finish_reason, runtime_gemini_finish_reason_failure,
    runtime_gemini_finish_reason_incomplete, runtime_gemini_image_generation_call_item_from_part,
    runtime_gemini_media_content_item_from_part, runtime_gemini_prompt_feedback_failure,
    runtime_gemini_response_metadata, runtime_gemini_responses_usage,
    runtime_gemini_text_from_special_part, runtime_gemini_web_search_call_from_grounding,
};
use super::super::gemini_thought_signatures::runtime_gemini_thought_signature;
use super::super::provider_tools::runtime_provider_split_flat_namespace_tool_name;
use super::RuntimeGeminiBindingRecorder;
use prodex_runtime_gemini::GEMINI_DEFAULT_MODEL;
use std::collections::BTreeMap;

pub(super) struct RuntimeGeminiSseState {
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
}

#[derive(Debug, Default)]
struct RuntimeGeminiToolCall {
    call_id: Option<String>,
    explicit_call_id: bool,
    name: Option<String>,
    arguments: String,
    thought_signature: Option<String>,
    added: bool,
    done: bool,
}

impl RuntimeGeminiSseState {
    pub(super) fn new(
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
        binding_recorder: Option<RuntimeGeminiBindingRecorder>,
    ) -> Self {
        Self {
            request_id,
            response_id: format!("resp_gemini_{request_id}"),
            created_at: runtime_deepseek_created_at(),
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
        }
    }

    pub(super) fn observe_generate_chunk(&mut self, value: &serde_json::Value) -> Vec<String> {
        if let Some((code, message)) = runtime_gemini_stream_error(value) {
            return self
                .failed_event(&code, &message)
                .into_iter()
                .collect::<Vec<_>>();
        }
        if let Some(id) = value
            .get("responseId")
            .or_else(|| value.get("id"))
            .and_then(serde_json::Value::as_str)
            && self.response_id.starts_with("resp_gemini_")
        {
            self.response_id = id.to_string();
        }
        if let Some(model) = value
            .get("modelVersion")
            .or_else(|| value.get("model"))
            .and_then(serde_json::Value::as_str)
        {
            self.model = Some(model.to_string());
        }
        if let Some(usage) = value
            .get("usageMetadata")
            .and_then(runtime_gemini_responses_usage)
        {
            self.usage = Some(usage);
        }
        if let Some(metadata) = runtime_gemini_response_metadata(value) {
            self.metadata = Some(runtime_gemini_merge_metadata(
                self.metadata.take(),
                metadata.clone(),
            ));
        }
        if let Some((code, message)) = runtime_gemini_prompt_feedback_failure(value) {
            return self
                .failed_event(&code, &message)
                .into_iter()
                .collect::<Vec<_>>();
        }
        let finish_reason = runtime_gemini_finish_reason(value);
        if let Some(reason) = finish_reason.clone() {
            self.finish_reason = Some(reason);
        }
        let mut events = Vec::new();
        if !self.created {
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.created",
                serde_json::json!({
                    "type": "response.created",
                    "sequence_number": sequence_number,
                    "created_at": self.created_at,
                    "response": {"id": self.response_id},
                }),
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
        let Some(parts) = value
            .get("candidates")
            .and_then(serde_json::Value::as_array)
            .and_then(|candidates| candidates.first())
            .and_then(|candidate| candidate.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(serde_json::Value::as_array)
        else {
            return events;
        };
        for (part_index, part) in parts.iter().enumerate() {
            if let Some(text) = part
                .get("text")
                .and_then(serde_json::Value::as_str)
                .filter(|text| !text.is_empty())
            {
                if part
                    .get("thought")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
                {
                    self.reasoning_content.push_str(text);
                    events.extend(self.reasoning_delta_events(text));
                } else {
                    if let Some(event) = self.output_text_item_added_event() {
                        events.push(event);
                    }
                    self.output_text.push_str(text);
                    let sequence_number = self.next_sequence_number();
                    events.push(self.event(
                        "response.output_text.delta",
                        serde_json::json!({
                            "type": "response.output_text.delta",
                            "sequence_number": sequence_number,
                            "created_at": self.created_at,
                            "response_id": self.response_id,
                            "delta": text,
                        }),
                    ));
                }
            }
            if let Some(content_item) = runtime_gemini_media_content_item_from_part(part) {
                if !self.media_content_items.contains(&content_item) {
                    self.media_content_items.push(content_item);
                }
                if !self.native_parts.contains(part) {
                    self.native_parts.push(part.clone());
                }
            }
            if part.get("videoMetadata").is_some() && !self.native_parts.contains(part) {
                self.native_parts.push(part.clone());
            }
            if let Some(text) = runtime_gemini_text_from_special_part(part) {
                events.extend(self.observe_output_text(&text));
            }
            if let Some(item) = runtime_gemini_image_generation_call_item_from_part(
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
            if let Some(function_call) = part.get("functionCall") {
                let thought_signature = runtime_gemini_thought_signature(part)
                    .or_else(|| runtime_gemini_thought_signature(function_call));
                events.extend(self.observe_function_call(
                    part_index,
                    function_call,
                    thought_signature,
                ));
            }
        }
        if let Some(reason) = finish_reason {
            if let Some(text) = runtime_gemini_citation_text(value) {
                events.extend(self.observe_citation_text(text));
            }
            if let Some((reason, message)) = runtime_gemini_finish_reason_incomplete(&reason) {
                events.extend(self.complete_tool_call_events());
                events.extend(self.complete_output_text_item_events());
                events.extend(self.complete_media_item_events());
                events.extend(self.complete_image_generation_item_events());
                if let Some(event) = self.incomplete_event(&reason, &message) {
                    events.push(event);
                }
                return events;
            }
            if let Some((code, message)) = runtime_gemini_finish_reason_failure(&reason) {
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
        let item = runtime_gemini_web_search_call_from_grounding(value, &self.response_id)?;
        self.web_search_call = Some(item.clone());
        let sequence_number = self.next_sequence_number();
        Some(self.event(
            "response.output_item.done",
            serde_json::json!({
                "type": "response.output_item.done",
                "sequence_number": sequence_number,
                "item": item,
            }),
        ))
    }

    fn observe_function_call(
        &mut self,
        part_index: usize,
        value: &serde_json::Value,
        thought_signature: Option<String>,
    ) -> Vec<String> {
        let explicit_call_id = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .map(str::to_string);
        let name = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("tool_call")
            .to_string();
        let index = self.function_call_index(part_index, explicit_call_id.as_deref(), &name);
        let args = value
            .get("args")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let args = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
        let mut events = Vec::new();
        let (call_id, should_add) = {
            let tool_call = self.tool_calls.entry(index).or_default();
            tool_call.call_id = explicit_call_id
                .clone()
                .or_else(|| tool_call.call_id.clone())
                .or_else(|| Some(format!("call_gemini_{}_{}", self.request_id, index)));
            if let Some(explicit_call_id) = explicit_call_id {
                self.tool_call_indices_by_id.insert(explicit_call_id, index);
                tool_call.explicit_call_id = true;
            }
            tool_call.name = Some(name.clone());
            tool_call.arguments = args.clone();
            if thought_signature.is_some() {
                tool_call.thought_signature = thought_signature.clone();
            }
            let call_id = tool_call.call_id.clone().unwrap_or_default();
            let should_add = !tool_call.added;
            if should_add {
                tool_call.added = true;
            }
            (call_id, should_add)
        };
        if should_add && name != "tool_search" && name != "apply_patch" {
            let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(&name);
            let mut item = serde_json::json!({
                "type": "function_call",
                "call_id": call_id,
                "name": name,
            });
            if let Some(namespace) = namespace {
                item["namespace"] = serde_json::Value::String(namespace);
            }
            if let Some(tool_call) = self.tool_calls.get(&index)
                && let Some(signature) = tool_call.thought_signature.clone()
            {
                item["gemini_thought_signature"] = serde_json::Value::String(signature);
            }
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.output_item.added",
                serde_json::json!({
                    "type": "response.output_item.added",
                    "sequence_number": sequence_number,
                    "item": item,
                }),
            ));
        }

        if name != "tool_search" && name != "apply_patch" {
            let sequence_number = self.next_sequence_number();
            let mut delta_event = serde_json::json!({
                "type": "response.function_call_arguments.delta",
                "sequence_number": sequence_number,
                "call_id": call_id,
                "delta": args,
            });
            if let Some(ref signature) = thought_signature.clone() {
                delta_event["thought_signature"] = serde_json::Value::String(signature.to_string());
            }
            events.push(self.event("response.function_call_arguments.delta", delta_event));
        }
        events
    }

    fn function_call_index(
        &self,
        part_index: usize,
        explicit_call_id: Option<&str>,
        name: &str,
    ) -> usize {
        if let Some(call_id) = explicit_call_id
            && let Some(index) = self.tool_call_indices_by_id.get(call_id)
        {
            return *index;
        }
        if explicit_call_id.is_none()
            && let Some(tool_call) = self.tool_calls.get(&part_index)
            && !tool_call.done
            && !tool_call.explicit_call_id
            && tool_call
                .name
                .as_deref()
                .is_none_or(|existing| existing == name)
        {
            return part_index;
        }
        if explicit_call_id.is_none()
            && let Some((index, _)) = self.tool_calls.iter().find(|(_, tool_call)| {
                !tool_call.done
                    && !tool_call.explicit_call_id
                    && tool_call.name.as_deref() == Some(name)
            })
        {
            return *index;
        }
        if !self.tool_calls.contains_key(&part_index) {
            return part_index;
        }
        let mut index = self.tool_calls.len();
        while self.tool_calls.contains_key(&index) {
            index = index.saturating_add(1);
        }
        index
    }

    fn complete_tool_call_events(&mut self) -> Vec<String> {
        let pending = self
            .tool_calls
            .iter_mut()
            .filter_map(|(index, tool_call)| {
                if tool_call.done {
                    return None;
                }
                tool_call.done = true;
                let call_id = tool_call
                    .call_id
                    .clone()
                    .unwrap_or_else(|| format!("call_gemini_{}_{}", self.request_id, index));
                let name = tool_call
                    .name
                    .clone()
                    .unwrap_or_else(|| "tool_call".to_string());
                let raw_arguments_value =
                    serde_json::from_str::<serde_json::Value>(&tool_call.arguments)
                        .unwrap_or_else(|_| serde_json::Value::String(tool_call.arguments.clone()));
                if let Some(blocked) =
                    runtime_gemini_blocked_tool_call_message(&name, &raw_arguments_value)
                {
                    return Some((call_id, name, blocked, *index, true));
                }
                let arguments = if name == "apply_patch" {
                    tool_call.arguments.clone()
                } else {
                    runtime_deepseek_rtk_wrapped_tool_arguments(&name, &tool_call.arguments)
                };
                tool_call.arguments = arguments.clone();
                Some((call_id, name, arguments, *index, false))
            })
            .collect::<Vec<_>>();
        let mut events = Vec::new();
        for (call_id, name, arguments, index, blocked) in pending {
            if blocked {
                let sequence_number = self.next_sequence_number();
                events.push(self.event(
                    "response.output_item.done",
                    serde_json::json!({
                        "type": "response.output_item.done",
                        "sequence_number": sequence_number,
                        "item": runtime_gemini_blocked_tool_call_item(&arguments),
                    }),
                ));
                continue;
            }
            if name == "tool_search" {
                let arguments = serde_json::from_str::<serde_json::Value>(&arguments)
                    .unwrap_or_else(|_| serde_json::json!({}));
                let sequence_number = self.next_sequence_number();
                events.push(self.event(
                    "response.output_item.done",
                    serde_json::json!({
                        "type": "response.output_item.done",
                        "sequence_number": sequence_number,
                        "item": {
                            "type": "tool_search_call",
                            "call_id": call_id,
                            "execution": "client",
                            "arguments": arguments,
                        },
                    }),
                ));
                continue;
            }
            if name == "apply_patch" {
                let sequence_number = self.next_sequence_number();
                events.push(self.event(
                    "response.output_item.done",
                    serde_json::json!({
                        "type": "response.output_item.done",
                        "sequence_number": sequence_number,
                        "item": {
                            "type": "custom_tool_call",
                            "call_id": call_id,
                            "name": name,
                            "input": runtime_gemini_custom_tool_input_from_arguments(&arguments),
                        },
                    }),
                ));
                continue;
            }
            let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(&name);
            let mut item = serde_json::json!({
                "type": "function_call",
                "call_id": call_id,
                "name": name,
                "arguments": arguments,
            });
            if let Some(namespace) = namespace {
                item["namespace"] = serde_json::Value::String(namespace);
            }
            if let Some(tool_call) = self.tool_calls.get(&index)
                && let Some(signature) = tool_call.thought_signature.clone()
            {
                item["gemini_thought_signature"] = serde_json::Value::String(signature);
            }
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.output_item.done",
                serde_json::json!({
                    "type": "response.output_item.done",
                    "sequence_number": sequence_number,
                    "item": item,
                }),
            ));
        }
        events
    }

    pub(super) fn complete_event(&mut self) -> Option<String> {
        if self.completed {
            return None;
        }
        let mut events = self.complete_tool_call_events();
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
        let mut response = serde_json::json!({
            "id": self.response_id,
            "output": output,
        });
        response["model"] = serde_json::Value::String(
            self.model
                .clone()
                .unwrap_or_else(|| GEMINI_DEFAULT_MODEL.to_string()),
        );
        if let Some(usage) = self.usage.clone() {
            response["usage"] = usage;
        }
        if let Some(metadata) = self.metadata.clone() {
            response["metadata"] = metadata;
        }
        let sequence_number = self.next_sequence_number();
        events.push(self.event(
            "response.completed",
            serde_json::json!({
                "type": "response.completed",
                "sequence_number": sequence_number,
                "created_at": self.created_at,
                "response": response,
            }),
        ));
        runtime_deepseek_store_conversation(
            &self.conversations,
            &self.response_id,
            self.conversation_messages.clone(),
            self.chat_assistant_messages(),
        );
        if let Some(recorder) = &self.binding_recorder {
            recorder(self.response_id.clone(), self.tool_call_ids());
        }
        self.completed = true;
        Some(events.join(""))
    }

    pub(super) fn incomplete_event(&mut self, reason: &str, message: &str) -> Option<String> {
        if self.completed {
            return None;
        }
        let sequence_number = self.next_sequence_number();
        self.completed = true;
        Some(self.event(
            "response.incomplete",
            serde_json::json!({
                "type": "response.incomplete",
                "sequence_number": sequence_number,
                "created_at": self.created_at,
                "response": {
                    "id": self.response_id,
                    "status": "incomplete",
                    "incomplete_details": {
                        "reason": reason,
                        "message": message,
                    },
                },
            }),
        ))
    }

    pub(super) fn failed_event(&mut self, code: &str, message: &str) -> Option<String> {
        if self.completed {
            return None;
        }
        let sequence_number = self.next_sequence_number();
        self.completed = true;
        Some(self.event(
            "response.failed",
            serde_json::json!({
                "type": "response.failed",
                "sequence_number": sequence_number,
                "created_at": self.created_at,
                "response": {
                    "id": self.response_id,
                    "error": {
                        "code": code,
                        "message": message,
                    },
                },
            }),
        ))
    }

    fn output_text_item_id(&self) -> String {
        format!("msg_gemini_{}", self.request_id)
    }

    fn observe_output_text(&mut self, text: &str) -> Vec<String> {
        let mut events = Vec::new();
        if text.is_empty() {
            return events;
        }
        if let Some(event) = self.output_text_item_added_event() {
            events.push(event);
        }
        self.output_text.push_str(text);
        let sequence_number = self.next_sequence_number();
        events.push(self.event(
            "response.output_text.delta",
            serde_json::json!({
                "type": "response.output_text.delta",
                "sequence_number": sequence_number,
                "created_at": self.created_at,
                "response_id": self.response_id,
                "delta": text,
            }),
        ));
        events
    }

    fn metadata_event(&mut self, metadata: serde_json::Value) -> Option<String> {
        if self.metadata_sent.as_ref() == Some(&metadata) {
            return None;
        }
        self.metadata_sent = Some(metadata.clone());
        let sequence_number = self.next_sequence_number();
        Some(self.event(
            "response.metadata",
            serde_json::json!({
                "type": "response.metadata",
                "sequence_number": sequence_number,
                "created_at": self.created_at,
                "response_id": self.response_id,
                "metadata": metadata,
            }),
        ))
    }

    fn output_text_item_added_event(&mut self) -> Option<String> {
        if self.output_text_item_added {
            return None;
        }
        self.output_text_item_added = true;
        let sequence_number = self.next_sequence_number();
        Some(self.event(
            "response.output_item.added",
            serde_json::json!({
                "type": "response.output_item.added",
                "sequence_number": sequence_number,
                "response_id": self.response_id,
                "item": {
                    "id": self.output_text_item_id(),
                    "type": "message",
                    "role": "assistant",
                    "content": [],
                },
            }),
        ))
    }

    fn reasoning_delta_events(&mut self, text: &str) -> Vec<String> {
        let mut events = Vec::new();
        if let Some(event) = self.output_text_item_added_event() {
            events.push(event);
        }
        if !self.reasoning_summary_part_added {
            self.reasoning_summary_part_added = true;
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.reasoning_summary_part.added",
                serde_json::json!({
                    "type": "response.reasoning_summary_part.added",
                    "sequence_number": sequence_number,
                    "response_id": self.response_id,
                    "summary_index": 0,
                }),
            ));
        }
        let sequence_number = self.next_sequence_number();
        events.push(self.event(
            "response.reasoning_summary_text.delta",
            serde_json::json!({
                "type": "response.reasoning_summary_text.delta",
                "sequence_number": sequence_number,
                "response_id": self.response_id,
                "summary_index": 0,
                "delta": text,
            }),
        ));
        events
    }

    fn complete_output_text_item_events(&mut self) -> Vec<String> {
        if self.output_text.is_empty() || self.output_text_item_done {
            return Vec::new();
        }
        let mut events = Vec::new();
        if let Some(event) = self.output_text_item_added_event() {
            events.push(event);
        }
        self.output_text_item_done = true;
        let sequence_number = self.next_sequence_number();
        events.push(self.event(
            "response.output_item.done",
            serde_json::json!({
                "type": "response.output_item.done",
                "sequence_number": sequence_number,
                "response_id": self.response_id,
                "item": {
                    "id": self.output_text_item_id(),
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": self.output_text,
                    }],
                },
            }),
        ));
        events
    }

    fn media_item_id(&self) -> String {
        format!("msg_gemini_media_{}", self.request_id)
    }

    fn citation_item_id(&self) -> String {
        format!("msg_gemini_citations_{}", self.request_id)
    }

    fn observe_citation_text(&mut self, text: String) -> Vec<String> {
        if self.citation_item_done || text.is_empty() {
            return Vec::new();
        }
        self.citation_item_done = true;
        self.citation_text = Some(text.clone());
        let sequence_number = self.next_sequence_number();
        vec![self.event(
            "response.output_item.done",
            serde_json::json!({
                "type": "response.output_item.done",
                "sequence_number": sequence_number,
                "response_id": self.response_id,
                "item": {
                    "id": self.citation_item_id(),
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": text,
                    }],
                },
            }),
        )]
    }

    fn complete_media_item_events(&mut self) -> Vec<String> {
        if self.media_content_items.is_empty() || self.media_item_done {
            return Vec::new();
        }
        self.media_item_done = true;
        let sequence_number = self.next_sequence_number();
        vec![self.event(
            "response.output_item.done",
            serde_json::json!({
                "type": "response.output_item.done",
                "sequence_number": sequence_number,
                "response_id": self.response_id,
                "item": {
                    "id": self.media_item_id(),
                    "type": "message",
                    "role": "assistant",
                    "content": self.media_content_items.clone(),
                },
            }),
        )]
    }

    fn complete_image_generation_item_events(&mut self) -> Vec<String> {
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
                serde_json::json!({
                    "type": "response.output_item.done",
                    "sequence_number": sequence_number,
                    "response_id": self.response_id,
                    "item": item,
                }),
            ));
        }
        events
    }

    fn store_conversation_snapshot(&self) {
        runtime_deepseek_store_conversation(
            &self.conversations,
            &self.response_id,
            self.conversation_messages.clone(),
            self.chat_assistant_messages(),
        );
    }

    fn chat_assistant_messages(&self) -> Vec<serde_json::Value> {
        if self.output_text.is_empty()
            && self.reasoning_content.is_empty()
            && self.media_content_items.is_empty()
            && self.image_generation_items.is_empty()
            && self.tool_calls.is_empty()
        {
            return Vec::new();
        }
        let mut assistant = serde_json::json!({
            "role": "assistant",
            "content": if self.output_text.is_empty() {
                if self.tool_calls.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::String(String::new())
                }
            } else {
                serde_json::Value::String(self.output_text.clone())
            },
        });
        if !self.reasoning_content.is_empty() {
            assistant["reasoning_content"] =
                serde_json::Value::String(self.reasoning_content.clone());
        }
        if !self.media_content_items.is_empty() {
            assistant["gemini_media_content"] =
                serde_json::Value::Array(self.media_content_items.clone());
        }
        if !self.native_parts.is_empty() {
            assistant["gemini_native_parts"] = serde_json::Value::Array(self.native_parts.clone());
        }
        if !self.image_generation_items.is_empty() {
            assistant["gemini_image_generation"] =
                serde_json::Value::Array(self.image_generation_items.clone());
        }
        if let Some(metadata) = self.metadata.clone() {
            assistant["gemini_metadata"] = metadata;
        }
        if !self.tool_calls.is_empty() {
            assistant["tool_calls"] = serde_json::Value::Array(
                self.tool_calls
                    .iter()
                    .map(|(index, tool_call)| {
                        let mut item = serde_json::json!({
                            "id": tool_call
                                .call_id
                                .clone()
                                .unwrap_or_else(|| format!("call_gemini_{}_{}", self.request_id, index)),
                            "type": "function",
                            "function": {
                                "name": tool_call
                                    .name
                                    .clone()
                                    .unwrap_or_else(|| "tool_call".to_string()),
                                "arguments": tool_call.arguments.clone(),
                            },
                        });
                        if let Some(signature) = tool_call.thought_signature.as_deref() {
                            item["gemini_thought_signature"] =
                                serde_json::Value::String(signature.to_string());
                        }
                        item
                    })
                    .collect(),
            );
        }
        vec![assistant]
    }

    fn output_items(&self) -> Vec<serde_json::Value> {
        let mut output = Vec::new();
        if let Some(item) = self.web_search_call.clone() {
            output.push(item);
        }
        output.extend(self.image_generation_items.clone());
        if !self.output_text.is_empty() {
            let mut content = vec![serde_json::json!({
                "type": "output_text",
                "text": self.output_text,
            })];
            content.extend(self.media_content_items.clone());
            output.push(serde_json::json!({
                "type": "message",
                "role": "assistant",
                "content": content,
            }));
        } else if !self.media_content_items.is_empty() {
            output.push(serde_json::json!({
                "type": "message",
                "role": "assistant",
                "content": self.media_content_items.clone(),
            }));
        }
        if let Some(citations) = self.citation_text.clone() {
            output.push(serde_json::json!({
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": citations,
                }],
            }));
        }
        for (index, tool_call) in &self.tool_calls {
            let call_id = tool_call
                .call_id
                .clone()
                .unwrap_or_else(|| format!("call_gemini_{}_{}", self.request_id, index));
            let flat_name = tool_call
                .name
                .clone()
                .unwrap_or_else(|| "tool_call".to_string());
            let raw_arguments_value =
                serde_json::from_str::<serde_json::Value>(&tool_call.arguments)
                    .unwrap_or_else(|_| serde_json::Value::String(tool_call.arguments.clone()));
            if let Some(blocked) =
                runtime_gemini_blocked_tool_call_message(&flat_name, &raw_arguments_value)
            {
                output.push(runtime_gemini_blocked_tool_call_item(&blocked));
                continue;
            }
            if flat_name == "tool_search" {
                let arguments = serde_json::from_str::<serde_json::Value>(&tool_call.arguments)
                    .unwrap_or_else(|_| serde_json::json!({}));
                output.push(serde_json::json!({
                    "type": "tool_search_call",
                    "call_id": call_id,
                    "execution": "client",
                    "arguments": arguments,
                }));
                continue;
            }
            if let Ok(args_value) = serde_json::from_str::<serde_json::Value>(&tool_call.arguments)
                && let Some(item) =
                    runtime_gemini_custom_tool_call_item(&call_id, &flat_name, &args_value)
            {
                output.push(item);
                continue;
            }
            let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(&flat_name);
            let mut item = serde_json::json!({
                "type": "function_call",
                "call_id": call_id,
                "name": name,
                "arguments": tool_call.arguments.clone(),
            });
            if let Some(namespace) = namespace {
                item["namespace"] = serde_json::Value::String(namespace);
            }
            if let Some(tool_call) = self.tool_calls.get(index)
                && let Some(signature) = tool_call.thought_signature.clone()
            {
                item["gemini_thought_signature"] = serde_json::Value::String(signature);
            }
            output.push(item);
        }
        output
    }

    fn tool_call_ids(&self) -> Vec<String> {
        self.tool_calls
            .iter()
            .map(|(index, tool_call)| {
                tool_call
                    .call_id
                    .clone()
                    .unwrap_or_else(|| format!("call_gemini_{}_{}", self.request_id, index))
            })
            .filter(|call_id| !call_id.trim().is_empty())
            .collect()
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

fn runtime_gemini_blocked_tool_call_item(message: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": message,
        }],
    })
}

fn runtime_gemini_stream_error(value: &serde_json::Value) -> Option<(String, String)> {
    let error = value.get("error")?;
    let code = error
        .get("status")
        .or_else(|| error.get("code"))
        .and_then(|code| {
            code.as_str()
                .map(str::to_string)
                .or_else(|| code.as_i64().map(|code| code.to_string()))
        })
        .unwrap_or_else(|| "provider_stream_error".to_string());
    let message = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Gemini stream returned an embedded provider error")
        .to_string();
    Some((code, message))
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

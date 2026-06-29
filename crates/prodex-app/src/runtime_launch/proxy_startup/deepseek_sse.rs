use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_chat_tool_call_thought_signature,
    runtime_deepseek_created_at, runtime_deepseek_merge_response_metadata,
    runtime_deepseek_responses_usage, runtime_deepseek_rtk_wrapped_tool_arguments,
    runtime_deepseek_store_conversation,
};
use super::gemini_rewrite::runtime_gemini_custom_tool_input_from_arguments;
use super::provider_sse_events::{
    runtime_provider_sse_failed_event, runtime_provider_sse_output_text_item_added_event,
    runtime_provider_sse_output_text_item_done_event,
};
use super::provider_tools::runtime_provider_split_flat_namespace_tool_name;
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct RuntimeDeepSeekSseState {
    request_id: u64,
    response_id: String,
    created_at: u64,
    sequence_number: u64,
    created: bool,
    completed: bool,
    pub(super) eof: bool,
    output_text_item_added: bool,
    output_text_item_done: bool,
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

#[derive(Debug, Default)]
struct RuntimeDeepSeekToolCall {
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
    thought_signature: Option<String>,
    added: bool,
    done: bool,
}

impl RuntimeDeepSeekSseState {
    pub(super) fn new(
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        response_metadata: Option<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
    ) -> Self {
        Self {
            request_id,
            response_id: format!("resp_deepseek_{request_id}"),
            created_at: runtime_deepseek_created_at(),
            sequence_number: 0,
            created: false,
            completed: false,
            eof: false,
            output_text_item_added: false,
            output_text_item_done: false,
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
        if let Some((code, message)) = runtime_chat_stream_error(value) {
            return self
                .failed_event(&code, &message)
                .into_iter()
                .collect::<Vec<_>>();
        }
        if let Some(id) = value.get("id").and_then(serde_json::Value::as_str)
            && self.response_id.starts_with("resp_deepseek_")
        {
            self.response_id = id.to_string();
        }
        if let Some(model) = value.get("model").and_then(serde_json::Value::as_str) {
            self.model = Some(model.to_string());
        }
        if let Some(created) = value.get("created").and_then(serde_json::Value::as_u64) {
            self.created_at = created;
        }
        if let Some(system_fingerprint) = value
            .get("system_fingerprint")
            .and_then(serde_json::Value::as_str)
            .filter(|system_fingerprint| !system_fingerprint.is_empty())
        {
            self.system_fingerprint = Some(system_fingerprint.to_string());
        }
        if let Some(usage) = value
            .get("usage")
            .and_then(runtime_deepseek_responses_usage)
        {
            self.usage = Some(usage);
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
        let Some(choice) = value
            .get("choices")
            .and_then(serde_json::Value::as_array)
            .and_then(|choices| choices.first())
        else {
            return events;
        };
        if let Some(logprobs) = choice
            .get("logprobs")
            .filter(|logprobs| !logprobs.is_null())
        {
            self.logprobs = Some(logprobs.clone());
        }
        if let Some(delta) = choice.get("delta") {
            if let Some(reasoning_content) = delta
                .get("reasoning_content")
                .and_then(serde_json::Value::as_str)
                .filter(|text| !text.is_empty())
            {
                self.reasoning_content.push_str(reasoning_content);
            }
            if let Some(refusal) = delta
                .get("refusal")
                .and_then(serde_json::Value::as_str)
                .filter(|text| !text.is_empty())
            {
                self.refusal.push_str(refusal);
            }
            if let Some(annotations) = delta
                .get("annotations")
                .and_then(serde_json::Value::as_array)
            {
                self.annotations.extend(annotations.iter().cloned());
            }
            if let Some(text) = delta
                .get("content")
                .and_then(serde_json::Value::as_str)
                .filter(|text| !text.is_empty())
            {
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
            if let Some(tool_calls) = delta
                .get("tool_calls")
                .and_then(serde_json::Value::as_array)
            {
                for tool_call in tool_calls {
                    events.extend(self.observe_tool_call_delta(tool_call));
                }
            }
        }
        if let Some(finish_reason) = choice
            .get("finish_reason")
            .and_then(serde_json::Value::as_str)
        {
            self.finish_reason = Some(finish_reason.to_string());
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

    fn observe_tool_call_delta(&mut self, value: &serde_json::Value) -> Vec<String> {
        let index = value
            .get("index")
            .and_then(serde_json::Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .unwrap_or(0);
        let mut events = Vec::new();
        if value
            .get("function")
            .and_then(serde_json::Value::as_object)
            .is_none()
            && !self.tool_calls.contains_key(&index)
        {
            if let Some(event) = self.failed_event(
                "invalid_tool_call_arguments",
                "DeepSeek streamed a tool call without a function object",
            ) {
                events.push(event);
            }
            return events;
        }
        let argument_delta = value
            .get("function")
            .and_then(|function| function.get("arguments"))
            .and_then(serde_json::Value::as_str)
            .filter(|arguments| !arguments.is_empty())
            .map(str::to_string);
        let (call_id, name, should_add) = {
            let tool_call = self.tool_calls.entry(index).or_default();
            if let Some(id) = value.get("id").and_then(serde_json::Value::as_str) {
                tool_call.call_id = Some(id.to_string());
            }
            if let Some(signature) = runtime_deepseek_chat_tool_call_thought_signature(value) {
                tool_call.thought_signature = Some(signature);
            }
            if let Some(function) = value.get("function") {
                if let Some(name) = function.get("name").and_then(serde_json::Value::as_str) {
                    tool_call.name = Some(name.to_string());
                }
                if let Some(arguments) = argument_delta.as_deref() {
                    tool_call.arguments.push_str(arguments);
                }
            }
            let call_id = tool_call
                .call_id
                .clone()
                .unwrap_or_else(|| format!("call_deepseek_{}_{}", self.request_id, index));
            let name = tool_call
                .name
                .clone()
                .unwrap_or_else(|| "tool_call".to_string());
            let should_add = !tool_call.added;
            if should_add {
                tool_call.added = true;
            }
            (call_id, name, should_add)
        };
        if should_add && name != "tool_search" {
            if name == "apply_patch" {
                let sequence_number = self.next_sequence_number();
                events.push(self.event(
                    "response.output_item.added",
                    serde_json::json!({
                        "type": "response.output_item.added",
                        "sequence_number": sequence_number,
                        "item": {
                            "type": "custom_tool_call",
                            "call_id": call_id,
                            "name": name,
                            "input": "",
                        },
                    }),
                ));
                return events;
            }
            let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(&name);
            let mut item = serde_json::json!({
                "type": "function_call",
                "call_id": call_id,
                "name": name,
            });
            if let Some(namespace) = namespace {
                item["namespace"] = serde_json::Value::String(namespace);
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
        events
    }

    fn complete_tool_call_events(&mut self) -> Vec<String> {
        let mut events = Vec::new();
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
                    .unwrap_or_else(|| format!("call_deepseek_{}_{}", self.request_id, index));
                let name = tool_call
                    .name
                    .clone()
                    .unwrap_or_else(|| "tool_call".to_string());
                let arguments =
                    runtime_deepseek_rtk_wrapped_tool_arguments(&name, &tool_call.arguments);
                tool_call.arguments = arguments.clone();
                Some((
                    call_id,
                    name,
                    arguments,
                    tool_call.thought_signature.clone(),
                ))
            })
            .collect::<Vec<_>>();
        for (call_id, name, arguments, thought_signature) in pending {
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
            if !arguments.is_empty() {
                let sequence_number = self.next_sequence_number();
                events.push(self.event(
                    "response.function_call_arguments.delta",
                    serde_json::json!({
                        "type": "response.function_call_arguments.delta",
                        "sequence_number": sequence_number,
                        "call_id": call_id,
                        "delta": arguments,
                    }),
                ));
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
            if let Some(signature) = thought_signature {
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
        if let Err(message) = self.validate_tool_call_arguments() {
            return self.failed_event("invalid_tool_call_arguments", &message);
        }
        let mut events = self.complete_tool_call_events();
        events.extend(self.complete_output_text_item_events());
        let mut response = serde_json::json!({
            "id": self.response_id,
            "output": self.output_items(),
        });
        if let Some(model) = self.model.as_deref() {
            response["model"] = serde_json::Value::String(model.to_string());
        }
        if let Some(usage) = self.usage.clone() {
            response["usage"] = usage;
        }
        let mut metadata = serde_json::Map::new();
        if let Some(logprobs) = self.logprobs.clone() {
            metadata.insert("logprobs".to_string(), logprobs);
        }
        if !self.reasoning_content.is_empty() {
            metadata.insert(
                "reasoning_content".to_string(),
                serde_json::Value::String(self.reasoning_content.clone()),
            );
        }
        if !self.refusal.is_empty() {
            metadata.insert(
                "refusal".to_string(),
                serde_json::Value::String(self.refusal.clone()),
            );
        }
        if !self.annotations.is_empty() {
            metadata.insert(
                "annotations".to_string(),
                serde_json::Value::Array(self.annotations.clone()),
            );
        }
        if let Some(finish_reason) = self.finish_reason.clone() {
            metadata.insert(
                "finish_reason".to_string(),
                serde_json::Value::String(finish_reason),
            );
        }
        if let Some(system_fingerprint) = self.system_fingerprint.clone() {
            metadata.insert(
                "system_fingerprint".to_string(),
                serde_json::Value::String(system_fingerprint),
            );
        }
        if !metadata.is_empty() {
            response["metadata"] = serde_json::json!({ "deepseek": metadata });
        }
        runtime_deepseek_merge_response_metadata(&mut response, self.response_metadata.clone());
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
        self.completed = true;
        Some(events.join(""))
    }

    fn validate_tool_call_arguments(&self) -> Result<(), String> {
        for (index, tool_call) in &self.tool_calls {
            let Some(name) = tool_call
                .name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
            else {
                return Err(format!(
                    "DeepSeek streamed a tool call without a function name at index {index}"
                ));
            };
            if tool_call.arguments.trim().is_empty() {
                continue;
            }
            if let Err(error) = serde_json::from_str::<serde_json::Value>(&tool_call.arguments) {
                return Err(format!(
                    "DeepSeek streamed malformed JSON arguments for tool call `{name}` at index {index}: {error}"
                ));
            }
        }
        Ok(())
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

    fn output_text_item_id(&self) -> String {
        format!("msg_deepseek_{}", self.request_id)
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
        events.push(runtime_provider_sse_output_text_item_done_event(
            sequence_number,
            &self.response_id,
            &self.output_text_item_id(),
            &self.output_text,
        ));
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
        if !self.tool_calls.is_empty() {
            assistant["tool_calls"] = serde_json::Value::Array(
                self.tool_calls
                    .iter()
                    .map(|(index, tool_call)| {
                        let mut tool_call_value = serde_json::json!({
                            "id": tool_call
                                .call_id
                                .clone()
                                .unwrap_or_else(|| format!("call_deepseek_{}_{}", self.request_id, index)),
                            "type": "function",
                            "function": {
                                "name": tool_call
                                    .name
                                    .clone()
                                    .unwrap_or_else(|| "tool_call".to_string()),
                                "arguments": tool_call.arguments,
                            },
                        });
                        if let Some(signature) = tool_call.thought_signature.as_deref() {
                            tool_call_value["extra_content"] = serde_json::json!({
                                "google": {
                                    "thought_signature": signature,
                                }
                            });
                        }
                        tool_call_value
                    })
                    .collect(),
            );
        }
        vec![assistant]
    }

    fn output_items(&self) -> Vec<serde_json::Value> {
        let mut output = Vec::new();
        if !self.output_text.is_empty() {
            output.push(serde_json::json!({
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": self.output_text,
                }],
            }));
        }
        for (index, tool_call) in &self.tool_calls {
            let call_id = tool_call
                .call_id
                .clone()
                .unwrap_or_else(|| format!("call_deepseek_{}_{}", self.request_id, index));
            let flat_name = tool_call
                .name
                .clone()
                .unwrap_or_else(|| "tool_call".to_string());
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
            if flat_name == "apply_patch" {
                output.push(serde_json::json!({
                    "type": "custom_tool_call",
                    "call_id": call_id,
                    "name": flat_name,
                    "input": runtime_gemini_custom_tool_input_from_arguments(&tool_call.arguments),
                }));
                continue;
            }
            let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(&flat_name);
            let mut item = serde_json::json!({
                "type": "function_call",
                "call_id": call_id,
                "name": name,
                "arguments": tool_call.arguments,
            });
            if let Some(namespace) = namespace {
                item["namespace"] = serde_json::Value::String(namespace);
            }
            if let Some(signature) = tool_call.thought_signature.as_deref() {
                item["gemini_thought_signature"] = serde_json::Value::String(signature.to_string());
            }
            output.push(item);
        }
        output
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

fn runtime_chat_stream_error(value: &serde_json::Value) -> Option<(String, String)> {
    let error = value.get("error")?;
    if let Some(message) = error.as_str() {
        return Some(("provider_stream_error".to_string(), message.to_string()));
    }
    let code = error
        .get("type")
        .or_else(|| error.get("code"))
        .and_then(|code| {
            code.as_str()
                .map(str::to_string)
                .or_else(|| code.as_i64().map(|code| code.to_string()))
        })
        .unwrap_or_else(|| "provider_stream_error".to_string());
    let message = error
        .get("message")
        .or_else(|| error.get("detail"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Provider stream returned an embedded error")
        .to_string();
    Some((code, message))
}

#[cfg(test)]
#[path = "deepseek_sse_tests.rs"]
mod tests;

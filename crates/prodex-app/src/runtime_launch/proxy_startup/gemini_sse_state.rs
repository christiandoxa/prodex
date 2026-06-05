use super::super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_created_at,
    runtime_deepseek_rtk_wrapped_tool_arguments, runtime_deepseek_store_conversation,
};
use super::super::gemini_rewrite::{
    runtime_gemini_custom_tool_call_item, runtime_gemini_custom_tool_input_from_arguments,
    runtime_gemini_responses_usage, runtime_gemini_web_search_call_from_grounding,
};
use super::super::gemini_thought_signatures::runtime_gemini_thought_signature;
use super::super::provider_tools::runtime_provider_split_flat_namespace_tool_name;
use super::RuntimeGeminiBindingRecorder;
use prodex_cli::SUPER_GEMINI_DEFAULT_MODEL;
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
    model: Option<String>,
    output_text: String,
    reasoning_content: String,
    tool_calls: BTreeMap<usize, RuntimeGeminiToolCall>,
    usage: Option<serde_json::Value>,
    web_search_call: Option<serde_json::Value>,
    conversation_messages: Vec<serde_json::Value>,
    conversations: RuntimeDeepSeekConversationStore,
    binding_recorder: Option<RuntimeGeminiBindingRecorder>,
}

#[derive(Debug, Default)]
struct RuntimeGeminiToolCall {
    call_id: Option<String>,
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
            model: None,
            output_text: String::new(),
            reasoning_content: String::new(),
            tool_calls: BTreeMap::new(),
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
        for part in parts {
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
            if let Some(function_call) = part.get("functionCall") {
                let thought_signature = runtime_gemini_thought_signature(part)
                    .or_else(|| runtime_gemini_thought_signature(function_call));
                events.extend(self.observe_function_call(function_call, thought_signature));
            }
        }
        if value
            .get("candidates")
            .and_then(serde_json::Value::as_array)
            .and_then(|candidates| candidates.first())
            .and_then(|candidate| candidate.get("finishReason"))
            .and_then(serde_json::Value::as_str)
            .is_some()
        {
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
        value: &serde_json::Value,
        thought_signature: Option<String>,
    ) -> Vec<String> {
        let index = self.tool_calls.len();
        let name = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("tool_call")
            .to_string();
        let args = value
            .get("args")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let args = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
        let mut events = Vec::new();
        let (call_id, should_add) = {
            let tool_call = self.tool_calls.entry(index).or_default();
            tool_call.call_id = value
                .get("id")
                .and_then(serde_json::Value::as_str)
                .filter(|id| !id.trim().is_empty())
                .map(str::to_string)
                .or_else(|| Some(format!("call_gemini_{}_{}", self.request_id, index)));
            tool_call.name = Some(name.clone());
            tool_call.arguments = args;
            if thought_signature.is_some() {
                tool_call.thought_signature = thought_signature;
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
                let arguments = if name == "apply_patch" {
                    tool_call.arguments.clone()
                } else {
                    runtime_deepseek_rtk_wrapped_tool_arguments(&name, &tool_call.arguments)
                };
                tool_call.arguments = arguments.clone();
                Some((call_id, name, arguments))
            })
            .collect::<Vec<_>>();
        let mut events = Vec::new();
        for (call_id, name, arguments) in pending {
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
        let mut response = serde_json::json!({
            "id": self.response_id,
            "output": self.output_items(),
        });
        response["model"] = serde_json::Value::String(
            self.model
                .clone()
                .unwrap_or_else(|| SUPER_GEMINI_DEFAULT_MODEL.to_string()),
        );
        if let Some(usage) = self.usage.clone() {
            response["usage"] = usage;
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
                                "arguments": tool_call.arguments,
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
                .unwrap_or_else(|| format!("call_gemini_{}_{}", self.request_id, index));
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
                "arguments": tool_call.arguments,
            });
            if let Some(namespace) = namespace {
                item["namespace"] = serde_json::Value::String(namespace);
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

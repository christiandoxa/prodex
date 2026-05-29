use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_created_at,
    runtime_deepseek_rtk_wrapped_tool_arguments, runtime_deepseek_store_conversation,
};
use super::gemini_rewrite::{
    runtime_gemini_normalized_response_value, runtime_gemini_responses_usage,
};
use prodex_cli::SUPER_GEMINI_DEFAULT_MODEL;
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::sync::Arc;

pub(super) type RuntimeGeminiBindingRecorder = Arc<dyn Fn(String, Vec<String>) + Send + Sync>;

pub(super) struct RuntimeGeminiGenerateSseReader<R: Read> {
    reader: std::io::BufReader<R>,
    pending: Vec<u8>,
    pending_offset: usize,
    state: RuntimeGeminiSseState,
}

impl<R: Read> RuntimeGeminiGenerateSseReader<R> {
    pub(super) fn new(
        reader: R,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
        binding_recorder: Option<RuntimeGeminiBindingRecorder>,
    ) -> Self {
        Self {
            reader: std::io::BufReader::new(reader),
            pending: Vec::new(),
            pending_offset: 0,
            state: RuntimeGeminiSseState::new(
                request_id,
                conversation_messages,
                conversations,
                binding_recorder,
            ),
        }
    }

    fn fill_pending(&mut self) -> io::Result<()> {
        while self.pending_offset >= self.pending.len() && !self.state.eof {
            self.pending.clear();
            self.pending_offset = 0;
            let Some(data_lines) = self.read_next_sse_data_lines()? else {
                if let Some(event) = self.state.complete_event() {
                    self.pending = event.into_bytes();
                }
                self.state.eof = true;
                break;
            };
            for data in data_lines {
                if data.trim() == "[DONE]" {
                    if let Some(event) = self.state.complete_event() {
                        self.pending.extend_from_slice(event.as_bytes());
                    }
                    self.state.eof = true;
                    break;
                }
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) {
                    let value = runtime_gemini_normalized_response_value(&value);
                    let events = self.state.observe_generate_chunk(&value);
                    for event in events {
                        self.pending.extend_from_slice(event.as_bytes());
                    }
                }
            }
        }
        Ok(())
    }

    fn read_next_sse_data_lines(&mut self) -> io::Result<Option<Vec<String>>> {
        let mut data_lines = Vec::new();
        loop {
            let mut line = String::new();
            let read = std::io::BufRead::read_line(&mut self.reader, &mut line)?;
            if read == 0 {
                return if data_lines.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(data_lines))
                };
            }
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if data_lines.is_empty() {
                    continue;
                }
                return Ok(Some(data_lines));
            }
            if let Some(data) = line.strip_prefix("data:") {
                data_lines.push(data.trim_start().to_string());
            }
        }
    }
}

impl<R: Read> Read for RuntimeGeminiGenerateSseReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.fill_pending()?;
        if self.pending_offset >= self.pending.len() {
            return Ok(0);
        }
        let len = buf.len().min(self.pending.len() - self.pending_offset);
        buf[..len].copy_from_slice(&self.pending[self.pending_offset..self.pending_offset + len]);
        self.pending_offset += len;
        Ok(len)
    }
}

struct RuntimeGeminiSseState {
    request_id: u64,
    response_id: String,
    created_at: u64,
    sequence_number: u64,
    created: bool,
    completed: bool,
    eof: bool,
    output_text_item_added: bool,
    output_text_item_done: bool,
    model: Option<String>,
    output_text: String,
    reasoning_content: String,
    tool_calls: BTreeMap<usize, RuntimeGeminiToolCall>,
    usage: Option<serde_json::Value>,
    conversation_messages: Vec<serde_json::Value>,
    conversations: RuntimeDeepSeekConversationStore,
    binding_recorder: Option<RuntimeGeminiBindingRecorder>,
}

#[derive(Debug, Default)]
struct RuntimeGeminiToolCall {
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
    added: bool,
    done: bool,
}

impl RuntimeGeminiSseState {
    fn new(
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
            conversation_messages,
            conversations,
            binding_recorder,
        }
    }

    fn observe_generate_chunk(&mut self, value: &serde_json::Value) -> Vec<String> {
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
                events.extend(self.observe_function_call(function_call));
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

    fn observe_function_call(&mut self, value: &serde_json::Value) -> Vec<String> {
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
            let call_id = tool_call.call_id.clone().unwrap_or_default();
            let should_add = !tool_call.added;
            if should_add {
                tool_call.added = true;
            }
            (call_id, should_add)
        };
        if should_add {
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.output_item.added",
                serde_json::json!({
                    "type": "response.output_item.added",
                    "sequence_number": sequence_number,
                    "item": {
                        "type": "function_call",
                        "call_id": call_id,
                        "name": name,
                    },
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
                let arguments =
                    runtime_deepseek_rtk_wrapped_tool_arguments(&name, &tool_call.arguments);
                tool_call.arguments = arguments.clone();
                Some((call_id, name, arguments))
            })
            .collect::<Vec<_>>();
        let mut events = Vec::new();
        for (call_id, name, arguments) in pending {
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
            let sequence_number = self.next_sequence_number();
            events.push(self.event(
                "response.output_item.done",
                serde_json::json!({
                    "type": "response.output_item.done",
                    "sequence_number": sequence_number,
                    "item": {
                        "type": "function_call",
                        "call_id": call_id,
                        "name": name,
                        "arguments": arguments,
                    },
                }),
            ));
        }
        events
    }

    fn complete_event(&mut self) -> Option<String> {
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
                        serde_json::json!({
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
                        })
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
            output.push(serde_json::json!({
                "type": "function_call",
                "call_id": tool_call
                    .call_id
                    .clone()
                    .unwrap_or_else(|| format!("call_gemini_{}_{}", self.request_id, index)),
                "name": tool_call
                    .name
                    .clone()
                    .unwrap_or_else(|| "tool_call".to_string()),
                "arguments": tool_call.arguments,
            }));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn conversation_store() -> RuntimeDeepSeekConversationStore {
        Arc::new(Mutex::new(BTreeMap::new()))
    }

    #[test]
    fn gemini_sse_reader_maps_text_and_function_call_to_responses_events() {
        let stream = concat!(
            "data: {\"responseId\":\"resp_1\",\"modelVersion\":\"gemini-2.5-pro\",\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]}}]}\n\n",
            "data: {\"responseId\":\"resp_1\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"shell\",\"args\":{\"cmd\":\"ls\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let mut reader = RuntimeGeminiGenerateSseReader::new(
            std::io::Cursor::new(stream.as_bytes()),
            9,
            Vec::new(),
            conversation_store(),
            None,
        );
        let mut output = String::new();
        reader.read_to_string(&mut output).unwrap();

        assert!(output.contains("event: response.created"));
        assert!(output.contains("\"type\":\"response.output_text.delta\""));
        assert!(output.contains("\"delta\":\"hi\""));
        assert!(output.contains("\"type\":\"response.output_item.added\""));
        assert!(output.contains("\"type\":\"response.function_call_arguments.delta\""));
        assert!(output.contains("\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\""));
        assert!(output.contains("event: response.completed"));
    }

    #[test]
    fn gemini_sse_reader_records_response_and_tool_call_bindings() {
        let captured = Arc::new(Mutex::new(None::<(String, Vec<String>)>));
        let captured_for_recorder = Arc::clone(&captured);
        let recorder: RuntimeGeminiBindingRecorder = Arc::new(move |response_id, call_ids| {
            *captured_for_recorder.lock().unwrap() = Some((response_id, call_ids));
        });
        let stream = concat!(
            "data: {\"responseId\":\"resp_1\",\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"id\":\"call_1\",\"name\":\"shell\",\"args\":{\"cmd\":\"ls\"}}}]},\"finishReason\":\"STOP\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let mut reader = RuntimeGeminiGenerateSseReader::new(
            std::io::Cursor::new(stream.as_bytes()),
            9,
            Vec::new(),
            conversation_store(),
            Some(recorder),
        );
        let mut output = String::new();
        reader.read_to_string(&mut output).unwrap();

        let (response_id, call_ids) = captured.lock().unwrap().clone().unwrap();
        assert_eq!(response_id, "resp_1");
        assert_eq!(call_ids, vec!["call_1"]);
    }
}

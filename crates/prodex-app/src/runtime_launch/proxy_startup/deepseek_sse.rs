use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_created_at,
    runtime_deepseek_responses_usage, runtime_deepseek_rtk_wrapped_tool_arguments,
    runtime_deepseek_store_conversation,
};
use std::collections::BTreeMap;
use std::io::{self, Read};

pub(super) struct RuntimeDeepSeekChatSseReader<R: Read> {
    reader: std::io::BufReader<R>,
    pending: Vec<u8>,
    pending_offset: usize,
    accumulated_json: String,
    state: RuntimeDeepSeekSseState,
}

impl<R: Read> RuntimeDeepSeekChatSseReader<R> {
    pub(super) fn new(
        reader: R,
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
        conversations: RuntimeDeepSeekConversationStore,
    ) -> Self {
        Self {
            reader: std::io::BufReader::new(reader),
            pending: Vec::new(),
            pending_offset: 0,
            accumulated_json: String::new(),
            state: RuntimeDeepSeekSseState::new(request_id, conversation_messages, conversations),
        }
    }

    fn fill_pending(&mut self) -> io::Result<()> {
        while self.pending_offset >= self.pending.len() && !self.state.eof {
            self.pending.clear();
            self.pending_offset = 0;
            let data_lines = match self.read_next_sse_data_lines() {
                Ok(data_lines) => data_lines,
                Err(err) => {
                    if let Some(event) = self
                        .state
                        .failed_event("provider_stream_error", &err.to_string())
                    {
                        self.pending = event.into_bytes();
                        self.state.eof = true;
                        break;
                    }
                    return Err(err);
                }
            };
            let Some(data_lines) = data_lines else {
                if !self.accumulated_json.trim().is_empty() {
                    if let Some(events) = self.flush_accumulated_json() {
                        self.pending = events.into_bytes();
                        continue;
                    }
                    if let Some(event) = self
                        .state
                        .failed_event("provider_stream_error", "incomplete JSON segment")
                    {
                        self.pending = event.into_bytes();
                    }
                    self.state.eof = true;
                    break;
                }
                if let Some(event) = self.state.complete_event() {
                    self.pending = event.into_bytes();
                }
                self.state.eof = true;
                break;
            };
            if data_lines.iter().any(|data| data.trim() == "[DONE]") {
                if !self.accumulated_json.trim().is_empty() {
                    if let Some(events) = self.flush_accumulated_json() {
                        self.pending.extend_from_slice(events.as_bytes());
                    } else if let Some(event) = self
                        .state
                        .failed_event("provider_stream_error", "incomplete JSON segment")
                    {
                        self.pending.extend_from_slice(event.as_bytes());
                        self.state.eof = true;
                        break;
                    }
                }
                if let Some(event) = self.state.complete_event() {
                    self.pending.extend_from_slice(event.as_bytes());
                }
                self.state.eof = true;
                break;
            }
            for event in self.observe_sse_data_lines(&data_lines) {
                self.pending.extend_from_slice(event.as_bytes());
            }
        }
        Ok(())
    }

    fn observe_sse_data_lines(&mut self, data_lines: &[String]) -> Vec<String> {
        let joined = data_lines.join("\n");
        if let Some(events) = self.observe_json_text(&joined) {
            return events;
        }

        let mut separate_values = Vec::new();
        for line in data_lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                separate_values.clear();
                break;
            };
            separate_values.push(value);
        }
        if !separate_values.is_empty() {
            let mut events = Vec::new();
            for value in separate_values {
                events.extend(self.state.observe_chat_chunk(&value));
            }
            return events;
        }

        self.accumulated_json.push_str(joined.trim());
        self.flush_accumulated_json()
            .map(|events| vec![events])
            .unwrap_or_default()
    }

    fn observe_json_text(&mut self, data: &str) -> Option<Vec<String>> {
        let data = data.trim();
        if data.is_empty() {
            return Some(Vec::new());
        }
        serde_json::from_str::<serde_json::Value>(data)
            .ok()
            .map(|value| self.state.observe_chat_chunk(&value))
    }

    fn flush_accumulated_json(&mut self) -> Option<String> {
        if self.accumulated_json.trim().is_empty() {
            self.accumulated_json.clear();
            return None;
        }
        let value = serde_json::from_str::<serde_json::Value>(&self.accumulated_json).ok()?;
        self.accumulated_json.clear();
        let events = self.state.observe_chat_chunk(&value);
        (!events.is_empty()).then(|| events.join(""))
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

impl<R: Read> Read for RuntimeDeepSeekChatSseReader<R> {
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

#[derive(Debug)]
struct RuntimeDeepSeekSseState {
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
    tool_calls: BTreeMap<usize, RuntimeDeepSeekToolCall>,
    usage: Option<serde_json::Value>,
    conversation_messages: Vec<serde_json::Value>,
    conversations: RuntimeDeepSeekConversationStore,
}

#[derive(Debug, Default)]
struct RuntimeDeepSeekToolCall {
    call_id: Option<String>,
    name: Option<String>,
    arguments: String,
    added: bool,
    done: bool,
}

impl RuntimeDeepSeekSseState {
    fn new(
        request_id: u64,
        conversation_messages: Vec<serde_json::Value>,
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
            tool_calls: BTreeMap::new(),
            usage: None,
            conversation_messages,
            conversations,
        }
    }

    fn observe_chat_chunk(&mut self, value: &serde_json::Value) -> Vec<String> {
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
        if let Some(delta) = choice.get("delta") {
            if let Some(reasoning_content) = delta
                .get("reasoning_content")
                .and_then(serde_json::Value::as_str)
                .filter(|text| !text.is_empty())
            {
                self.reasoning_content.push_str(reasoning_content);
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
        if choice
            .get("finish_reason")
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

    fn observe_tool_call_delta(&mut self, value: &serde_json::Value) -> Vec<String> {
        let index = value
            .get("index")
            .and_then(serde_json::Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .unwrap_or(0);
        let mut events = Vec::new();
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
                Some((call_id, name, arguments))
            })
            .collect::<Vec<_>>();
        for (call_id, name, arguments) in pending {
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
        if let Some(model) = self.model.as_deref() {
            response["model"] = serde_json::Value::String(model.to_string());
        }
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
        self.completed = true;
        Some(events.join(""))
    }

    fn failed_event(&mut self, code: &str, message: &str) -> Option<String> {
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
        format!("msg_deepseek_{}", self.request_id)
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
                                .unwrap_or_else(|| format!("call_deepseek_{}_{}", self.request_id, index)),
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
                    .unwrap_or_else(|| format!("call_deepseek_{}_{}", self.request_id, index)),
                "name": tool_call
                    .name
                    .clone()
                    .unwrap_or_else(|| "tool_call".to_string()),
                "arguments": tool_call.arguments,
            }));
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

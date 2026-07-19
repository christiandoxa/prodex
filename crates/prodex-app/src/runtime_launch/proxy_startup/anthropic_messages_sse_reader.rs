use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
use super::deepseek_sse::RuntimeDeepSeekSseState;
use super::provider_bridge::RuntimeProviderBridgeKind;
use super::provider_sse_reader::{RuntimeProviderSseJsonReader, RuntimeProviderSseJsonState};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};

pub(super) struct RuntimeAnthropicMessagesSseReader<R: Read> {
    inner: RuntimeProviderSseJsonReader<R, RuntimeAnthropicMessagesSseState>,
}

impl<R: Read> RuntimeAnthropicMessagesSseReader<R> {
    #[cfg(test)]
    pub(super) fn new(
        reader: R,
        request_id: u64,
        conversation_messages: Vec<Value>,
        response_metadata: Option<Value>,
        conversations: RuntimeDeepSeekConversationStore,
    ) -> Self {
        Self::new_with_provider(
            reader,
            RuntimeProviderBridgeKind::Anthropic,
            request_id,
            conversation_messages,
            response_metadata,
            conversations,
        )
    }

    pub(super) fn new_with_provider(
        reader: R,
        provider_kind: RuntimeProviderBridgeKind,
        request_id: u64,
        conversation_messages: Vec<Value>,
        response_metadata: Option<Value>,
        conversations: RuntimeDeepSeekConversationStore,
    ) -> Self {
        Self {
            inner: RuntimeProviderSseJsonReader::new_with_observer(
                reader,
                RuntimeAnthropicMessagesSseState {
                    inner: RuntimeDeepSeekSseState::new_with_provider(
                        provider_kind,
                        request_id,
                        conversation_messages,
                        response_metadata,
                        conversations,
                    ),
                    input_tokens: 0,
                    output_tokens: 0,
                    web_searches: BTreeMap::new(),
                    ignored_blocks: BTreeSet::new(),
                },
                None,
            ),
        }
    }
}

impl<R: Read> Read for RuntimeAnthropicMessagesSseReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

struct RuntimeAnthropicMessagesSseState {
    inner: RuntimeDeepSeekSseState,
    input_tokens: u64,
    output_tokens: u64,
    web_searches: BTreeMap<u64, RuntimeAnthropicWebSearch>,
    ignored_blocks: BTreeSet<u64>,
}

struct RuntimeAnthropicWebSearch {
    id: String,
    input_json: String,
    sources: Vec<Value>,
    done: bool,
}

impl RuntimeProviderSseJsonState for RuntimeAnthropicMessagesSseState {
    fn eof(&self) -> bool {
        self.inner.eof
    }

    fn set_eof(&mut self, eof: bool) {
        self.inner.eof = eof;
    }

    fn observe_value(&mut self, value: &Value) -> Vec<String> {
        match value.get("type").and_then(Value::as_str) {
            Some("message_start") => {
                let usage = self.observe_usage(value.pointer("/message/usage"));
                self.inner.observe_chat_chunk(&json!({
                    "id": value.pointer("/message/id"),
                    "model": value.pointer("/message/model"),
                    "choices": [{"delta": {"role": "assistant"}, "finish_reason": null}],
                    "usage": usage,
                }))
            }
            Some("content_block_start") => {
                let Some(block) = value.get("content_block") else {
                    return self.failed("invalid_anthropic_stream", "content block is missing");
                };
                let index = value.get("index").and_then(Value::as_u64).unwrap_or(0);
                match block.get("type").and_then(Value::as_str) {
                    Some("text") | Some("thinking") => Vec::new(),
                    Some("tool_use") => self.inner.observe_chat_chunk(&json!({
                        "choices": [{
                            "delta": {"tool_calls": [{
                                "index": value.get("index").and_then(Value::as_u64).unwrap_or(0),
                                "id": block.get("id"),
                                "type": "function",
                                "function": {"name": block.get("name"), "arguments": ""},
                            }]},
                            "finish_reason": null,
                        }]
                    })),
                    Some("server_tool_use")
                        if block.get("name").and_then(Value::as_str) == Some("web_search") =>
                    {
                        self.start_web_search(index, block)
                    }
                    Some("web_search_tool_result") => {
                        self.ignored_blocks.insert(index);
                        self.finish_web_search_result(block)
                    }
                    Some(_) => Vec::new(),
                    None => {
                        self.failed("invalid_anthropic_stream", "content block type is missing")
                    }
                }
            }
            Some("content_block_delta") => {
                let index = value.get("index").and_then(Value::as_u64).unwrap_or(0);
                if self.ignored_blocks.contains(&index) {
                    return Vec::new();
                }
                if let Some(search) = self.web_searches.get_mut(&index) {
                    if value.pointer("/delta/type").and_then(Value::as_str)
                        == Some("input_json_delta")
                        && let Some(delta) =
                            value.pointer("/delta/partial_json").and_then(Value::as_str)
                    {
                        search.input_json.push_str(delta);
                    }
                    return Vec::new();
                }
                match value.pointer("/delta/type").and_then(Value::as_str) {
                    Some("text_delta") => self.inner.observe_chat_chunk(&json!({
                        "choices": [{
                            "delta": {"content": value.pointer("/delta/text")},
                            "finish_reason": null,
                        }]
                    })),
                    Some("input_json_delta") => self.inner.observe_chat_chunk(&json!({
                        "choices": [{
                            "delta": {"tool_calls": [{
                                "index": index,
                                "function": {"arguments": value.pointer("/delta/partial_json")},
                            }]},
                            "finish_reason": null,
                        }]
                    })),
                    Some("thinking_delta") => self.inner.observe_reasoning_delta(
                        value,
                        value
                            .pointer("/delta/thinking")
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                    ),
                    Some("signature_delta") | Some(_) => Vec::new(),
                    None => {
                        self.failed("invalid_anthropic_stream", "content delta type is missing")
                    }
                }
            }
            Some("message_delta") => {
                let usage = self.observe_usage(value.get("usage"));
                self.inner.observe_chat_chunk(&json!({
                    "choices": [{
                        "delta": {},
                        "finish_reason": anthropic_finish_reason(
                            value.pointer("/delta/stop_reason").and_then(Value::as_str)
                        ),
                    }],
                    "usage": usage,
                }))
            }
            Some("message_stop") => {
                self.inner.eof = true;
                let mut events = self.complete_web_searches();
                events.extend(self.inner.complete_event());
                events
            }
            Some("error") => {
                self.inner.eof = true;
                self.failed(
                    value
                        .pointer("/error/type")
                        .and_then(Value::as_str)
                        .unwrap_or("anthropic_stream_error"),
                    value
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or("Anthropic stream failed"),
                )
            }
            // Anthropic permits ping and future event types; transport stays open.
            Some("content_block_stop") => {
                if let Some(index) = value.get("index").and_then(Value::as_u64) {
                    self.ignored_blocks.remove(&index);
                }
                Vec::new()
            }
            Some("ping") | Some(_) | None => Vec::new(),
        }
    }

    fn complete_event(&mut self) -> Option<String> {
        self.inner.complete_event()
    }

    fn failed_event(&mut self, code: &str, message: &str) -> Option<String> {
        self.inner.failed_event(code, message)
    }
}

impl RuntimeAnthropicMessagesSseState {
    fn start_web_search(&mut self, index: u64, block: &Value) -> Vec<String> {
        let id = block
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("web_search_{index}"));
        let input_json = block
            .get("input")
            .filter(|input| input.as_object().is_some_and(|input| !input.is_empty()))
            .and_then(|input| serde_json::to_string(input).ok())
            .unwrap_or_default();
        let search = RuntimeAnthropicWebSearch {
            id,
            input_json,
            sources: Vec::new(),
            done: false,
        };
        let item = anthropic_web_search_stream_item(&search, "in_progress");
        self.web_searches.insert(index, search);
        let sequence_number = self.inner.next_sequence_number();
        vec![self.inner.event(
            "response.output_item.added",
            json!({
                "type": "response.output_item.added",
                "sequence_number": sequence_number,
                "output_index": index,
                "item": item,
            }),
        )]
    }

    fn finish_web_search_result(&mut self, block: &Value) -> Vec<String> {
        let Some(tool_use_id) = block.get("tool_use_id").and_then(Value::as_str) else {
            return Vec::new();
        };
        let Some(index) = self
            .web_searches
            .iter()
            .find_map(|(index, search)| (search.id == tool_use_id).then_some(*index))
        else {
            return Vec::new();
        };
        if let Some(search) = self.web_searches.get_mut(&index) {
            search.sources = anthropic_web_search_stream_sources(block);
        }
        self.complete_web_search(index).into_iter().collect()
    }

    fn complete_web_searches(&mut self) -> Vec<String> {
        let indexes = self.web_searches.keys().copied().collect::<Vec<_>>();
        indexes
            .into_iter()
            .filter_map(|index| self.complete_web_search(index))
            .collect()
    }

    fn complete_web_search(&mut self, index: u64) -> Option<String> {
        let item = {
            let search = self.web_searches.get_mut(&index)?;
            if search.done {
                return None;
            }
            search.done = true;
            anthropic_web_search_stream_item(search, "completed")
        };
        self.inner.record_external_output_item(item.clone());
        let sequence_number = self.inner.next_sequence_number();
        Some(self.inner.event(
            "response.output_item.done",
            json!({
                "type": "response.output_item.done",
                "sequence_number": sequence_number,
                "output_index": index,
                "item": item,
            }),
        ))
    }

    fn failed(&mut self, code: &str, message: &str) -> Vec<String> {
        self.inner.failed_event(code, message).into_iter().collect()
    }

    fn observe_usage(&mut self, usage: Option<&Value>) -> Value {
        if let Some(input_tokens) = usage
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(Value::as_u64)
        {
            self.input_tokens = input_tokens;
        }
        if let Some(output_tokens) = usage
            .and_then(|usage| usage.get("output_tokens"))
            .and_then(Value::as_u64)
        {
            self.output_tokens = output_tokens;
        }
        anthropic_chat_usage(self.input_tokens, self.output_tokens)
    }
}

fn anthropic_web_search_stream_item(search: &RuntimeAnthropicWebSearch, status: &str) -> Value {
    let input = serde_json::from_str::<Value>(&search.input_json).unwrap_or_else(|_| json!({}));
    let queries = input
        .get("query")
        .and_then(Value::as_str)
        .map(|query| vec![Value::String(query.to_string())])
        .or_else(|| input.get("queries").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    json!({
        "type": "web_search_call",
        "id": search.id,
        "status": status,
        "action": {
            "type": "search",
            "queries": queries,
            "sources": search.sources,
        },
    })
}

fn anthropic_web_search_stream_sources(block: &Value) -> Vec<Value> {
    block
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|result| {
            let url = result.get("url").and_then(Value::as_str)?;
            let mut source = json!({"type": "url", "url": url});
            if let Some(title) = result.get("title").and_then(Value::as_str) {
                source["title"] = Value::String(title.to_string());
            }
            Some(source)
        })
        .collect()
}

fn anthropic_finish_reason(reason: Option<&str>) -> Option<&'static str> {
    reason.map(|reason| match reason {
        "tool_use" => "tool_calls",
        "max_tokens" => "length",
        _ => "stop",
    })
}

fn anthropic_chat_usage(input: u64, output: u64) -> Value {
    json!({
        "prompt_tokens": input,
        "completion_tokens": output,
        "total_tokens": input.saturating_add(output),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn render(stream: &str) -> String {
        let mut output = String::new();
        RuntimeAnthropicMessagesSseReader::new(
            stream.as_bytes(),
            7,
            Vec::new(),
            None,
            Default::default(),
        )
        .read_to_string(&mut output)
        .unwrap();
        output
    }

    #[test]
    fn native_anthropic_stream_tolerates_ping_and_translates_text() {
        let output = render(concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_test\",\"model\":\"claude-sonnet-4-6\",\"usage\":{\"input_tokens\":2,\"output_tokens\":0}}}\n\n",
            "event: ping\ndata: {\"type\":\"ping\"}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ));
        assert!(output.contains("response.created"), "{output}");
        assert!(output.contains("response.output_text.delta"), "{output}");
        assert!(output.contains("hello"), "{output}");
        assert!(output.contains("response.completed"), "{output}");
        assert!(output.contains("\"input_tokens\":2"), "{output}");
        assert!(output.contains("\"output_tokens\":1"), "{output}");
        assert!(output.contains("\"total_tokens\":3"), "{output}");
    }

    #[test]
    fn native_anthropic_stream_accumulates_tool_input_json() {
        let output = render(concat!(
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_tool\",\"model\":\"claude-sonnet-4-6\"}}\n\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call_test\",\"name\":\"read_file\",\"input\":{}}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"/tmp/test\\\"}\"}}\n\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":4}}\n\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        ));
        assert!(output.contains("response.output_item.added"), "{output}");
        assert!(output.contains("call_test"), "{output}");
        assert!(output.contains("response.output_item.done"), "{output}");
        assert!(output.contains("/tmp/test"), "{output}");
    }

    #[test]
    fn native_anthropic_stream_preserves_web_search_call_and_sources() {
        let output = render(concat!(
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_search\",\"model\":\"deepseek-chat\"}}\n\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"server_tool_use\",\"id\":\"srv_1\",\"name\":\"web_search\",\"input\":{}}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"query\\\":\\\"current release\\\"}\"}}\n\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"web_search_tool_result\",\"tool_use_id\":\"srv_1\",\"content\":[{\"type\":\"web_search_result\",\"url\":\"https://example.com/release\",\"title\":\"Release\"}]}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"text_delta\",\"text\":\"Found it.\"}}\n\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        ));
        assert!(output.contains("response.output_item.added"), "{output}");
        assert!(output.contains("response.output_item.done"), "{output}");
        assert!(output.contains("\"type\":\"web_search_call\""), "{output}");
        assert!(output.contains("current release"), "{output}");
        assert!(output.contains("https://example.com/release"), "{output}");
        assert!(output.contains("Found it."), "{output}");
    }

    #[test]
    fn native_anthropic_stream_emits_live_reasoning_events() {
        let output = render(concat!(
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_reason\",\"model\":\"claude-sonnet-4-6\"}}\n\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"considering\"}}\n\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        ));

        let delta = output
            .find("response.reasoning_summary_text.delta")
            .unwrap();
        let completed = output.find("response.completed").unwrap();
        assert!(delta < completed, "{output}");
        assert!(
            output.contains("response.reasoning_summary_text.done"),
            "{output}"
        );
    }
}

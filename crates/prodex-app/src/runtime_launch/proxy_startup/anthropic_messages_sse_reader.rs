use super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
use super::deepseek_sse::RuntimeDeepSeekSseState;
use super::provider_bridge::RuntimeProviderBridgeKind;
use super::provider_sse_reader::{RuntimeProviderSseJsonReader, RuntimeProviderSseJsonState};
use serde_json::{Value, json};
use std::io::{self, Read};

pub(super) struct RuntimeAnthropicMessagesSseReader<R: Read> {
    inner: RuntimeProviderSseJsonReader<R, RuntimeAnthropicMessagesSseState>,
}

impl<R: Read> RuntimeAnthropicMessagesSseReader<R> {
    pub(super) fn new(
        reader: R,
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
                        RuntimeProviderBridgeKind::Anthropic,
                        request_id,
                        conversation_messages,
                        response_metadata,
                        conversations,
                    ),
                    input_tokens: 0,
                    output_tokens: 0,
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
                    Some(_) => Vec::new(),
                    None => {
                        self.failed("invalid_anthropic_stream", "content block type is missing")
                    }
                }
            }
            Some("content_block_delta") => {
                let index = value.get("index").and_then(Value::as_u64).unwrap_or(0);
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
                self.inner.complete_event().into_iter().collect()
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

use super::*;

mod content;
mod events;
mod parser;
mod reader;
mod terminal;
mod tool_use;

#[derive(Debug, Clone, Default)]
pub struct RuntimeAnthropicStreamToolUse {
    call_id: String,
    name: String,
    arguments: String,
    saw_delta: bool,
    server_tool_name: Option<String>,
    server_tool_block_type: Option<String>,
}

pub struct RuntimeAnthropicSseReader {
    inner: Box<dyn Read + Send>,
    pending: VecDeque<u8>,
    upstream_line: Vec<u8>,
    upstream_data_lines: Vec<String>,
    message_id: String,
    model: String,
    want_thinking: bool,
    content_index: usize,
    thinking_block_open: bool,
    text_block_open: bool,
    has_tool_calls: bool,
    has_content: bool,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: Option<u64>,
    web_search_requests: u64,
    web_fetch_requests: u64,
    code_execution_requests: u64,
    tool_search_requests: u64,
    server_tools: RuntimeAnthropicServerTools,
    active_tool_use: Option<RuntimeAnthropicStreamToolUse>,
    terminal_sent: bool,
    inner_finished: bool,
}

impl RuntimeAnthropicSseReader {
    pub fn new(
        inner: Box<dyn Read + Send>,
        model: String,
        want_thinking: bool,
        carried_usage: RuntimeAnthropicServerToolUsage,
        server_tools: RuntimeAnthropicServerTools,
    ) -> Self {
        let mut reader = Self {
            inner,
            pending: VecDeque::new(),
            upstream_line: Vec::new(),
            upstream_data_lines: Vec::new(),
            message_id: runtime_anthropic_message_id(),
            model,
            want_thinking,
            content_index: 0,
            thinking_block_open: false,
            text_block_open: false,
            has_tool_calls: false,
            has_content: false,
            input_tokens: 0,
            output_tokens: 0,
            cached_tokens: None,
            web_search_requests: carried_usage.web_search_requests,
            web_fetch_requests: carried_usage.web_fetch_requests,
            code_execution_requests: carried_usage.code_execution_requests,
            tool_search_requests: carried_usage.tool_search_requests,
            server_tools,
            active_tool_use: None,
            terminal_sent: false,
            inner_finished: false,
        };
        reader.push_event(
            "message_start",
            serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": reader.message_id.clone(),
                    "type": "message",
                    "role": "assistant",
                    "content": [],
                    "model": reader.model.clone(),
                    "stop_reason": serde_json::Value::Null,
                    "stop_sequence": serde_json::Value::Null,
                    "usage": {
                        "input_tokens": 0,
                        "output_tokens": 0,
                        "server_tool_use": {
                            "web_search_requests": carried_usage.web_search_requests,
                            "web_fetch_requests": carried_usage.web_fetch_requests,
                            "code_execution_requests": carried_usage.code_execution_requests,
                            "tool_search_requests": carried_usage.tool_search_requests,
                        }
                    }
                }
            }),
        );
        reader
    }
}

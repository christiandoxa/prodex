use super::*;

impl RuntimeAnthropicSseReader {
    pub(super) fn finish_success(&mut self) {
        if self.terminal_sent {
            return;
        }
        self.finish_active_tool_use(None, None, None);
        self.close_thinking_block();
        self.close_text_block();
        if !self.has_content {
            self.ensure_text_block();
            self.close_text_block();
        }
        let usage = runtime_anthropic_usage_json(
            self.input_tokens,
            self.output_tokens,
            self.cached_tokens,
            self.web_search_requests,
            self.web_fetch_requests,
            self.code_execution_requests,
            self.tool_search_requests,
        );
        self.push_event(
            "message_delta",
            serde_json::json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": if self.has_tool_calls { "tool_use" } else { "end_turn" },
                },
                "usage": usage,
            }),
        );
        self.push_event(
            "message_stop",
            serde_json::json!({
                "type": "message_stop",
            }),
        );
        self.terminal_sent = true;
        self.inner_finished = true;
    }

    pub(super) fn finish_error(&mut self, message: &str) {
        if self.terminal_sent {
            return;
        }
        self.finish_active_tool_use(None, None, None);
        self.close_thinking_block();
        self.close_text_block();
        self.ensure_text_block();
        self.push_event(
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": self.content_index,
                "delta": {
                    "type": "text_delta",
                    "text": format!("[Error] {message}"),
                }
            }),
        );
        self.has_content = true;
        self.close_text_block();
        self.push_event(
            "error",
            serde_json::json!({
                "type": "error",
                "error": {
                    "type": "api_error",
                    "message": message,
                }
            }),
        );
        self.push_event(
            "message_stop",
            serde_json::json!({
                "type": "message_stop",
            }),
        );
        self.terminal_sent = true;
        self.inner_finished = true;
    }
}

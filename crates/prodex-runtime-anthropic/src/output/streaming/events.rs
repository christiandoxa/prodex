use super::*;

impl RuntimeAnthropicSseReader {
    pub(super) fn observe_upstream_event(&mut self, value: &serde_json::Value) {
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("response.reasoning_summary_text.delta") if self.want_thinking => {
                self.observe_stream_reasoning_delta(value);
            }
            Some("response.output_text.delta") => {
                self.observe_stream_text_delta(value);
            }
            Some("response.output_item.added") => {
                self.observe_stream_output_item_added(value);
            }
            Some("response.function_call_arguments.delta") => {
                self.observe_stream_function_call_arguments_delta(value);
            }
            Some("response.function_call_arguments.done") => {
                self.observe_stream_function_call_arguments_done(value);
            }
            Some("response.output_item.done") => {
                self.observe_stream_output_item_done(value);
            }
            Some("response.completed") => {
                self.observe_stream_completed(value);
            }
            Some("error" | "response.failed") => {
                self.finish_error(runtime_anthropic_response_event_error_message(value));
            }
            _ => {}
        }
    }

    fn observe_stream_reasoning_delta(&mut self, value: &serde_json::Value) {
        self.close_text_block();
        self.ensure_thinking_block();
        if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
            self.push_event(
                "content_block_delta",
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": self.content_index,
                    "delta": {
                        "type": "thinking_delta",
                        "thinking": delta,
                    }
                }),
            );
            self.has_content = true;
        }
    }

    fn observe_stream_text_delta(&mut self, value: &serde_json::Value) {
        self.close_thinking_block();
        self.ensure_text_block();
        if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
            self.push_event(
                "content_block_delta",
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": self.content_index,
                    "delta": {
                        "type": "text_delta",
                        "text": delta,
                    }
                }),
            );
            self.has_content = true;
        }
    }

    fn observe_stream_output_item_added(&mut self, value: &serde_json::Value) {
        let Some(item) = runtime_anthropic_response_event_item(value) else {
            return;
        };
        match runtime_anthropic_output_item_type(item) {
            Some("function_call") => {
                self.start_tool_use_block(
                    runtime_anthropic_output_item_call_id(item, "tool_call"),
                    runtime_anthropic_output_item_name(item, "tool"),
                );
            }
            Some("shell_call") => {
                self.start_tool_use_block(
                    runtime_anthropic_output_item_call_id(item, "shell_call"),
                    "bash",
                );
            }
            Some("computer_call") => {
                self.start_tool_use_block(
                    runtime_anthropic_output_item_call_id(item, "computer_call"),
                    "computer",
                );
            }
            _ => {}
        }
    }

    fn observe_stream_function_call_arguments_delta(&mut self, value: &serde_json::Value) {
        let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) else {
            return;
        };
        if let Some(active_tool_use) = self.active_tool_use.as_mut() {
            active_tool_use.saw_delta = true;
            active_tool_use.arguments.push_str(delta);
        }
        self.push_event(
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": self.content_index,
                "delta": {
                    "type": "input_json_delta",
                    "partial_json": delta,
                }
            }),
        );
    }

    fn observe_stream_function_call_arguments_done(&mut self, value: &serde_json::Value) {
        if let Some(active_tool_use) = self.active_tool_use.as_mut()
            && let Some(arguments) = value.get("arguments").and_then(serde_json::Value::as_str)
            && !active_tool_use.saw_delta
        {
            active_tool_use.arguments = arguments.to_string();
        }
    }

    fn observe_stream_output_item_done(&mut self, value: &serde_json::Value) {
        let Some(item) = runtime_anthropic_response_event_item(value) else {
            return;
        };
        match runtime_anthropic_output_item_type(item) {
            Some("function_call") => self.observe_stream_function_call_item_done(item),
            Some("web_search_call") => self.observe_stream_web_search_item_done(item),
            Some("mcp_call") => self.emit_mcp_call_blocks(item),
            Some("shell_call") => self.observe_stream_shell_call_item_done(item),
            Some("computer_call") => self.observe_stream_computer_call_item_done(item),
            Some("mcp_approval_request") => self.emit_completed_content_block(
                runtime_anthropic_mcp_approval_request_block_from_output_item(item),
                true,
            ),
            Some("mcp_list_tools") => self.emit_completed_content_block(
                runtime_anthropic_mcp_list_tools_block_from_output_item(item),
                false,
            ),
            _ => {}
        }
    }

    fn observe_stream_function_call_item_done(&mut self, item: &serde_json::Value) {
        self.add_stream_output_item_server_tool_usage(item);
        if self.active_tool_use.is_none() {
            self.start_tool_use_block(
                runtime_anthropic_output_item_call_id(item, "tool_call"),
                runtime_anthropic_output_item_name(item, "tool"),
            );
        }
        self.finish_active_tool_use(
            item.get("arguments").and_then(serde_json::Value::as_str),
            item.get("name").and_then(serde_json::Value::as_str),
            item.get("call_id").and_then(serde_json::Value::as_str),
        );
    }

    fn observe_stream_web_search_item_done(&mut self, item: &serde_json::Value) {
        self.web_search_requests +=
            runtime_anthropic_web_search_request_count_from_output_item(item);
    }

    fn observe_stream_shell_call_item_done(&mut self, item: &serde_json::Value) {
        if self.active_tool_use.is_none() {
            self.start_tool_use_block(
                runtime_anthropic_output_item_call_id(item, "shell_call"),
                "bash",
            );
        }
        let arguments =
            serde_json::to_string(&runtime_anthropic_shell_tool_input_from_output_item(item)).ok();
        self.finish_active_tool_use(
            arguments.as_deref(),
            Some("bash"),
            item.get("call_id").and_then(serde_json::Value::as_str),
        );
    }

    fn observe_stream_computer_call_item_done(&mut self, item: &serde_json::Value) {
        if self.active_tool_use.is_none() {
            self.start_tool_use_block(
                runtime_anthropic_output_item_call_id(item, "computer_call"),
                "computer",
            );
        }
        let input = runtime_anthropic_computer_tool_input_from_output_item(item)
            .unwrap_or_else(|| runtime_anthropic_raw_computer_tool_input_from_output_item(item));
        let arguments = serde_json::to_string(&input).ok();
        self.finish_active_tool_use(
            arguments.as_deref(),
            Some("computer"),
            item.get("call_id").and_then(serde_json::Value::as_str),
        );
    }

    fn observe_stream_completed(&mut self, value: &serde_json::Value) {
        let (input_tokens, output_tokens, cached_tokens) =
            runtime_anthropic_usage_from_value(value);
        self.input_tokens = input_tokens;
        self.output_tokens = output_tokens;
        self.cached_tokens = cached_tokens;
        self.web_search_requests = self.web_search_requests.max(
            runtime_anthropic_tool_usage_web_search_requests_from_value(value),
        );
        self.code_execution_requests = self
            .code_execution_requests
            .max(runtime_anthropic_tool_usage_code_execution_requests_from_value(value));
        self.tool_search_requests = self
            .tool_search_requests
            .max(runtime_anthropic_tool_usage_tool_search_requests_from_value(value));
        if let Some(output) = value
            .get("response")
            .and_then(|response| response.get("output"))
            .and_then(serde_json::Value::as_array)
        {
            self.merge_stream_output_usage(output);
        }
        self.finish_success();
    }

    fn add_stream_output_item_server_tool_usage(&mut self, item: &serde_json::Value) {
        let usage = runtime_anthropic_output_item_server_tool_usage(item, Some(&self.server_tools));
        self.web_search_requests += usage.web_search_requests;
        self.web_fetch_requests += usage.web_fetch_requests;
        self.code_execution_requests += usage.code_execution_requests;
        self.tool_search_requests += usage.tool_search_requests;
    }

    fn merge_stream_output_usage(&mut self, output: &[serde_json::Value]) {
        self.web_search_requests =
            self.web_search_requests
                .max(runtime_anthropic_web_search_request_count_from_output(
                    output,
                    Some(&self.server_tools),
                ));
        self.web_fetch_requests =
            self.web_fetch_requests
                .max(runtime_anthropic_web_fetch_request_count_from_output(
                    output,
                    Some(&self.server_tools),
                ));
        self.code_execution_requests = self.code_execution_requests.max(
            runtime_anthropic_code_execution_request_count_from_output(
                output,
                Some(&self.server_tools),
            ),
        );
        self.tool_search_requests =
            self.tool_search_requests
                .max(runtime_anthropic_tool_search_request_count_from_output(
                    output,
                    Some(&self.server_tools),
                ));
    }
}

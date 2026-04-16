use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeAnthropicStreamToolUse {
    call_id: String,
    name: String,
    arguments: String,
    saw_delta: bool,
    server_tool_name: Option<String>,
    server_tool_block_type: Option<String>,
}

pub(crate) struct RuntimeAnthropicSseReader {
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        inner: Box<dyn Read + Send>,
        model: String,
        want_thinking: bool,
        carried_web_search_requests: u64,
        carried_web_fetch_requests: u64,
        carried_code_execution_requests: u64,
        carried_tool_search_requests: u64,
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
            web_search_requests: carried_web_search_requests,
            web_fetch_requests: carried_web_fetch_requests,
            code_execution_requests: carried_code_execution_requests,
            tool_search_requests: carried_tool_search_requests,
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
                            "web_search_requests": carried_web_search_requests,
                            "web_fetch_requests": carried_web_fetch_requests,
                            "code_execution_requests": carried_code_execution_requests,
                            "tool_search_requests": carried_tool_search_requests,
                        }
                    }
                }
            }),
        );
        reader
    }

    fn push_event(&mut self, event_type: &str, data: serde_json::Value) {
        let frame = format!(
            "event: {event_type}\ndata: {}\n\n",
            serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
        );
        self.pending.extend(frame.into_bytes());
    }

    fn close_thinking_block(&mut self) {
        if !self.thinking_block_open {
            return;
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.thinking_block_open = false;
    }

    fn close_text_block(&mut self) {
        if !self.text_block_open {
            return;
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.text_block_open = false;
    }

    fn ensure_text_block(&mut self) {
        if self.text_block_open {
            return;
        }
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": {
                    "type": "text",
                    "text": "",
                }
            }),
        );
        self.text_block_open = true;
    }

    fn ensure_thinking_block(&mut self) {
        if self.thinking_block_open {
            return;
        }
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": {
                    "type": "thinking",
                    "thinking": "",
                }
            }),
        );
        self.thinking_block_open = true;
    }

    fn start_tool_use_block(&mut self, call_id: &str, name: &str) {
        let server_tool_registration =
            runtime_anthropic_server_tool_registration_for_call(name, Some(&self.server_tools));
        let block_type = server_tool_registration
            .as_ref()
            .map(|(_, block_type)| block_type.as_str())
            .unwrap_or("tool_use");
        let output_name = server_tool_registration
            .as_ref()
            .map(|(server_tool_name, _)| server_tool_name.as_str())
            .unwrap_or(name);
        let mut content_block = serde_json::Map::new();
        content_block.insert(
            "type".to_string(),
            serde_json::Value::String(block_type.to_string()),
        );
        content_block.insert(
            "id".to_string(),
            serde_json::Value::String(call_id.to_string()),
        );
        content_block.insert(
            "name".to_string(),
            serde_json::Value::String(output_name.to_string()),
        );
        content_block.insert("input".to_string(), serde_json::json!({}));
        self.close_thinking_block();
        self.close_text_block();
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": serde_json::Value::Object(content_block)
            }),
        );
        self.active_tool_use = Some(RuntimeAnthropicStreamToolUse {
            call_id: call_id.to_string(),
            name: name.to_string(),
            server_tool_name: server_tool_registration
                .as_ref()
                .map(|(server_tool_name, _)| server_tool_name.clone()),
            server_tool_block_type: server_tool_registration.map(|(_, block_type)| block_type),
            ..RuntimeAnthropicStreamToolUse::default()
        });
        self.has_content = true;
        self.has_tool_calls = true;
    }

    fn finish_active_tool_use(
        &mut self,
        arguments_override: Option<&str>,
        name_override: Option<&str>,
        call_id_override: Option<&str>,
    ) {
        let Some(mut active_tool_use) = self.active_tool_use.take() else {
            return;
        };
        if let Some(name) = name_override {
            active_tool_use.name = name.to_string();
            if let Some((server_tool_name, block_type)) =
                runtime_anthropic_server_tool_registration_for_call(name, Some(&self.server_tools))
            {
                active_tool_use.server_tool_name = Some(server_tool_name);
                active_tool_use.server_tool_block_type = Some(block_type);
            } else {
                active_tool_use.server_tool_name = None;
                active_tool_use.server_tool_block_type = None;
            }
        }
        if let Some(call_id) = call_id_override {
            active_tool_use.call_id = call_id.to_string();
        }
        if let Some(arguments) = arguments_override
            && !active_tool_use.saw_delta
        {
            active_tool_use.arguments = arguments.to_string();
        }
        if !active_tool_use.saw_delta && !active_tool_use.arguments.is_empty() {
            self.push_event(
                "content_block_delta",
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": self.content_index,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": active_tool_use.arguments,
                    }
                }),
            );
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
    }

    fn emit_completed_content_block(&mut self, block: serde_json::Value, has_tool_calls: bool) {
        let Some(block_type) = block.get("type").and_then(serde_json::Value::as_str) else {
            return;
        };

        self.finish_active_tool_use(None, None, None);
        self.close_thinking_block();
        self.close_text_block();

        match block_type {
            "mcp_tool_use" => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                let mut content_block = serde_json::Map::new();
                content_block.insert(
                    "type".to_string(),
                    serde_json::Value::String("mcp_tool_use".to_string()),
                );
                content_block.insert(
                    "id".to_string(),
                    block
                        .get("id")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String("mcp_tool_use".to_string())),
                );
                content_block.insert(
                    "name".to_string(),
                    block
                        .get("name")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String("mcp_tool".to_string())),
                );
                content_block.insert(
                    "server_name".to_string(),
                    block
                        .get("server_name")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String("mcp".to_string())),
                );
                content_block.insert("input".to_string(), serde_json::json!({}));
                self.push_event(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": self.content_index,
                        "content_block": serde_json::Value::Object(content_block),
                    }),
                );
                self.push_event(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": self.content_index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                );
            }
            block_type if block_type.ends_with("_tool_result") => {
                let mut content_block = serde_json::Map::new();
                content_block.insert(
                    "type".to_string(),
                    serde_json::Value::String(block_type.to_string()),
                );
                content_block.insert(
                    "tool_use_id".to_string(),
                    block
                        .get("tool_use_id")
                        .cloned()
                        .unwrap_or_else(|| serde_json::Value::String(format!("{block_type}_call"))),
                );
                content_block.insert(
                    "content".to_string(),
                    block
                        .get("content")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                );
                if let Some(is_error) = block.get("is_error").cloned() {
                    content_block.insert("is_error".to_string(), is_error);
                }
                self.push_event(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": self.content_index,
                        "content_block": serde_json::Value::Object(content_block),
                    }),
                );
            }
            "mcp_approval_request" | "mcp_list_tools" => {
                self.push_event(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": self.content_index,
                        "content_block": block,
                    }),
                );
            }
            _ => return,
        }

        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.has_content = true;
        self.has_tool_calls |= has_tool_calls;
    }

    fn emit_mcp_call_blocks(&mut self, item: &serde_json::Value) {
        for block in runtime_anthropic_mcp_call_blocks_from_output_item(item) {
            self.emit_completed_content_block(block, false);
        }
    }

    fn finish_success(&mut self) {
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

    fn finish_error(&mut self, message: &str) {
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

    fn observe_upstream_event(&mut self, value: &serde_json::Value) {
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

    fn process_upstream_event(&mut self) -> io::Result<()> {
        if self.upstream_data_lines.is_empty() {
            return Ok(());
        }
        let payload = self.upstream_data_lines.join("\n");
        self.upstream_data_lines.clear();
        let value = serde_json::from_str::<serde_json::Value>(&payload).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse runtime Responses SSE payload: {err}"),
            )
        })?;
        self.observe_upstream_event(&value);
        Ok(())
    }

    fn observe_upstream_bytes(&mut self, chunk: &[u8]) -> io::Result<()> {
        for byte in chunk {
            self.upstream_line.push(*byte);
            if *byte != b'\n' {
                continue;
            }
            let line_text = String::from_utf8_lossy(&self.upstream_line);
            let trimmed = line_text.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                self.process_upstream_event()?;
                self.upstream_line.clear();
                if self.inner_finished {
                    break;
                }
                continue;
            }
            if let Some(payload) = trimmed.strip_prefix("data:") {
                self.upstream_data_lines
                    .push(payload.trim_start().to_string());
            }
            self.upstream_line.clear();
        }
        Ok(())
    }
}

impl Read for RuntimeAnthropicSseReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let read = buf.len().min(self.pending.len());
            if read > 0 {
                for (index, byte) in self.pending.drain(..read).enumerate() {
                    buf[index] = byte;
                }
                return Ok(read);
            }
            if self.inner_finished {
                return Ok(0);
            }

            let mut upstream_buffer = [0_u8; 8192];
            let read = self.inner.read(&mut upstream_buffer)?;
            if read == 0 {
                self.finish_success();
                continue;
            }
            self.observe_upstream_bytes(&upstream_buffer[..read])?;
        }
    }
}

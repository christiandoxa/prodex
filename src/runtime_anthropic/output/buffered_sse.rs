use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeAnthropicCollectedToolUse {
    call_id: String,
    name: String,
    arguments: String,
    saw_delta: bool,
    server_tool_name: Option<String>,
    server_tool_block_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeAnthropicCollectedResponse {
    content: Vec<serde_json::Value>,
    pending_text: String,
    pending_thinking: String,
    active_tool_use: Option<RuntimeAnthropicCollectedToolUse>,
    final_output: Option<Vec<serde_json::Value>>,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: Option<u64>,
    web_search_requests: u64,
    web_fetch_requests: u64,
    code_execution_requests: u64,
    tool_search_requests: u64,
    has_tool_calls: bool,
    want_thinking: bool,
    server_tools: RuntimeAnthropicServerTools,
}

pub(crate) fn runtime_anthropic_response_event_item(
    value: &serde_json::Value,
) -> Option<&serde_json::Value> {
    value.get("item")
}

pub(crate) fn runtime_anthropic_output_item_type(item: &serde_json::Value) -> Option<&str> {
    item.get("type").and_then(serde_json::Value::as_str)
}

pub(crate) fn runtime_anthropic_output_item_call_id<'a>(
    item: &'a serde_json::Value,
    default: &'static str,
) -> &'a str {
    item.get("call_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default)
}

pub(crate) fn runtime_anthropic_output_item_name<'a>(
    item: &'a serde_json::Value,
    default: &'static str,
) -> &'a str {
    item.get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default)
}

pub(crate) fn runtime_anthropic_response_event_error_message(value: &serde_json::Value) -> &str {
    value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Codex returned an error.")
}

impl RuntimeAnthropicCollectedResponse {
    fn flush_text(&mut self) {
        if self.pending_text.is_empty() {
            return;
        }
        self.content.push(serde_json::json!({
            "type": "text",
            "text": std::mem::take(&mut self.pending_text),
        }));
    }

    fn flush_thinking(&mut self) {
        if self.pending_thinking.is_empty() {
            return;
        }
        self.content.push(serde_json::json!({
            "type": "thinking",
            "thinking": std::mem::take(&mut self.pending_thinking),
        }));
    }

    fn flush_pending_textual_content(&mut self) {
        self.flush_thinking();
        self.flush_text();
    }

    fn close_active_tool_use(&mut self) {
        let Some(active_tool_use) = self.active_tool_use.take() else {
            return;
        };
        self.has_tool_calls = true;
        let input = runtime_anthropic_tool_input_from_arguments(&active_tool_use.arguments);
        self.content.push(
            active_tool_use
                .server_tool_name
                .as_deref()
                .and_then(|server_tool_name| {
                    runtime_anthropic_server_tool_use_block(
                        &active_tool_use.call_id,
                        server_tool_name,
                        input.clone(),
                        Some(&self.server_tools),
                    )
                })
                .unwrap_or_else(|| {
                    serde_json::json!({
                        "type": "tool_use",
                        "id": active_tool_use.call_id,
                        "name": active_tool_use.name,
                        "input": input,
                    })
                }),
        );
    }

    fn observe_event(&mut self, value: &serde_json::Value) -> Result<()> {
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("response.reasoning_summary_text.delta") if self.want_thinking => {
                self.observe_reasoning_delta(value);
            }
            Some("response.output_text.delta") => {
                self.observe_text_delta(value);
            }
            Some("response.output_item.added") => {
                self.observe_output_item_added(value);
            }
            Some("response.function_call_arguments.delta") => {
                self.observe_function_call_arguments_delta(value);
            }
            Some("response.function_call_arguments.done") => {
                self.observe_function_call_arguments_done(value);
            }
            Some("response.output_item.done") => {
                self.observe_output_item_done(value);
            }
            Some("response.completed") => {
                self.observe_completed(value);
            }
            Some("error" | "response.failed") => {
                bail!(runtime_anthropic_response_event_error_message(value).to_string());
            }
            _ => {}
        }
        Ok(())
    }

    fn observe_reasoning_delta(&mut self, value: &serde_json::Value) {
        self.flush_text();
        if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
            self.pending_thinking.push_str(delta);
        }
    }

    fn observe_text_delta(&mut self, value: &serde_json::Value) {
        self.flush_thinking();
        if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
            self.pending_text.push_str(delta);
        }
    }

    fn observe_output_item_added(&mut self, value: &serde_json::Value) {
        let Some(item) = runtime_anthropic_response_event_item(value) else {
            return;
        };
        if runtime_anthropic_output_item_type(item) != Some("function_call") {
            return;
        }
        self.flush_pending_textual_content();
        self.active_tool_use = Some(self.collected_tool_use_from_item(item));
    }

    fn collected_tool_use_from_item(
        &self,
        item: &serde_json::Value,
    ) -> RuntimeAnthropicCollectedToolUse {
        let name = runtime_anthropic_output_item_name(item, "tool");
        let server_tool_registration =
            runtime_anthropic_server_tool_registration_for_call(name, Some(&self.server_tools));
        RuntimeAnthropicCollectedToolUse {
            call_id: runtime_anthropic_output_item_call_id(item, "tool_call").to_string(),
            name: name.to_string(),
            server_tool_name: server_tool_registration
                .as_ref()
                .map(|(server_tool_name, _)| server_tool_name.clone()),
            server_tool_block_type: server_tool_registration.map(|(_, block_type)| block_type),
            ..RuntimeAnthropicCollectedToolUse::default()
        }
    }

    fn observe_function_call_arguments_delta(&mut self, value: &serde_json::Value) {
        if let Some(active_tool_use) = self.active_tool_use.as_mut()
            && let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str)
        {
            active_tool_use.saw_delta = true;
            active_tool_use.arguments.push_str(delta);
        }
    }

    fn observe_function_call_arguments_done(&mut self, value: &serde_json::Value) {
        if let Some(active_tool_use) = self.active_tool_use.as_mut()
            && let Some(arguments) = value.get("arguments").and_then(serde_json::Value::as_str)
            && !active_tool_use.saw_delta
        {
            active_tool_use.arguments = arguments.to_string();
        }
    }

    fn observe_output_item_done(&mut self, value: &serde_json::Value) {
        let Some(item) = runtime_anthropic_response_event_item(value) else {
            return;
        };
        match runtime_anthropic_output_item_type(item) {
            Some("function_call") => self.observe_function_call_item_done(item),
            Some("web_search_call") => self.observe_web_search_item_done(item),
            Some("shell_call") => self.observe_shell_call_item_done(item),
            Some("computer_call") => self.observe_computer_call_item_done(item),
            _ => {}
        }
    }

    fn observe_function_call_item_done(&mut self, item: &serde_json::Value) {
        self.add_output_item_server_tool_usage(item);
        if let Some(active_tool_use) = self.active_tool_use.as_mut() {
            if let Some(arguments) = item.get("arguments").and_then(serde_json::Value::as_str)
                && !active_tool_use.saw_delta
            {
                active_tool_use.arguments = arguments.to_string();
            }
            if let Some(name) = item.get("name").and_then(serde_json::Value::as_str) {
                active_tool_use.name = name.to_string();
                Self::set_collected_tool_server_registration(
                    active_tool_use,
                    name,
                    &self.server_tools,
                );
            }
        }
        self.close_active_tool_use();
    }

    fn set_collected_tool_server_registration(
        active_tool_use: &mut RuntimeAnthropicCollectedToolUse,
        name: &str,
        server_tools: &RuntimeAnthropicServerTools,
    ) {
        if let Some((server_tool_name, block_type)) =
            runtime_anthropic_server_tool_registration_for_call(name, Some(server_tools))
        {
            active_tool_use.server_tool_name = Some(server_tool_name);
            active_tool_use.server_tool_block_type = Some(block_type);
        } else {
            active_tool_use.server_tool_name = None;
            active_tool_use.server_tool_block_type = None;
        }
    }

    fn observe_web_search_item_done(&mut self, item: &serde_json::Value) {
        self.flush_pending_textual_content();
        self.web_search_requests +=
            runtime_anthropic_web_search_request_count_from_output_item(item);
        self.content
            .extend(runtime_anthropic_web_search_blocks_from_output_item(
                item,
                &BTreeMap::new(),
            ));
    }

    fn observe_shell_call_item_done(&mut self, item: &serde_json::Value) {
        self.flush_pending_textual_content();
        self.has_tool_calls = true;
        self.content
            .push(runtime_anthropic_shell_tool_use_block_from_output_item(
                item,
            ));
    }

    fn observe_computer_call_item_done(&mut self, item: &serde_json::Value) {
        self.flush_pending_textual_content();
        self.has_tool_calls = true;
        self.content
            .push(runtime_anthropic_computer_tool_use_block_from_output_item(
                item,
            ));
    }

    fn observe_completed(&mut self, value: &serde_json::Value) {
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
        let final_output = value
            .get("response")
            .and_then(|response| response.get("output"))
            .and_then(serde_json::Value::as_array)
            .cloned();
        if let Some(output) = final_output.as_ref() {
            self.merge_output_usage(output);
        }
        self.final_output = final_output;
    }

    fn add_output_item_server_tool_usage(&mut self, item: &serde_json::Value) {
        let usage = runtime_anthropic_output_item_server_tool_usage(item, Some(&self.server_tools));
        self.web_search_requests += usage.web_search_requests;
        self.web_fetch_requests += usage.web_fetch_requests;
        self.code_execution_requests += usage.code_execution_requests;
        self.tool_search_requests += usage.tool_search_requests;
    }

    fn merge_output_usage(&mut self, output: &[serde_json::Value]) {
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

    fn into_response(mut self, requested_model: &str) -> serde_json::Value {
        if let Some(output) = self.final_output.take().filter(|output| !output.is_empty()) {
            let (content, has_tool_calls) = runtime_anthropic_output_blocks_from_json(
                &output,
                self.want_thinking,
                Some(&self.server_tools),
            );
            self.content = content;
            self.has_tool_calls = has_tool_calls;
        } else {
            self.close_active_tool_use();
            self.flush_pending_textual_content();
            if self.content.is_empty() {
                self.content.push(serde_json::json!({
                    "type": "text",
                    "text": "",
                }));
            }
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
        serde_json::json!({
            "id": runtime_anthropic_message_id(),
            "type": "message",
            "role": "assistant",
            "content": self.content,
            "model": requested_model,
            "stop_reason": if self.has_tool_calls { "tool_use" } else { "end_turn" },
            "stop_sequence": serde_json::Value::Null,
            "usage": usage,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_anthropic_response_from_sse_bytes(
    body: &[u8],
    requested_model: &str,
    want_thinking: bool,
) -> Result<serde_json::Value> {
    runtime_anthropic_response_from_sse_bytes_with_carried_usage(
        body,
        requested_model,
        want_thinking,
        0,
        0,
        0,
        0,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_anthropic_response_from_sse_bytes_with_carried_usage(
    body: &[u8],
    requested_model: &str,
    want_thinking: bool,
    carried_web_search_requests: u64,
    carried_web_fetch_requests: u64,
    carried_code_execution_requests: u64,
    carried_tool_search_requests: u64,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> Result<serde_json::Value> {
    let mut collected = RuntimeAnthropicCollectedResponse {
        want_thinking,
        web_search_requests: carried_web_search_requests,
        web_fetch_requests: carried_web_fetch_requests,
        code_execution_requests: carried_code_execution_requests,
        tool_search_requests: carried_tool_search_requests,
        server_tools: server_tools.cloned().unwrap_or_default(),
        ..RuntimeAnthropicCollectedResponse::default()
    };
    let mut line = Vec::new();
    let mut data_lines = Vec::new();

    let mut process_event = |data_lines: &mut Vec<String>| -> Result<()> {
        if data_lines.is_empty() {
            return Ok(());
        }
        let payload = data_lines.join("\n");
        let value = serde_json::from_str::<serde_json::Value>(&payload)
            .context("failed to parse buffered Responses SSE payload")?;
        collected.observe_event(&value)?;
        data_lines.clear();
        Ok(())
    };

    for byte in body {
        line.push(*byte);
        if *byte != b'\n' {
            continue;
        }
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            process_event(&mut data_lines)?;
            line.clear();
            continue;
        }
        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
        line.clear();
    }
    if !line.is_empty() {
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
    }
    process_event(&mut data_lines)?;
    Ok(collected.into_response(requested_model))
}

use super::*;

mod buffered_sse;
mod errors;
mod response_parts;
mod streaming;
mod tool_blocks;
mod usage;

pub(crate) use buffered_sse::*;
pub(crate) use errors::*;
pub(crate) use response_parts::*;
pub(crate) use streaming::*;
pub(crate) use tool_blocks::*;
pub(crate) use usage::*;

struct RuntimeAnthropicOutputBlockTranslator<'a> {
    content: Vec<serde_json::Value>,
    has_tool_calls: bool,
    want_thinking: bool,
    server_tools: Option<&'a RuntimeAnthropicServerTools>,
    annotation_titles_by_url: BTreeMap<String, String>,
}

impl<'a> RuntimeAnthropicOutputBlockTranslator<'a> {
    fn new(
        output: &[serde_json::Value],
        want_thinking: bool,
        server_tools: Option<&'a RuntimeAnthropicServerTools>,
    ) -> Self {
        Self {
            content: Vec::new(),
            has_tool_calls: false,
            want_thinking,
            server_tools,
            annotation_titles_by_url: runtime_anthropic_message_annotation_titles_by_url(output),
        }
    }

    fn translate(mut self, output: &[serde_json::Value]) -> (Vec<serde_json::Value>, bool) {
        for item in output {
            self.push_item(item);
        }
        if self.content.is_empty() {
            self.content.push(serde_json::json!({
                "type": "text",
                "text": "",
            }));
        }
        (self.content, self.has_tool_calls)
    }

    fn push_item(&mut self, item: &serde_json::Value) {
        match item.get("type").and_then(serde_json::Value::as_str) {
            Some("reasoning") if self.want_thinking => {
                self.push_reasoning(item);
            }
            Some("message") => {
                self.push_message_text(item);
            }
            Some("web_search_call") => {
                self.content
                    .extend(runtime_anthropic_web_search_blocks_from_output_item(
                        item,
                        &self.annotation_titles_by_url,
                    ));
            }
            Some("mcp_call") => {
                self.content
                    .extend(runtime_anthropic_mcp_call_blocks_from_output_item(item));
            }
            Some("mcp_approval_request") => {
                self.has_tool_calls = true;
                self.content
                    .push(runtime_anthropic_mcp_approval_request_block_from_output_item(item));
            }
            Some("mcp_list_tools") => {
                self.content
                    .push(runtime_anthropic_mcp_list_tools_block_from_output_item(
                        item,
                    ));
            }
            Some("shell_call") => {
                self.has_tool_calls = true;
                self.content
                    .push(runtime_anthropic_shell_tool_use_block_from_output_item(
                        item,
                    ));
            }
            Some("computer_call") => {
                self.has_tool_calls = true;
                self.content
                    .push(runtime_anthropic_computer_tool_use_block_from_output_item(
                        item,
                    ));
            }
            Some("function_call") => {
                self.push_function_call(item);
            }
            _ => {}
        }
    }

    fn push_reasoning(&mut self, item: &serde_json::Value) {
        if !self.want_thinking {
            return;
        }
        let thinking = runtime_anthropic_reasoning_summary_text(item);
        if !thinking.is_empty() {
            self.content.push(serde_json::json!({
                "type": "thinking",
                "thinking": thinking,
            }));
        }
    }

    fn push_message_text(&mut self, item: &serde_json::Value) {
        let Some(parts) = item.get("content").and_then(serde_json::Value::as_array) else {
            return;
        };
        let mut text = String::new();
        for part in parts {
            if part
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|part_type| matches!(part_type, "output_text" | "text"))
                && let Some(part_text) = part.get("text").and_then(serde_json::Value::as_str)
            {
                text.push_str(part_text);
            }
        }
        if !text.is_empty() {
            self.content.push(serde_json::json!({
                "type": "text",
                "text": text,
            }));
        }
    }

    fn push_function_call(&mut self, item: &serde_json::Value) {
        self.has_tool_calls = true;
        let call_id = item
            .get("call_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("tool_call");
        let name = item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("tool");
        let input = runtime_anthropic_tool_input_from_arguments(
            item.get("arguments")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("{}"),
        );
        self.content.push(
            runtime_anthropic_server_tool_use_block(
                call_id,
                name,
                input.clone(),
                self.server_tools,
            )
            .unwrap_or_else(|| {
                serde_json::json!({
                    "type": "tool_use",
                    "id": call_id,
                    "name": name,
                    "input": input,
                })
            }),
        );
    }
}

pub(crate) fn runtime_anthropic_output_blocks_from_json(
    output: &[serde_json::Value],
    want_thinking: bool,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> (Vec<serde_json::Value>, bool) {
    RuntimeAnthropicOutputBlockTranslator::new(output, want_thinking, server_tools)
        .translate(output)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_anthropic_response_from_json_value(
    value: &serde_json::Value,
    requested_model: &str,
    want_thinking: bool,
) -> serde_json::Value {
    runtime_anthropic_response_from_json_value_with_carried_usage(
        value,
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
pub(crate) fn runtime_anthropic_response_from_json_value_with_carried_usage(
    value: &serde_json::Value,
    requested_model: &str,
    want_thinking: bool,
    carried_web_search_requests: u64,
    carried_web_fetch_requests: u64,
    carried_code_execution_requests: u64,
    carried_tool_search_requests: u64,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> serde_json::Value {
    let (input_tokens, output_tokens, cached_tokens) = runtime_anthropic_usage_from_value(value);
    let output = value
        .get("output")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let web_search_requests = runtime_anthropic_tool_usage_web_search_requests_from_value(value)
        .max(runtime_anthropic_web_search_request_count_from_output(
            &output,
            server_tools,
        ))
        .max(carried_web_search_requests);
    let web_fetch_requests =
        runtime_anthropic_web_fetch_request_count_from_output(&output, server_tools)
            .max(carried_web_fetch_requests);
    let code_execution_requests =
        runtime_anthropic_tool_usage_code_execution_requests_from_value(value)
            .max(runtime_anthropic_code_execution_request_count_from_output(
                &output,
                server_tools,
            ))
            .max(carried_code_execution_requests);
    let tool_search_requests = runtime_anthropic_tool_usage_tool_search_requests_from_value(value)
        .max(runtime_anthropic_tool_search_request_count_from_output(
            &output,
            server_tools,
        ))
        .max(carried_tool_search_requests);
    let (content, has_tool_calls) =
        runtime_anthropic_output_blocks_from_json(&output, want_thinking, server_tools);
    let usage = runtime_anthropic_usage_json(
        input_tokens,
        output_tokens,
        cached_tokens,
        web_search_requests,
        web_fetch_requests,
        code_execution_requests,
        tool_search_requests,
    );
    serde_json::json!({
        "id": runtime_anthropic_message_id(),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": requested_model,
        "stop_reason": if has_tool_calls { "tool_use" } else { "end_turn" },
        "stop_sequence": serde_json::Value::Null,
        "usage": usage,
    })
}

pub(crate) fn translate_runtime_buffered_responses_reply_to_anthropic(
    parts: RuntimeBufferedResponseParts,
    request: &RuntimeAnthropicMessagesRequest,
) -> Result<RuntimeResponsesReply> {
    if parts.status >= 400 {
        return Ok(RuntimeResponsesReply::Buffered(
            runtime_anthropic_error_from_upstream_parts(parts),
        ));
    }

    let content_type = runtime_buffered_response_content_type(&parts)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let looks_like_sse = content_type.contains("text/event-stream")
        || runtime_response_body_looks_like_sse(&parts.body);
    if request.stream && looks_like_sse {
        return Ok(RuntimeResponsesReply::Buffered(
            runtime_anthropic_sse_response_parts_from_responses_sse_bytes(
                &parts.body,
                &request.requested_model,
                request.want_thinking,
                request.carried_web_search_requests,
                request.carried_web_fetch_requests,
                request.carried_code_execution_requests,
                request.carried_tool_search_requests,
                &request.server_tools,
            )?,
        ));
    }

    let response = if looks_like_sse {
        runtime_anthropic_response_from_sse_bytes_with_carried_usage(
            &parts.body,
            &request.requested_model,
            request.want_thinking,
            request.carried_web_search_requests,
            request.carried_web_fetch_requests,
            request.carried_code_execution_requests,
            request.carried_tool_search_requests,
            Some(&request.server_tools),
        )?
    } else {
        let value = serde_json::from_slice::<serde_json::Value>(&parts.body)
            .context("failed to parse buffered Responses JSON body")?;
        if value.get("error").is_some() {
            return Ok(RuntimeResponsesReply::Buffered(
                runtime_anthropic_error_from_upstream_parts(parts),
            ));
        }
        runtime_anthropic_response_from_json_value_with_carried_usage(
            &value,
            &request.requested_model,
            request.want_thinking,
            request.carried_web_search_requests,
            request.carried_web_fetch_requests,
            request.carried_code_execution_requests,
            request.carried_tool_search_requests,
            Some(&request.server_tools),
        )
    };

    if request.stream {
        return Ok(RuntimeResponsesReply::Buffered(
            runtime_anthropic_sse_response_parts_from_message_value(response),
        ));
    }

    Ok(RuntimeResponsesReply::Buffered(
        runtime_anthropic_json_response_parts(response),
    ))
}

pub(crate) fn translate_runtime_responses_reply_to_anthropic(
    response: RuntimeResponsesReply,
    request: &RuntimeAnthropicMessagesRequest,
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    if request.server_tools.needs_buffered_translation() {
        let mut parts = match response {
            RuntimeResponsesReply::Buffered(parts) => parts,
            RuntimeResponsesReply::Streaming(response) => {
                buffer_runtime_streaming_response_parts(response)?
            }
        };
        let mut carried_usage = RuntimeAnthropicServerToolUsage {
            web_search_requests: request.carried_web_search_requests,
            web_fetch_requests: request.carried_web_fetch_requests,
            code_execution_requests: request.carried_code_execution_requests,
            tool_search_requests: request.carried_tool_search_requests,
        };

        for followup_attempt in 0..=RUNTIME_PROXY_ANTHROPIC_WEB_SEARCH_FOLLOWUP_LIMIT {
            if std::env::var_os("PRODEX_DEBUG_ANTHROPIC_COMPAT").is_some() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http anthropic_translated_upstream status={} content_type={:?} followup_attempt={} body_snippet={}",
                        parts.status,
                        runtime_buffered_response_content_type(&parts),
                        followup_attempt,
                        runtime_proxy_body_snippet(&parts.body, 2048),
                    ),
                );
            }

            if parts.status >= 400 {
                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_error_from_upstream_parts(parts),
                ));
            }

            if !runtime_response_body_looks_like_sse(&parts.body)
                && !runtime_buffered_response_content_type(&parts)
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains("text/event-stream")
                && serde_json::from_slice::<serde_json::Value>(&parts.body)
                    .ok()
                    .is_some_and(|value| value.get("error").is_some())
            {
                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_error_from_upstream_parts(parts),
                ));
            }

            let response_message =
                runtime_anthropic_message_from_buffered_responses_parts_with_carried_usage(
                    &parts,
                    request,
                    carried_usage,
                )?;
            carried_usage = runtime_anthropic_message_server_tool_usage(&response_message);

            if followup_attempt == RUNTIME_PROXY_ANTHROPIC_WEB_SEARCH_FOLLOWUP_LIMIT
                || !runtime_anthropic_message_needs_server_tool_followup(&response_message)
            {
                if request.stream {
                    return Ok(RuntimeResponsesReply::Buffered(
                        runtime_anthropic_sse_response_parts_from_message_value(response_message),
                    ));
                }

                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_json_response_parts(response_message),
                ));
            }

            let Some(previous_response_id) = runtime_buffered_response_ids(&parts).last().cloned()
            else {
                if request.stream {
                    return Ok(RuntimeResponsesReply::Buffered(
                        runtime_anthropic_sse_response_parts_from_message_value(response_message),
                    ));
                }
                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_json_response_parts(response_message),
                ));
            };

            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http anthropic_server_tool_followup previous_response_id={previous_response_id} attempt={}",
                    followup_attempt + 1,
                ),
            );
            let followup_request = runtime_request_for_anthropic_server_tool_followup(
                &request.translated_request,
                &previous_response_id,
            )?;
            parts = match proxy_runtime_responses_request(request_id, &followup_request, shared)? {
                RuntimeResponsesReply::Buffered(parts) => parts,
                RuntimeResponsesReply::Streaming(response) => {
                    buffer_runtime_streaming_response_parts(response)?
                }
            };
        }

        unreachable!("anthropic buffered server-tool translation should return inside loop");
    }

    match response {
        RuntimeResponsesReply::Buffered(parts) => {
            translate_runtime_buffered_responses_reply_to_anthropic(parts, request)
        }
        RuntimeResponsesReply::Streaming(response) => {
            if !request.stream {
                let parts = buffer_runtime_streaming_response_parts(response)?;
                return translate_runtime_buffered_responses_reply_to_anthropic(parts, request);
            }

            let mut headers = response.headers;
            headers.retain(|(name, _)| !name.eq_ignore_ascii_case("content-type"));
            headers.push(("Content-Type".to_string(), "text/event-stream".to_string()));
            Ok(RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                status: response.status,
                headers,
                body: Box::new(RuntimeAnthropicSseReader::new(
                    response.body,
                    request.requested_model.clone(),
                    request.want_thinking,
                    request.carried_web_search_requests,
                    request.carried_web_fetch_requests,
                    request.carried_code_execution_requests,
                    request.carried_tool_search_requests,
                    request.server_tools.clone(),
                )),
                request_id: response.request_id,
                profile_name: response.profile_name,
                log_path: response.log_path,
                shared: response.shared,
                _inflight_guard: response._inflight_guard,
            }))
        }
    }
}

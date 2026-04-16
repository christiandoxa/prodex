use super::*;

pub(crate) fn runtime_anthropic_json_response_parts(
    value: serde_json::Value,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status: 200,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()),
    }
}

pub(crate) fn runtime_anthropic_sse_event_bytes(
    event_type: &str,
    data: serde_json::Value,
) -> Vec<u8> {
    format!(
        "event: {event_type}\ndata: {}\n\n",
        serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
    )
    .into_bytes()
}

pub(crate) fn runtime_anthropic_sse_response_parts_from_message_value(
    value: serde_json::Value,
) -> RuntimeBufferedResponseParts {
    let mut body = Vec::new();
    let message_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("msg_prodex")
        .to_string();
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("claude-sonnet-4-6")
        .to_string();
    let stop_reason = value
        .get("stop_reason")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let stop_sequence = value
        .get("stop_sequence")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let usage = value
        .get("usage")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let message_start_usage = serde_json::json!({
        "input_tokens": 0,
        "output_tokens": 0,
        "server_tool_use": usage
            .get("server_tool_use")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({
                "web_search_requests": 0,
                "web_fetch_requests": 0,
                "code_execution_requests": 0,
                "tool_search_requests": 0,
            })),
    });

    body.extend(runtime_anthropic_sse_event_bytes(
        "message_start",
        serde_json::json!({
            "type": "message_start",
            "message": {
                "id": message_id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": model,
                "stop_reason": serde_json::Value::Null,
                "stop_sequence": serde_json::Value::Null,
                "usage": message_start_usage,
            }
        }),
    ));

    for (index, block) in value
        .get("content")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let index_value = serde_json::Value::Number((index as u64).into());
        match block.get("type").and_then(serde_json::Value::as_str) {
            Some("thinking") => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "thinking",
                            "thinking": "",
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "thinking_delta",
                            "thinking": block
                                .get("thinking")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or(""),
                        }
                    }),
                ));
            }
            Some("tool_use") => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "tool_use",
                            "id": block.get("id").cloned().unwrap_or(serde_json::Value::String("tool_use".to_string())),
                            "name": block.get("name").cloned().unwrap_or(serde_json::Value::String("tool".to_string())),
                            "input": serde_json::json!({}),
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                ));
            }
            Some("server_tool_use") => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "server_tool_use",
                            "id": block.get("id").cloned().unwrap_or(serde_json::Value::String("server_tool_use".to_string())),
                            "name": block.get("name").cloned().unwrap_or(serde_json::Value::String("web_search".to_string())),
                            "input": serde_json::json!({}),
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                ));
            }
            Some("mcp_tool_use") => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "mcp_tool_use",
                            "id": block.get("id").cloned().unwrap_or(serde_json::Value::String("mcp_tool_use".to_string())),
                            "name": block.get("name").cloned().unwrap_or(serde_json::Value::String("mcp_tool".to_string())),
                            "server_name": block.get("server_name").cloned().unwrap_or_else(|| serde_json::Value::String("mcp".to_string())),
                            "input": serde_json::json!({}),
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                ));
            }
            Some(block_type) if block_type.ends_with("_tool_result") => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": block_type,
                            "tool_use_id": block.get("tool_use_id").cloned().unwrap_or_else(|| {
                                serde_json::Value::String(format!("{block_type}_call"))
                            }),
                            "content": block.get("content").cloned().unwrap_or(serde_json::Value::Null),
                        }
                    }),
                ));
            }
            Some("mcp_approval_request" | "mcp_list_tools") => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": block.clone(),
                    }),
                ));
            }
            _ => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "text",
                            "text": "",
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "text_delta",
                            "text": block.get("text").and_then(serde_json::Value::as_str).unwrap_or(""),
                        }
                    }),
                ));
            }
        }
        body.extend(runtime_anthropic_sse_event_bytes(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": index,
            }),
        ));
    }

    body.extend(runtime_anthropic_sse_event_bytes(
        "message_delta",
        serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": stop_sequence,
            },
            "usage": usage,
        }),
    ));
    body.extend(runtime_anthropic_sse_event_bytes(
        "message_stop",
        serde_json::json!({
            "type": "message_stop",
        }),
    ));

    RuntimeBufferedResponseParts {
        status: 200,
        headers: vec![("Content-Type".to_string(), b"text/event-stream".to_vec())],
        body,
    }
}

pub(crate) fn runtime_response_body_looks_like_sse(body: &[u8]) -> bool {
    let trimmed = body
        .iter()
        .copied()
        .skip_while(|byte| byte.is_ascii_whitespace());
    let prefix = trimmed.take(8).collect::<Vec<_>>();
    prefix.starts_with(b"event:") || prefix.starts_with(b"data:")
}

pub(crate) fn runtime_buffered_response_ids(parts: &RuntimeBufferedResponseParts) -> Vec<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&parts.body) {
        return extract_runtime_response_ids_from_value(&value);
    }

    let mut response_ids = Vec::new();
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let push_data_lines = |data_lines: &mut Vec<String>, response_ids: &mut Vec<String>| {
        if let Some(value) = parse_runtime_sse_payload(data_lines) {
            for response_id in extract_runtime_response_ids_from_value(&value) {
                push_runtime_response_id(response_ids, Some(&response_id));
            }
        }
        data_lines.clear();
    };

    for byte in &parts.body {
        line.push(*byte);
        if *byte != b'\n' {
            continue;
        }
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            push_data_lines(&mut data_lines, &mut response_ids);
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
    push_data_lines(&mut data_lines, &mut response_ids);
    response_ids
}

pub(crate) fn runtime_request_for_anthropic_server_tool_followup(
    request: &RuntimeProxyRequest,
    previous_response_id: &str,
) -> Result<RuntimeProxyRequest> {
    let mut value = serde_json::from_slice::<serde_json::Value>(&request.body)
        .context("failed to parse translated Anthropic request body")?;
    let object = value
        .as_object_mut()
        .context("translated Anthropic request body must be a JSON object")?;
    object.remove("input");
    object.remove("tool_choice");
    object.insert(
        "previous_response_id".to_string(),
        serde_json::Value::String(previous_response_id.to_string()),
    );
    let stream = object
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    object.insert("stream".to_string(), serde_json::Value::Bool(stream));
    let body = serde_json::to_vec(&value)
        .context("failed to serialize Anthropic server-tool follow-up request")?;
    Ok(RuntimeProxyRequest {
        method: request.method.clone(),
        path_and_query: request.path_and_query.clone(),
        headers: request.headers.clone(),
        body,
    })
}

pub(crate) fn runtime_anthropic_message_needs_server_tool_followup(
    value: &serde_json::Value,
) -> bool {
    let Some(content) = value.get("content").and_then(serde_json::Value::as_array) else {
        return false;
    };

    let mut saw_server_tool_use = false;
    for block in content {
        match block.get("type").and_then(serde_json::Value::as_str) {
            Some("server_tool_use") => saw_server_tool_use = true,
            Some("web_search_tool_result") => {}
            _ => return false,
        }
    }

    saw_server_tool_use
}

pub(crate) fn runtime_anthropic_message_server_tool_usage(
    value: &serde_json::Value,
) -> RuntimeAnthropicServerToolUsage {
    let usage = value
        .get("usage")
        .and_then(|usage| usage.get("server_tool_use"));
    RuntimeAnthropicServerToolUsage {
        web_search_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        web_fetch_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        code_execution_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("code_execution_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        tool_search_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("tool_search_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
    }
}

pub(crate) fn runtime_anthropic_message_from_buffered_responses_parts_with_carried_usage(
    parts: &RuntimeBufferedResponseParts,
    request: &RuntimeAnthropicMessagesRequest,
    carried_usage: RuntimeAnthropicServerToolUsage,
) -> Result<serde_json::Value> {
    let content_type = runtime_buffered_response_content_type(parts)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let looks_like_sse = content_type.contains("text/event-stream")
        || runtime_response_body_looks_like_sse(&parts.body);
    if looks_like_sse {
        return runtime_anthropic_response_from_sse_bytes_with_carried_usage(
            &parts.body,
            &request.requested_model,
            request.want_thinking,
            carried_usage.web_search_requests,
            carried_usage.web_fetch_requests,
            carried_usage.code_execution_requests,
            carried_usage.tool_search_requests,
            Some(&request.server_tools),
        );
    }

    let value = serde_json::from_slice::<serde_json::Value>(&parts.body)
        .context("failed to parse buffered Responses JSON body")?;
    Ok(
        runtime_anthropic_response_from_json_value_with_carried_usage(
            &value,
            &request.requested_model,
            request.want_thinking,
            carried_usage.web_search_requests,
            carried_usage.web_fetch_requests,
            carried_usage.code_execution_requests,
            carried_usage.tool_search_requests,
            Some(&request.server_tools),
        ),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_anthropic_sse_response_parts_from_responses_sse_bytes(
    body: &[u8],
    requested_model: &str,
    want_thinking: bool,
    carried_web_search_requests: u64,
    carried_web_fetch_requests: u64,
    carried_code_execution_requests: u64,
    carried_tool_search_requests: u64,
    server_tools: &RuntimeAnthropicServerTools,
) -> Result<RuntimeBufferedResponseParts> {
    let response = runtime_anthropic_response_from_sse_bytes_with_carried_usage(
        body,
        requested_model,
        want_thinking,
        carried_web_search_requests,
        carried_web_fetch_requests,
        carried_code_execution_requests,
        carried_tool_search_requests,
        Some(server_tools),
    )
    .context("failed to translate buffered Responses SSE body")?;
    Ok(runtime_anthropic_sse_response_parts_from_message_value(
        response,
    ))
}

pub(crate) fn buffer_runtime_streaming_response_parts(
    response: RuntimeStreamingResponse,
) -> Result<RuntimeBufferedResponseParts> {
    let RuntimeStreamingResponse {
        status,
        headers,
        mut body,
        ..
    } = response;
    let mut buffered_body = Vec::new();
    body.read_to_end(&mut buffered_body)
        .context("failed to buffer streaming runtime response")?;
    Ok(RuntimeBufferedResponseParts {
        status,
        headers: headers
            .into_iter()
            .map(|(name, value)| (name, value.into_bytes()))
            .collect(),
        body: buffered_body,
    })
}

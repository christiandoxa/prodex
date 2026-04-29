use super::*;
use runtime_anthropic_crate as anthropic;

#[allow(unused_imports)]
pub(super) use anthropic::{
    RUNTIME_PROXY_ANTHROPIC_MEMORY_TOOL_INSTRUCTIONS, RuntimeAnthropicMcpServer,
    RuntimeAnthropicNativeClientToolCall, RuntimeAnthropicNativeClientToolKind,
    RuntimeAnthropicRegisteredServerTool, RuntimeAnthropicServerToolUsage,
    RuntimeAnthropicServerTools, RuntimeAnthropicSseReader, RuntimeAnthropicStreamToolUse,
    RuntimeAnthropicTranslatedTools, runtime_anthropic_code_execution_request_count_from_output,
    runtime_anthropic_computer_key_combo_from_output_action,
    runtime_anthropic_computer_tool_input_from_output_item,
    runtime_anthropic_computer_tool_use_block_from_output_item,
    runtime_anthropic_error_type_for_status,
    runtime_anthropic_mcp_approval_request_block_from_output_item,
    runtime_anthropic_mcp_call_blocks_from_output_item,
    runtime_anthropic_mcp_list_tools_block_from_output_item,
    runtime_anthropic_message_annotation_titles_by_url, runtime_anthropic_message_id,
    runtime_anthropic_message_needs_server_tool_followup,
    runtime_anthropic_message_server_tool_usage, runtime_anthropic_output_blocks_from_json,
    runtime_anthropic_output_item_call_id, runtime_anthropic_output_item_name,
    runtime_anthropic_output_item_server_tool_usage, runtime_anthropic_output_item_type,
    runtime_anthropic_raw_computer_tool_input_from_output_item,
    runtime_anthropic_reasoning_summary_text, runtime_anthropic_response_event_error_message,
    runtime_anthropic_response_event_item, runtime_anthropic_response_from_json_value,
    runtime_anthropic_response_from_json_value_with_carried_usage,
    runtime_anthropic_response_from_sse_bytes,
    runtime_anthropic_response_from_sse_bytes_with_carried_usage,
    runtime_anthropic_server_tool_name_for_call,
    runtime_anthropic_server_tool_registration_for_call, runtime_anthropic_server_tool_use_block,
    runtime_anthropic_shell_tool_input_from_output_item,
    runtime_anthropic_shell_tool_use_block_from_output_item, runtime_anthropic_sse_event_bytes,
    runtime_anthropic_tool_input_from_arguments,
    runtime_anthropic_tool_search_request_count_from_output,
    runtime_anthropic_tool_usage_code_execution_requests_from_value,
    runtime_anthropic_tool_usage_tool_search_requests_from_value,
    runtime_anthropic_tool_usage_web_search_requests_from_value,
    runtime_anthropic_usage_from_value, runtime_anthropic_usage_json,
    runtime_anthropic_web_fetch_request_count_from_output,
    runtime_anthropic_web_search_blocks_from_output_item,
    runtime_anthropic_web_search_request_count_from_output,
    runtime_anthropic_web_search_request_count_from_output_item,
    runtime_proxy_anthropic_append_tool_instructions,
    runtime_proxy_anthropic_builtin_client_tool_description,
    runtime_proxy_anthropic_builtin_client_tool_name_from_type,
    runtime_proxy_anthropic_builtin_client_tool_schema,
    runtime_proxy_anthropic_builtin_server_tool_name,
    runtime_proxy_anthropic_carried_server_tool_usage, runtime_proxy_anthropic_client_tool_name,
    runtime_proxy_anthropic_coordinate_component, runtime_proxy_anthropic_coordinate_pair,
    runtime_proxy_anthropic_default_tool_schema,
    runtime_proxy_anthropic_has_ambiguous_native_shell_choice,
    runtime_proxy_anthropic_has_ambiguous_native_tool_choice,
    runtime_proxy_anthropic_image_data_url,
    runtime_proxy_anthropic_is_special_input_item_block_type,
    runtime_proxy_anthropic_is_tool_result_block_type,
    runtime_proxy_anthropic_is_tool_use_block_type,
    runtime_proxy_anthropic_message_has_tool_chain_blocks,
    runtime_proxy_anthropic_model_descriptor, runtime_proxy_anthropic_model_display_name,
    runtime_proxy_anthropic_model_id_from_path, runtime_proxy_anthropic_models_list,
    runtime_proxy_anthropic_native_computer_enabled_for_request,
    runtime_proxy_anthropic_native_shell_enabled_for_request,
    runtime_proxy_anthropic_normalize_tool_schema, runtime_proxy_anthropic_reasoning_effort,
    runtime_proxy_anthropic_register_server_tools_from_messages,
    runtime_proxy_anthropic_server_tool_name_from_type, runtime_proxy_anthropic_tool_description,
    runtime_proxy_anthropic_tool_use_server_tool_usage, runtime_proxy_anthropic_tool_version,
    runtime_proxy_anthropic_unversioned_tool_type, runtime_proxy_anthropic_wants_thinking,
    runtime_proxy_anthropic_web_search_query_from_tool_result_text,
    runtime_proxy_anthropic_web_search_urls_from_tool_result_text,
    runtime_proxy_compact_web_search_tool_result_summary,
    runtime_proxy_extract_balanced_json_array_bounds,
    runtime_proxy_normalize_anthropic_tool_result_text, runtime_proxy_request_header_value,
    runtime_proxy_translate_anthropic_block_fallback_text,
    runtime_proxy_translate_anthropic_computer_action,
    runtime_proxy_translate_anthropic_computer_tool_call,
    runtime_proxy_translate_anthropic_computer_tool_result,
    runtime_proxy_translate_anthropic_document_text,
    runtime_proxy_translate_anthropic_error_tool_result_output,
    runtime_proxy_translate_anthropic_image_part,
    runtime_proxy_translate_anthropic_mcp_approval_response,
    runtime_proxy_translate_anthropic_mcp_servers, runtime_proxy_translate_anthropic_mcp_tool,
    runtime_proxy_translate_anthropic_message_content,
    runtime_proxy_translate_anthropic_reasoning_effort,
    runtime_proxy_translate_anthropic_shell_tool_call,
    runtime_proxy_translate_anthropic_shell_tool_result,
    runtime_proxy_translate_anthropic_text_blocks,
    runtime_proxy_translate_anthropic_text_from_block, runtime_proxy_translate_anthropic_tool,
    runtime_proxy_translate_anthropic_tool_call, runtime_proxy_translate_anthropic_tool_choice,
    runtime_proxy_translate_anthropic_tool_result,
    runtime_proxy_translate_anthropic_tool_result_content,
    runtime_proxy_translate_anthropic_tool_result_payload, runtime_proxy_translate_anthropic_tools,
    runtime_proxy_translate_anthropic_user_content_blocks, runtime_response_body_looks_like_sse,
};

pub(super) fn handle_runtime_proxy_anthropic_compat_request(
    request: &tiny_http::Request,
) -> Option<tiny_http::ResponseBox> {
    let path = path_without_query(request.url());
    let method = request.method().as_str();
    if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
        return None;
    }

    match path {
        "/" => Some(build_runtime_proxy_json_response(
            200,
            serde_json::json!({
                "service": "prodex",
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
            })
            .to_string(),
        )),
        RUNTIME_PROXY_ANTHROPIC_HEALTH_PATH => Some(build_runtime_proxy_json_response(
            200,
            serde_json::json!({
                "status": "ok",
            })
            .to_string(),
        )),
        RUNTIME_PROXY_ANTHROPIC_MODELS_PATH => Some(build_runtime_proxy_json_response(
            200,
            runtime_proxy_anthropic_models_list().to_string(),
        )),
        _ => runtime_proxy_anthropic_model_id_from_path(path).map(|model_id| {
            build_runtime_proxy_json_response(
                200,
                runtime_proxy_anthropic_model_descriptor(model_id).to_string(),
            )
        }),
    }
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeAnthropicMessagesRequest {
    pub(super) translated_request: RuntimeProxyRequest,
    pub(super) requested_model: String,
    pub(super) stream: bool,
    pub(super) want_thinking: bool,
    pub(super) server_tools: RuntimeAnthropicServerTools,
    pub(super) carried_web_search_requests: u64,
    pub(super) carried_web_fetch_requests: u64,
    pub(super) carried_code_execution_requests: u64,
    pub(super) carried_tool_search_requests: u64,
}

fn runtime_request_to_anthropic(request: &RuntimeProxyRequest) -> anthropic::RuntimeProxyRequest {
    anthropic::RuntimeProxyRequest {
        method: request.method.clone(),
        path_and_query: request.path_and_query.clone(),
        headers: request.headers.clone(),
        body: request.body.clone(),
    }
}

fn runtime_request_from_anthropic(request: anthropic::RuntimeProxyRequest) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: request.method,
        path_and_query: request.path_and_query,
        headers: request.headers,
        body: request.body,
    }
}

fn buffered_parts_to_anthropic(
    parts: RuntimeBufferedResponseParts,
) -> anthropic::RuntimeBufferedResponseParts {
    anthropic::RuntimeBufferedResponseParts {
        status: parts.status,
        headers: parts.headers,
        body: parts.body.into_vec(),
    }
}

fn buffered_parts_ref_to_anthropic(
    parts: &RuntimeBufferedResponseParts,
) -> anthropic::RuntimeBufferedResponseParts {
    anthropic::RuntimeBufferedResponseParts {
        status: parts.status,
        headers: parts.headers.clone(),
        body: parts.body.to_vec(),
    }
}

fn buffered_parts_from_anthropic(
    parts: anthropic::RuntimeBufferedResponseParts,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status: parts.status,
        headers: parts.headers,
        body: parts.body.into(),
    }
}

fn messages_request_to_anthropic(
    request: &RuntimeAnthropicMessagesRequest,
) -> anthropic::RuntimeAnthropicMessagesRequest {
    anthropic::RuntimeAnthropicMessagesRequest {
        translated_request: runtime_request_to_anthropic(&request.translated_request),
        requested_model: request.requested_model.clone(),
        stream: request.stream,
        want_thinking: request.want_thinking,
        server_tools: request.server_tools.clone(),
        carried_web_search_requests: request.carried_web_search_requests,
        carried_web_fetch_requests: request.carried_web_fetch_requests,
        carried_code_execution_requests: request.carried_code_execution_requests,
        carried_tool_search_requests: request.carried_tool_search_requests,
    }
}

fn messages_request_from_anthropic(
    request: anthropic::RuntimeAnthropicMessagesRequest,
) -> RuntimeAnthropicMessagesRequest {
    RuntimeAnthropicMessagesRequest {
        translated_request: runtime_request_from_anthropic(request.translated_request),
        requested_model: request.requested_model,
        stream: request.stream,
        want_thinking: request.want_thinking,
        server_tools: request.server_tools,
        carried_web_search_requests: request.carried_web_search_requests,
        carried_web_fetch_requests: request.carried_web_fetch_requests,
        carried_code_execution_requests: request.carried_code_execution_requests,
        carried_tool_search_requests: request.carried_tool_search_requests,
    }
}

pub(super) fn translate_runtime_anthropic_messages_request(
    request: &RuntimeProxyRequest,
) -> Result<RuntimeAnthropicMessagesRequest> {
    anthropic::translate_runtime_anthropic_messages_request(&runtime_request_to_anthropic(request))
        .map(messages_request_from_anthropic)
}

pub(super) fn runtime_anthropic_json_response_parts(
    value: serde_json::Value,
) -> RuntimeBufferedResponseParts {
    buffered_parts_from_anthropic(anthropic::runtime_anthropic_json_response_parts(value))
}

pub(super) fn runtime_anthropic_sse_response_parts_from_message_value(
    value: serde_json::Value,
) -> RuntimeBufferedResponseParts {
    buffered_parts_from_anthropic(
        anthropic::runtime_anthropic_sse_response_parts_from_message_value(value),
    )
}

pub(super) fn runtime_buffered_response_ids(parts: &RuntimeBufferedResponseParts) -> Vec<String> {
    anthropic::runtime_buffered_response_ids(&buffered_parts_ref_to_anthropic(parts))
}

pub(super) fn runtime_request_for_anthropic_server_tool_followup(
    request: &RuntimeProxyRequest,
    previous_response_id: &str,
) -> Result<RuntimeProxyRequest> {
    anthropic::runtime_request_for_anthropic_server_tool_followup(
        &runtime_request_to_anthropic(request),
        previous_response_id,
    )
    .map(runtime_request_from_anthropic)
}

pub(super) fn runtime_anthropic_message_from_buffered_responses_parts_with_carried_usage(
    parts: &RuntimeBufferedResponseParts,
    request: &RuntimeAnthropicMessagesRequest,
    carried_usage: RuntimeAnthropicServerToolUsage,
) -> Result<serde_json::Value> {
    anthropic::runtime_anthropic_message_from_buffered_responses_parts_with_carried_usage(
        &buffered_parts_ref_to_anthropic(parts),
        &messages_request_to_anthropic(request),
        carried_usage,
    )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn runtime_anthropic_sse_response_parts_from_responses_sse_bytes(
    body: &[u8],
    requested_model: &str,
    want_thinking: bool,
    carried_usage: RuntimeAnthropicServerToolUsage,
    server_tools: &RuntimeAnthropicServerTools,
) -> Result<RuntimeBufferedResponseParts> {
    anthropic::runtime_anthropic_sse_response_parts_from_responses_sse_bytes(
        body,
        requested_model,
        want_thinking,
        carried_usage.web_search_requests,
        carried_usage.web_fetch_requests,
        carried_usage.code_execution_requests,
        carried_usage.tool_search_requests,
        server_tools,
    )
    .map(buffered_parts_from_anthropic)
}

#[allow(dead_code)]
pub(super) fn runtime_anthropic_error_message_from_parts(
    parts: &RuntimeBufferedResponseParts,
) -> String {
    anthropic::runtime_anthropic_error_message_from_parts(&buffered_parts_ref_to_anthropic(parts))
}

pub(super) fn build_runtime_anthropic_error_parts(
    status: u16,
    error_type: &str,
    message: &str,
) -> RuntimeBufferedResponseParts {
    buffered_parts_from_anthropic(anthropic::build_runtime_anthropic_error_parts(
        status, error_type, message,
    ))
}

pub(super) fn runtime_anthropic_error_from_upstream_parts(
    parts: RuntimeBufferedResponseParts,
) -> RuntimeBufferedResponseParts {
    buffered_parts_from_anthropic(anthropic::runtime_anthropic_error_from_upstream_parts(
        buffered_parts_to_anthropic(parts),
    ))
}

pub(super) fn buffer_runtime_streaming_response_parts(
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
        body: buffered_body.into(),
    })
}

pub(super) fn translate_runtime_buffered_responses_reply_to_anthropic(
    parts: RuntimeBufferedResponseParts,
    request: &RuntimeAnthropicMessagesRequest,
) -> Result<RuntimeResponsesReply> {
    let reply = anthropic::translate_runtime_buffered_responses_reply_to_anthropic(
        buffered_parts_to_anthropic(parts),
        &messages_request_to_anthropic(request),
    )?;
    match reply {
        anthropic::RuntimeResponsesReply::Buffered(parts) => Ok(RuntimeResponsesReply::Buffered(
            buffered_parts_from_anthropic(parts),
        )),
        anthropic::RuntimeResponsesReply::Streaming(_) => {
            bail!("Anthropic buffered translation unexpectedly returned streaming response")
        }
    }
}

pub(super) fn translate_runtime_responses_reply_to_anthropic(
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
                        runtime_proxy_redacted_body_snippet(&parts.body, 2048),
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

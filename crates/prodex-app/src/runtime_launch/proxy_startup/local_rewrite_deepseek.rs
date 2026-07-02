use super::deepseek_rewrite::{
    RuntimeDeepSeekPendingRequest, RuntimeDeepSeekRewriteOptions,
    runtime_deepseek_chat_request_body_with_options,
};
use super::local_rewrite::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
    runtime_local_rewrite_model_selection,
};
use super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_deepseek_upstream_url,
    runtime_local_rewrite_api_key_attempts, send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
    runtime_provider_log_request_conformance, runtime_provider_model_fallback_chain,
    runtime_provider_request_body_with_model, runtime_provider_request_conformance_result,
    runtime_provider_should_retry_with_next_model,
    runtime_provider_should_rotate_auth_after_response,
};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, runtime_proxy_log};
use anyhow::Result;
use prodex_provider_core::ProviderTransformLoss;
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};

pub(super) fn send_runtime_deepseek_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_keys: &[String],
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let api_key_attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys);
    if api_key_attempts.is_empty() {
        anyhow::bail!("DeepSeek provider has no API keys configured");
    }
    let api_key_attempt_count = api_key_attempts.len();
    if path_without_query(&request.path_and_query).ends_with("/responses") {
        send_runtime_deepseek_responses_request(
            request_id,
            request,
            shared,
            body,
            api_key_attempts,
            api_key_attempt_count,
        )
    } else {
        send_runtime_deepseek_passthrough_request(
            request_id,
            request,
            shared,
            body,
            api_key_attempts,
            api_key_attempt_count,
        )
    }
}

fn send_runtime_deepseek_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_key_attempts: Vec<(String, &str)>,
    api_key_attempt_count: usize,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let model_selection = runtime_local_rewrite_model_selection(
        shared,
        RuntimeProviderBridgeKind::DeepSeek,
        request,
        &body,
        prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL,
    );
    let model_chain = runtime_provider_model_fallback_chain(
        RuntimeProviderBridgeKind::DeepSeek,
        &model_selection.model,
    );
    let (strict_tools, beta_base_url, web_search_mode) = match &shared.provider {
        super::local_rewrite_options::RuntimeLocalRewriteProviderOptions::DeepSeek {
            strict_tools,
            beta_base_url,
            web_search_mode,
            ..
        } => (*strict_tools, beta_base_url.as_str(), *web_search_mode),
        _ => (
            false,
            shared.upstream_base_url.as_str(),
            super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode::Auto,
        ),
    };
    let upstream_base_url = if strict_tools {
        beta_base_url
    } else {
        &shared.upstream_base_url
    };
    let upstream_url = runtime_deepseek_upstream_url(
        upstream_base_url,
        &shared.mount_path,
        &request.path_and_query,
    );
    for (api_key_index, (api_key_label, api_key)) in api_key_attempts.into_iter().enumerate() {
        for (model_index, model) in model_chain.iter().enumerate() {
            let model_body = runtime_provider_request_body_with_model(&model_selection.body, model);
            let conformance = runtime_provider_request_conformance_result(
                RuntimeProviderBridgeKind::DeepSeek,
                request,
                &model_body,
            );
            if let Some(result) = conformance.as_ref() {
                runtime_provider_log_request_conformance(
                    &shared.runtime_shared,
                    request_id,
                    RuntimeProviderBridgeKind::DeepSeek,
                    &result,
                );
            }
            let mut translated = runtime_deepseek_chat_request_body_with_options(
                &model_body,
                &shared.deepseek_conversations,
                RuntimeDeepSeekRewriteOptions {
                    strict_tools,
                    web_search_mode,
                },
            )?;
            if runtime_deepseek_provider_core_simple_request(&model_body)
                && let Some(body) = conformance
                    .as_ref()
                    .and_then(runtime_deepseek_provider_core_request_body)
            {
                translated.body = body;
            }
            if let Ok(mut pending) = shared.deepseek_pending_messages.lock() {
                pending.insert(
                    request_id,
                    RuntimeDeepSeekPendingRequest {
                        messages: translated.messages,
                        response_metadata: translated.response_metadata,
                    },
                );
            }
            let send_result =
                send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
                    RuntimeLocalRewriteSearchFallbackRequest {
                        request_id,
                        request,
                        shared,
                        upstream_url: &upstream_url,
                        body: translated.body,
                        provider_kind: RuntimeProviderBridgeKind::DeepSeek,
                        auth_label: &api_key_label,
                        model,
                        auth_factory: || RuntimeLocalRewritePreparedAuth::DeepSeek { api_key },
                    },
                )?;
            let (status, parts, class) = match send_result {
                RuntimeLocalRewritePreparedSendResult::Live(response) => {
                    return Ok(runtime_deepseek_live_result(response));
                }
                RuntimeLocalRewritePreparedSendResult::Error {
                    status,
                    parts,
                    class,
                } => (status, parts, class),
            };
            if model_index + 1 < model_chain.len()
                && runtime_provider_should_retry_with_next_model(class)
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_model_fallback",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("provider", "deepseek"),
                            runtime_proxy_log_field("auth", api_key_label.as_str()),
                            runtime_proxy_log_field("from_model", model.as_str()),
                            runtime_proxy_log_field(
                                "to_model",
                                model_chain[model_index + 1].as_str(),
                            ),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("class", format!("{class:?}")),
                        ],
                    ),
                );
                continue;
            }
            if api_key_index + 1 < api_key_attempt_count
                && runtime_provider_should_rotate_auth_after_response(class)
            {
                runtime_deepseek_log_auth_rotate(shared, request_id, &api_key_label, status, class);
                break;
            }
            return Ok(runtime_deepseek_buffered_result(parts));
        }
        if api_key_index + 1 < api_key_attempt_count {
            continue;
        }
    }
    anyhow::bail!("no DeepSeek model attempts were available");
}

fn send_runtime_deepseek_passthrough_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_key_attempts: Vec<(String, &str)>,
    api_key_attempt_count: usize,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let upstream_url = runtime_deepseek_upstream_url(
        &shared.upstream_base_url,
        &shared.mount_path,
        &request.path_and_query,
    );
    for (api_key_index, (api_key_label, api_key)) in api_key_attempts.into_iter().enumerate() {
        let response = send_runtime_local_rewrite_prepared_request(
            request_id,
            request,
            shared,
            &upstream_url,
            body.clone(),
            RuntimeLocalRewritePreparedAuth::DeepSeek { api_key },
        )?;
        let status = response.status().as_u16();
        if status >= 400 {
            let parts = runtime_local_rewrite_buffered_response_from_response(response)?;
            let class = runtime_provider_error_class(
                RuntimeProviderBridgeKind::DeepSeek,
                status,
                &parts.body,
            );
            if api_key_index + 1 < api_key_attempt_count
                && runtime_provider_should_rotate_auth_after_response(class)
            {
                runtime_deepseek_log_auth_rotate(shared, request_id, &api_key_label, status, class);
                continue;
            }
            return Ok(runtime_deepseek_buffered_result(parts));
        }
        return Ok(runtime_deepseek_live_result(response));
    }
    anyhow::bail!("no DeepSeek API key attempts were available")
}

fn runtime_deepseek_log_auth_rotate(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    api_key_label: &str,
    status: u16,
    class: RuntimeProviderErrorClass,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_auth_rotate",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("provider", "deepseek"),
                runtime_proxy_log_field("auth", api_key_label),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("class", format!("{class:?}")),
            ],
        ),
    );
}

fn runtime_deepseek_buffered_result(
    parts: RuntimeHeapTrimmedBufferedResponseParts,
) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
        gemini_context: None,
        copilot_context: None,
    }
}

fn runtime_deepseek_live_result(
    response: reqwest::blocking::Response,
) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Live(RuntimeLocalRewriteLiveResponse::new(
            response,
        )),
        gemini_context: None,
        copilot_context: None,
    }
}

fn runtime_deepseek_provider_core_simple_request(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.contains_key("previous_response_id")
        || object.contains_key("instructions")
        || object.contains_key("tools")
        || object.contains_key("tool_choice")
        || object.contains_key("response_format")
        || object.contains_key("web_search_options")
        || object.contains_key("safety_identifier")
    {
        return false;
    }
    match object.get("input") {
        Some(serde_json::Value::String(_)) => true,
        Some(serde_json::Value::Array(items)) => {
            items.iter().all(runtime_deepseek_simple_input_item)
        }
        _ => false,
    }
}

fn runtime_deepseek_simple_input_item(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    match object.get("type").and_then(serde_json::Value::as_str) {
        Some("message") | None => {}
        Some("function_call_output") => {
            return object
                .get("call_id")
                .is_some_and(serde_json::Value::is_string)
                && object.get("output").is_some();
        }
        Some("mcp_tool_result") | Some("mcp_call_output") => {
            return object
                .get("call_id")
                .or_else(|| object.get("tool_call_id"))
                .or_else(|| object.get("id"))
                .is_some_and(serde_json::Value::is_string)
                && ["output", "content", "result", "error"]
                    .iter()
                    .any(|key| object.contains_key(*key));
        }
        Some("custom_tool_call_output") => {
            return object
                .get("call_id")
                .is_some_and(serde_json::Value::is_string)
                && ["output", "content", "result", "error"]
                    .iter()
                    .any(|key| object.contains_key(*key));
        }
        Some("function_call") => {
            let has_name = object
                .get("name")
                .or_else(|| object.get("tool_name"))
                .or_else(|| {
                    object
                        .get("function")
                        .and_then(|function| function.get("name"))
                })
                .is_some_and(serde_json::Value::is_string);
            return has_name
                && object
                    .get("call_id")
                    .or_else(|| object.get("tool_call_id"))
                    .or_else(|| object.get("id"))
                    .is_none_or(serde_json::Value::is_string);
        }
        Some("mcp_call") => {
            let has_name = object
                .get("name")
                .or_else(|| object.get("tool_name"))
                .or_else(|| {
                    object
                        .get("function")
                        .and_then(|function| function.get("name"))
                })
                .is_some_and(serde_json::Value::is_string);
            return has_name
                && object
                    .get("call_id")
                    .or_else(|| object.get("tool_call_id"))
                    .or_else(|| object.get("id"))
                    .is_none_or(serde_json::Value::is_string);
        }
        Some("custom_tool_call") => {
            let has_name = object
                .get("name")
                .or_else(|| object.get("tool_name"))
                .is_some_and(serde_json::Value::is_string);
            return has_name
                && object
                    .get("call_id")
                    .or_else(|| object.get("tool_call_id"))
                    .or_else(|| object.get("id"))
                    .is_none_or(serde_json::Value::is_string);
        }
        Some("local_shell_call") => {
            let has_call_id = object
                .get("call_id")
                .or_else(|| object.get("tool_call_id"))
                .or_else(|| object.get("id"))
                .is_none_or(serde_json::Value::is_string);
            let has_command = object
                .get("command")
                .is_some_and(serde_json::Value::is_string)
                || object
                    .get("action")
                    .and_then(|action| action.get("command"))
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|parts| parts.iter().all(serde_json::Value::is_string));
            return has_call_id && has_command;
        }
        Some(_) => return false,
    }
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("user");
    if !matches!(role, "system" | "user" | "assistant" | "tool") {
        return false;
    }
    match object.get("content") {
        Some(serde_json::Value::String(_)) | None => true,
        Some(serde_json::Value::Array(parts)) => {
            parts.iter().all(runtime_deepseek_simple_content_item)
        }
        _ => false,
    }
}

fn runtime_deepseek_simple_content_item(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object
        .get("type")
        .is_some_and(|value| !matches!(value.as_str(), Some("input_text" | "output_text" | "text")))
    {
        return false;
    }
    object
        .get("text")
        .or_else(|| object.get("input_text"))
        .or_else(|| object.get("output_text"))
        .is_some_and(serde_json::Value::is_string)
}

fn runtime_deepseek_provider_core_request_body(
    result: &prodex_provider_core::ProviderTransformResult,
) -> Option<Vec<u8>> {
    match result.loss {
        ProviderTransformLoss::Lossless | ProviderTransformLoss::DegradedButSafe { .. } => {
            result.body.clone()
        }
        ProviderTransformLoss::Rejected { .. }
        | ProviderTransformLoss::UnsupportedUpstream { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::runtime_deepseek_provider_core_simple_request;

    #[test]
    fn deepseek_provider_core_simple_request_accepts_plain_text_first_turn() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "hello",
            "stream": true,
            "temperature": 0.2
        }))
        .unwrap();
        assert!(runtime_deepseek_provider_core_simple_request(&body));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_message_content_arrays() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "system",
                    "content": [{"type": "input_text", "text": "You are Codex."}]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "read commit history"}]
                }
            ]
        }))
        .unwrap();
        assert!(runtime_deepseek_provider_core_simple_request(&body));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_assistant_tool_history() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "find the symbol"}]
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": ""}],
                    "tool_calls": [
                        {
                            "id":"call_1",
                            "type":"function",
                            "function":{
                                "name":"grep",
                                "arguments":"{\"pattern\":\"ProviderTranslator\"}"
                            }
                        }
                    ]
                },
                {
                    "type": "message",
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": [{"type": "output_text", "text": "{\"match_count\":1}"}]
                }
            ]
        }))
        .unwrap();
        assert!(runtime_deepseek_provider_core_simple_request(&body));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_function_call_and_output_items() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "find the symbol"}]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "grep",
                    "arguments": {"pattern": "ProviderTranslator"}
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "{\"match_count\":1}"
                }
            ]
        }))
        .unwrap();
        assert!(runtime_deepseek_provider_core_simple_request(&body));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_custom_and_shell_call_items() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "custom_tool_call",
                    "call_id": "call_patch_1",
                    "name": "apply_patch",
                    "input": "*** Begin Patch\n*** End Patch"
                },
                {
                    "type": "custom_tool_call_output",
                    "call_id": "call_patch_1",
                    "output": "applied"
                },
                {
                    "type": "local_shell_call",
                    "call_id": "call_shell_1",
                    "action": {
                        "command": ["git", "status", "--short"]
                    }
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_shell_1",
                    "output": " M Cargo.toml"
                }
            ]
        }))
        .unwrap();
        assert!(runtime_deepseek_provider_core_simple_request(&body));
    }

    #[test]
    fn deepseek_provider_core_simple_request_accepts_mcp_call_and_result_items() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "mcp_call",
                    "id": "call_sqz_1",
                    "name": "mcp__prodex_sqz__compress",
                    "arguments": {"text": "large repeated content"},
                    "output": "ref:abc123"
                },
                {
                    "type": "mcp_tool_result",
                    "call_id": "call_sqz_1",
                    "content": [{"type": "output_text", "text": "ref:abc123"}]
                }
            ]
        }))
        .unwrap();
        assert!(runtime_deepseek_provider_core_simple_request(&body));
    }

    #[test]
    fn deepseek_provider_core_simple_request_rejects_continuations_and_tools() {
        let continuation = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "hello",
            "previous_response_id": "resp_1"
        }))
        .unwrap();
        assert!(!runtime_deepseek_provider_core_simple_request(
            &continuation
        ));

        let tools = serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": "hello",
            "tools": [{"type":"function","function":{"name":"search"}}]
        }))
        .unwrap();
        assert!(!runtime_deepseek_provider_core_simple_request(&tools));
    }
}

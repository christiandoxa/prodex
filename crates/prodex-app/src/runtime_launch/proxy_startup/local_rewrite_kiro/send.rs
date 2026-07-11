use super::chat_compat::runtime_kiro_chat_completion_value_from_response;
use super::runtime_kiro_json_parts;
use super::streaming::runtime_kiro_streaming_reader;
use super::streaming_anthropic::{
    runtime_kiro_anthropic_message_parts_from_response,
    runtime_kiro_anthropic_streaming_local_response,
};
use super::{
    RuntimeKiroProfileAuth, runtime_kiro_prompt_from_messages, runtime_kiro_request_body_for_path,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use crate::RuntimeProxyRequest;
use crate::runtime_anthropic::{
    build_runtime_anthropic_error_parts, translate_runtime_anthropic_messages_request,
};
use crate::runtime_kiro_acp::{
    runtime_kiro_acp_chat_assistant_messages_from_prompt_turn,
    runtime_kiro_acp_responses_value_from_prompt_turn,
};
use crate::runtime_launch::proxy_startup::chat_compatible_rewrite::runtime_provider_chat_compatible_request_body;
use crate::runtime_launch::proxy_startup::deepseek_rewrite::runtime_deepseek_store_conversation;
use crate::runtime_launch::proxy_startup::local_rewrite::{
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult,
};
use crate::runtime_launch::proxy_startup::local_rewrite_upstream::RuntimeLocalRewriteStreamingResponse;
use crate::runtime_launch::proxy_startup::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderRouteKind, runtime_provider_route_kind,
};
use anyhow::{Context, Result};
use runtime_proxy_crate::path_without_query;
use serde_json::Value;

pub(crate) fn send_runtime_kiro_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeKiroProfileAuth,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let path = path_without_query(&request.path_and_query);
    let route_kind = runtime_provider_route_kind(path);
    let chat_completions_route =
        matches!(route_kind, Some(RuntimeProviderRouteKind::ChatCompletions));
    let messages_route = path.ends_with("/messages");
    if !(matches!(route_kind, Some(RuntimeProviderRouteKind::Responses))
        || chat_completions_route
        || messages_route)
    {
        return Ok(RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Buffered(runtime_kiro_json_parts(
                501,
                prodex_provider_core::kiro_provider_core_unsupported_path_error_value(path),
            )),
            gemini_context: None,
            copilot_context: None,
        });
    }
    let anthropic_request = if messages_route {
        match translate_runtime_anthropic_messages_request(request) {
            Ok(translated) => Some(translated),
            Err(err) => {
                return Ok(RuntimeLocalRewriteUpstreamResult {
                    response: RuntimeLocalRewriteUpstreamResponse::Buffered(
                        build_runtime_anthropic_error_parts(
                            400,
                            "invalid_request_error",
                            &err.to_string(),
                        ),
                    ),
                    gemini_context: None,
                    copilot_context: None,
                });
            }
        }
    } else {
        None
    };
    let client_stream = if messages_route {
        serde_json::from_slice::<Value>(&request.body)
            .ok()
            .and_then(|value| value.get("stream").and_then(Value::as_bool))
            .unwrap_or(false)
    } else {
        false
    };
    let body = anthropic_request
        .as_ref()
        .map(|translated| translated.translated_request.body.clone())
        .unwrap_or(body);
    let body = match runtime_kiro_request_body_for_path(path, body) {
        Ok(body) => body,
        Err(parts) => {
            return Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                gemini_context: None,
                copilot_context: None,
            });
        }
    };

    let value: Value =
        serde_json::from_slice(&body).context("failed to parse Codex Responses request JSON")?;
    let stream = if messages_route {
        client_stream
    } else {
        value
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };

    let translated = runtime_provider_chat_compatible_request_body(
        &body,
        &shared.deepseek_conversations,
        RuntimeProviderBridgeKind::Kiro,
        "",
        false,
        Default::default(),
    )?;
    let prompt_messages = translated.messages.clone();
    let prompt = runtime_kiro_prompt_from_messages(&translated.messages);
    super::invoke::runtime_kiro_prompt_turn(
        auth,
        "runtime",
        &prompt,
        &shared.runtime_shared.async_runtime,
    )
    .and_then(|turn| {
        let mut response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, request_id);
        prodex_provider_core::kiro_provider_core_apply_response_runtime_metadata(
            &mut response,
            &auth.profile_name,
            value.get("model").and_then(Value::as_str),
            None,
        );
        if response.get("status").and_then(Value::as_str) != Some("failed")
            && let Some(response_id) = response.get("id").and_then(Value::as_str)
        {
            runtime_deepseek_store_conversation(
                &shared.deepseek_conversations,
                response_id,
                translated.messages,
                runtime_kiro_acp_chat_assistant_messages_from_prompt_turn(&turn),
            );
        }
        if stream {
            let response = RuntimeLocalRewriteUpstreamResponse::Streaming(
                RuntimeLocalRewriteStreamingResponse {
                    status: 200,
                    headers: vec![(
                        "content-type".to_string(),
                        "text/event-stream; charset=utf-8".to_string(),
                    )],
                    body: Box::new(runtime_kiro_streaming_reader(
                        request_id,
                        prompt,
                        prompt_messages,
                        auth,
                        value
                            .get("model")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string),
                        chat_completions_route,
                        shared,
                    )?),
                    profile_name: auth.profile_name.clone(),
                },
            );
            let response = if let Some(anthropic_request) = anthropic_request.as_ref() {
                runtime_kiro_anthropic_streaming_local_response(
                    response,
                    anthropic_request,
                    request_id,
                    &shared.runtime_shared,
                )?
            } else {
                response
            };
            Ok(RuntimeLocalRewriteUpstreamResult {
                response,
                gemini_context: None,
                copilot_context: None,
            })
        } else {
            let body = if chat_completions_route {
                serde_json::to_vec(&runtime_kiro_chat_completion_value_from_response(
                    &response, request_id,
                ))
                .context("failed to serialize Kiro chat completion JSON")?
            } else {
                serde_json::to_vec(&response).context("failed to serialize Kiro response JSON")?
            };
            let response = RuntimeLocalRewriteUpstreamResponse::Buffered(
                RuntimeHeapTrimmedBufferedResponseParts {
                    status: 200,
                    headers: vec![(
                        "content-type".to_string(),
                        b"application/json; charset=utf-8".to_vec(),
                    )],
                    body: body.into(),
                },
            );
            let response = if let Some(anthropic_request) = anthropic_request.as_ref() {
                RuntimeLocalRewriteUpstreamResponse::Buffered(
                    runtime_kiro_anthropic_message_parts_from_response(
                        &response,
                        anthropic_request,
                    ),
                )
            } else {
                response
            };
            Ok(RuntimeLocalRewriteUpstreamResult {
                response,
                gemini_context: None,
                copilot_context: None,
            })
        }
    })
}

use super::super::local_rewrite::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared,
};
use super::super::local_rewrite_copilot::RuntimeCopilotRequestContext;
use super::super::local_rewrite_gemini::RuntimeGeminiRequestContext;
use super::super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::super::local_rewrite_upstream::RuntimeLocalRewriteLiveResponse;
use super::local_rewrite_response_chat_compatible::{
    RuntimeChatCompatibleRewriteContext, respond_runtime_chat_compatible_rewrite,
};
use super::local_rewrite_response_copilot::respond_runtime_copilot_rewrite;
use super::local_rewrite_response_gemini::{
    RuntimeGeminiRewriteContext, respond_runtime_gemini_rewrite,
};
use super::local_rewrite_response_passthrough::respond_runtime_passthrough_rewrite;
use super::*;
use crate::runtime_launch::proxy_startup::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderRouteKind, runtime_provider_route_kind,
};
use runtime_proxy_crate::path_without_query;

pub(super) fn respond_runtime_local_rewrite_live_response(
    request_id: u64,
    request: RuntimeLocalRewriteRequest,
    live_response: RuntimeLocalRewriteLiveResponse,
    gemini_context: Option<RuntimeGeminiRequestContext>,
    copilot_context: Option<RuntimeCopilotRequestContext>,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let RuntimeLocalRewriteLiveResponse { prefix, response } = live_response;
    let status = response.status().as_u16();
    let headers = runtime_proxy_crate::runtime_forward_binary_response_headers(
        response
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_bytes())),
    );
    let text_headers = runtime_proxy_crate::runtime_forward_text_response_headers(
        response
            .headers()
            .iter()
            .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str(), value))),
    );
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if runtime_local_rewrite_is_responses_route(
        &shared.provider,
        &captured.path_and_query,
        RuntimeProviderBridgeKind::DeepSeek,
    ) && (200..300).contains(&status)
    {
        respond_runtime_chat_compatible_rewrite(
            request_id,
            request,
            response,
            RuntimeChatCompatibleRewriteContext {
                status,
                content_type: &content_type,
                shared,
                captured,
                provider_kind: RuntimeProviderBridgeKind::DeepSeek,
                profile_name: None,
                binding_recorder: None,
            },
        );
        return;
    }

    if runtime_local_rewrite_is_responses_route(
        &shared.provider,
        &captured.path_and_query,
        RuntimeProviderBridgeKind::Anthropic,
    ) && (200..300).contains(&status)
    {
        respond_runtime_chat_compatible_rewrite(
            request_id,
            request,
            response,
            RuntimeChatCompatibleRewriteContext {
                status,
                content_type: &content_type,
                shared,
                captured,
                provider_kind: RuntimeProviderBridgeKind::Anthropic,
                profile_name: None,
                binding_recorder: None,
            },
        );
        return;
    }

    if runtime_local_rewrite_is_responses_route(
        &shared.provider,
        &captured.path_and_query,
        RuntimeProviderBridgeKind::Gemini,
    ) && (200..300).contains(&status)
    {
        if gemini_context.is_none() {
            respond_runtime_chat_compatible_rewrite(
                request_id,
                request,
                response,
                RuntimeChatCompatibleRewriteContext {
                    status,
                    content_type: &content_type,
                    shared,
                    captured,
                    provider_kind: RuntimeProviderBridgeKind::Gemini,
                    profile_name: None,
                    binding_recorder: None,
                },
            );
        } else {
            respond_runtime_gemini_rewrite(
                request_id,
                request,
                response,
                RuntimeGeminiRewriteContext {
                    prefix,
                    status,
                    content_type: &content_type,
                    shared,
                    captured,
                    gemini_context,
                },
            );
        }
        return;
    }

    if runtime_local_rewrite_is_responses_route(
        &shared.provider,
        &captured.path_and_query,
        RuntimeProviderBridgeKind::Copilot,
    ) && (200..300).contains(&status)
    {
        respond_runtime_copilot_rewrite(
            request_id,
            request,
            response,
            status,
            &content_type,
            text_headers,
            headers,
            shared,
            captured,
            copilot_context,
        );
        return;
    }

    let is_sse = content_type.contains("text/event-stream");
    respond_runtime_passthrough_rewrite(
        request_id,
        request,
        response,
        status,
        text_headers,
        headers,
        shared,
        captured,
        gemini_context
            .as_ref()
            .map(|context| context.profile_name.clone())
            .or_else(|| {
                copilot_context
                    .as_ref()
                    .map(|context| context.profile_name.clone())
            })
            .unwrap_or_else(|| "local".to_string()),
        is_sse,
    );
}

fn runtime_local_rewrite_is_responses_route(
    provider: &RuntimeLocalRewriteProviderOptions,
    path_and_query: &str,
    provider_kind: RuntimeProviderBridgeKind,
) -> bool {
    provider.bridge_kind() == provider_kind
        && matches!(
            runtime_provider_route_kind(path_without_query(path_and_query)),
            Some(RuntimeProviderRouteKind::Responses)
        )
}

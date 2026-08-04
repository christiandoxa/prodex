use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::local_rewrite_copilot::{
    RuntimeCopilotRequestContext, RuntimeCopilotResponsesSseBindingReader,
};
use super::super::local_rewrite_gemini::RuntimeGeminiRequestContext;
use super::super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::super::local_rewrite_upstream::{
    RuntimeLocalRewriteLiveResponse, runtime_local_rewrite_remember_accepted_binding,
};
use super::local_rewrite_response_anthropic_messages::{
    RuntimeAnthropicMessagesRewriteContext, respond_runtime_anthropic_messages_rewrite,
};
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

#[allow(clippy::too_many_arguments)]
pub(super) fn respond_runtime_local_rewrite_live_response(
    request_id: u64,
    request: RuntimeLocalRewriteRequest,
    live_response: RuntimeLocalRewriteLiveResponse,
    gemini_context: Option<RuntimeGeminiRequestContext>,
    copilot_context: Option<RuntimeCopilotRequestContext>,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    response_governance: RuntimeGatewayResponseGovernance,
) {
    let RuntimeLocalRewriteLiveResponse {
        prefix,
        status,
        headers: upstream_headers,
        body,
        native_anthropic_messages,
        mut chat_compatible_request,
        accepted_binding_recorder,
        accepted_binding,
        ..
    } = live_response;
    let headers = runtime_proxy_crate::runtime_forward_binary_response_headers(
        upstream_headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_bytes())),
    );
    let text_headers = runtime_proxy_crate::runtime_forward_text_response_headers(
        upstream_headers
            .iter()
            .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str(), value))),
    );
    let content_type = upstream_headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut response = body
        .expect("live response body should be present before dispatch")
        .into_reader();
    if let Some(binding_acceptance) = copilot_context
        .as_ref()
        .and_then(|context| context.binding_acceptance.as_ref())
    {
        binding_acceptance();
    }
    if let Some(binding) = accepted_binding.as_ref() {
        let _ = runtime_local_rewrite_remember_accepted_binding(
            shared,
            &binding.identity,
            binding.previous_response_id.as_deref(),
            binding.turn_state.as_deref(),
            binding.session_id.as_deref(),
        );
    }
    let mut buffered_binding_recorder = None;
    if let Some(binding_recorder) = accepted_binding_recorder {
        if content_type.contains("text/event-stream") {
            response = Box::new(RuntimeCopilotResponsesSseBindingReader::new(
                response,
                Some(binding_recorder),
            ));
        } else {
            buffered_binding_recorder = Some(binding_recorder);
        }
    }

    let responses_provider = ((200..300).contains(&status)
        && matches!(
            runtime_provider_route_kind(path_without_query(&captured.path_and_query)),
            Some(RuntimeProviderRouteKind::Responses)
        ))
    .then(|| shared.provider.bridge_kind());
    match responses_provider {
        Some(
            provider_kind @ (RuntimeProviderBridgeKind::DeepSeek
            | RuntimeProviderBridgeKind::Anthropic),
        ) => {
            if native_anthropic_messages {
                respond_runtime_anthropic_messages_rewrite(
                    request_id,
                    request,
                    response,
                    RuntimeAnthropicMessagesRewriteContext {
                        status,
                        content_type: &content_type,
                        upstream_headers: upstream_headers.clone(),
                        shared,
                        captured,
                        provider_kind,
                        pending_request: chat_compatible_request.take().unwrap_or_default(),
                        binding_recorder: buffered_binding_recorder.take(),
                        response_governance,
                    },
                );
            } else {
                respond_runtime_chat_compatible_rewrite(
                    request_id,
                    request,
                    response,
                    RuntimeChatCompatibleRewriteContext {
                        status,
                        content_type: &content_type,
                        upstream_headers: upstream_headers.clone(),
                        prefix,
                        shared,
                        captured,
                        provider_kind,
                        profile_name: None,
                        binding_recorder: buffered_binding_recorder.take(),
                        pending_request: chat_compatible_request.take().unwrap_or_default(),
                        response_governance,
                    },
                );
            }
            return;
        }
        Some(RuntimeProviderBridgeKind::Gemini) => {
            if gemini_context.is_none() {
                respond_runtime_chat_compatible_rewrite(
                    request_id,
                    request,
                    response,
                    RuntimeChatCompatibleRewriteContext {
                        status,
                        content_type: &content_type,
                        upstream_headers: upstream_headers.clone(),
                        prefix,
                        shared,
                        captured,
                        provider_kind: RuntimeProviderBridgeKind::Gemini,
                        profile_name: None,
                        binding_recorder: buffered_binding_recorder.take(),
                        pending_request: chat_compatible_request.take().unwrap_or_default(),
                        response_governance,
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
                        upstream_headers: upstream_headers.clone(),
                        shared,
                        captured,
                        gemini_context,
                        external_binding_recorder: buffered_binding_recorder.take(),
                        response_governance,
                    },
                );
            }
            return;
        }
        Some(RuntimeProviderBridgeKind::Copilot) => {
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
                response_governance,
            );
            return;
        }
        _ => {}
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
        buffered_binding_recorder,
        response_governance,
    );
}

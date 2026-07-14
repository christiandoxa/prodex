use super::super::chat_compatible_rewrite::RuntimeChatCompatibleSseReader;
use super::super::deepseek_rewrite::{
    runtime_deepseek_chat_buffered_response_parts, runtime_deepseek_take_pending_messages,
};
use super::super::local_rewrite::{RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteProxyShared};
use super::super::local_rewrite_copilot::{
    RuntimeCopilotBindingRecorder, RuntimeCopilotResponsesSseBindingReader,
    runtime_copilot_remember_bindings_from_responses_body,
};
use super::super::local_rewrite_rate_limits::{
    append_binary_rate_limit_headers, append_text_rate_limit_headers,
    runtime_deepseek_codex_rate_limit_headers, runtime_provider_codex_rate_limit_headers,
};
use super::super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::super::local_rewrite_response_spend::{
    emit_runtime_gateway_response_spend_event_for_body, runtime_gateway_spend_stream_body,
};
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_log_stream_conformance,
    runtime_provider_stream_event_conformance_result,
};
use super::RuntimeGatewayResponseGovernance;
use super::respond_runtime_local_rewrite_stream;
use super::runtime_local_rewrite_append_call_id_header;
use super::runtime_local_rewrite_governed_response_with_call_id;
use super::runtime_local_rewrite_invalid_response;
use crate::{RuntimeProxyRequest, RuntimeStreamingResponse};
use std::io::Read;
use std::sync::Arc;
use std::time::Instant;

pub(super) struct RuntimeChatCompatibleRewriteContext<'a> {
    pub(super) status: u16,
    pub(super) content_type: &'a str,
    pub(super) shared: &'a RuntimeLocalRewriteProxyShared,
    pub(super) captured: &'a RuntimeProxyRequest,
    pub(super) provider_kind: RuntimeProviderBridgeKind,
    pub(super) profile_name: Option<String>,
    pub(super) binding_recorder: Option<RuntimeCopilotBindingRecorder>,
    pub(super) response_governance: RuntimeGatewayResponseGovernance,
}

pub(super) fn respond_runtime_chat_compatible_rewrite(
    request_id: u64,
    request: RuntimeLocalRewriteRequest,
    response: reqwest::blocking::Response,
    context: RuntimeChatCompatibleRewriteContext<'_>,
) {
    let RuntimeChatCompatibleRewriteContext {
        status,
        content_type,
        shared,
        captured,
        provider_kind,
        profile_name,
        binding_recorder,
        response_governance,
    } = context;
    let profile_name = profile_name.unwrap_or_else(|| RUNTIME_LOCAL_REWRITE_PROFILE.to_string());
    let rate_limit_headers = if provider_kind == RuntimeProviderBridgeKind::DeepSeek {
        runtime_deepseek_codex_rate_limit_headers(response.headers())
    } else {
        runtime_provider_codex_rate_limit_headers(provider_kind, response.headers())
    };
    let pending_request =
        runtime_deepseek_take_pending_messages(&shared.deepseek_pending_messages, request_id);
    let conversation_messages = pending_request.messages;
    let response_metadata = pending_request.response_metadata;
    if content_type.contains("text/event-stream") {
        let mut headers = vec![(
            "content-type".to_string(),
            "text/event-stream; charset=utf-8".to_string(),
        )];
        append_text_rate_limit_headers(&mut headers, rate_limit_headers);
        runtime_local_rewrite_append_call_id_header(&mut headers, request_id, shared);
        let observer = {
            let shared = shared.runtime_shared.clone();
            Arc::new(move |value: &serde_json::Value| {
                let body = format!("data: {}\n\n", value).into_bytes();
                let result = runtime_provider_stream_event_conformance_result(provider_kind, &body);
                runtime_provider_log_stream_conformance(
                    &shared,
                    request_id,
                    provider_kind,
                    &result,
                );
            })
        };
        let reader = RuntimeChatCompatibleSseReader::new_with_provider_and_observer(
            response,
            provider_kind,
            request_id,
            conversation_messages,
            response_metadata,
            shared.deepseek_conversations.clone(),
            Some(observer),
        );
        let body: Box<dyn Read + Send> = if let Some(binding_recorder) = binding_recorder {
            Box::new(RuntimeCopilotResponsesSseBindingReader::new(
                reader,
                Some(binding_recorder),
            ))
        } else {
            Box::new(reader)
        };
        let body = runtime_gateway_spend_stream_body(body, request_id, status, captured, shared);
        let streaming = RuntimeStreamingResponse {
            status,
            headers,
            body,
            request_id,
            profile_name,
            log_path: shared.runtime_shared.log_path.clone(),
            shared: shared.runtime_shared.clone(),
            _inflight_guard: None,
        };
        respond_runtime_local_rewrite_stream(request, streaming, shared, response_governance);
        return;
    }

    let response_started_at = Instant::now();
    let response = runtime_deepseek_chat_buffered_response_parts(
        provider_kind,
        status,
        response,
        request_id,
        conversation_messages,
        response_metadata,
        &shared.deepseek_conversations,
        &shared.runtime_shared,
    )
    .map(|mut parts| {
        runtime_copilot_remember_bindings_from_responses_body(
            binding_recorder.as_ref(),
            &parts.body,
        );
        append_binary_rate_limit_headers(&mut parts.headers, rate_limit_headers);
        emit_runtime_gateway_response_spend_event_for_body(
            request_id,
            captured,
            shared,
            parts.status,
            response_started_at.elapsed().as_millis(),
            parts.body.as_slice(),
        );
        runtime_local_rewrite_governed_response_with_call_id(
            parts,
            request_id,
            shared,
            response_governance,
        )
    })
    .unwrap_or_else(|err| runtime_local_rewrite_invalid_response(request_id, shared, &err));
    let _ = request.respond(response);
}

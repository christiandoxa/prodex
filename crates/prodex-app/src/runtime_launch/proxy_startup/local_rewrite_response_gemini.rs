use super::super::gemini_rewrite::runtime_gemini_generate_buffered_response_parts;
use super::super::gemini_sse::RuntimeGeminiGenerateSseReader;
use super::super::local_rewrite::{RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteProxyShared};
use super::super::local_rewrite_gemini::{
    RuntimeGeminiRequestContext, runtime_gemini_remember_bindings_from_responses_body,
};
use super::super::local_rewrite_rate_limits::{
    append_binary_rate_limit_headers, append_text_rate_limit_headers,
    runtime_provider_codex_rate_limit_headers,
};
use super::super::local_rewrite_response_guardrails::runtime_gateway_guardrail_stream_body;
use super::super::local_rewrite_response_spend::{
    emit_runtime_gateway_response_spend_event_for_body, runtime_gateway_spend_stream_body,
};
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_log_stream_conformance,
    runtime_provider_stream_event_conformance_result,
};
use super::runtime_local_rewrite_append_call_id_header;
use super::runtime_local_rewrite_buffered_response_parts;
use super::runtime_local_rewrite_response_with_call_id;
use crate::{
    RuntimeProxyRequest, RuntimeStreamingResponse, build_runtime_proxy_text_response,
    write_runtime_streaming_response,
};
use std::io::Read;
use std::sync::Arc;
use std::time::Instant;

pub(super) struct RuntimeGeminiRewriteContext<'a> {
    pub(super) prefix: Vec<u8>,
    pub(super) status: u16,
    pub(super) content_type: &'a str,
    pub(super) shared: &'a RuntimeLocalRewriteProxyShared,
    pub(super) captured: &'a RuntimeProxyRequest,
    pub(super) gemini_context: Option<RuntimeGeminiRequestContext>,
}

pub(super) fn respond_runtime_gemini_rewrite(
    request_id: u64,
    request: tiny_http::Request,
    response: reqwest::blocking::Response,
    context: RuntimeGeminiRewriteContext<'_>,
) {
    let RuntimeGeminiRewriteContext {
        prefix,
        status,
        content_type,
        shared,
        captured,
        gemini_context,
    } = context;
    let RuntimeGeminiRequestContext {
        profile_name,
        conversation_messages,
        binding_recorder,
    } = gemini_context.unwrap_or_else(|| RuntimeGeminiRequestContext {
        profile_name: RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
        conversation_messages: Vec::new(),
        binding_recorder: None,
    });
    let mut rate_limit_headers = shared
        .gemini_oauth_pool
        .as_ref()
        .map(|pool| pool.quota_headers_for_profile(&profile_name))
        .unwrap_or_default();
    if rate_limit_headers.is_empty() {
        rate_limit_headers = runtime_provider_codex_rate_limit_headers(
            RuntimeProviderBridgeKind::Gemini,
            response.headers(),
        );
    }
    if content_type.contains("text/event-stream") {
        let writer = request.into_writer();
        let mut headers = vec![(
            "content-type".to_string(),
            "text/event-stream; charset=utf-8".to_string(),
        )];
        append_text_rate_limit_headers(&mut headers, rate_limit_headers);
        headers.push(("x-reasoning-included".to_string(), "true".to_string()));
        runtime_local_rewrite_append_call_id_header(&mut headers, request_id, shared);
        let body: Box<dyn Read + Send> = if prefix.is_empty() {
            Box::new(response)
        } else {
            Box::new(std::io::Cursor::new(prefix).chain(response))
        };
        let observer = {
            let shared = shared.runtime_shared.clone();
            Arc::new(move |value: &serde_json::Value| {
                let body = format!("data: {}\n\n", value).into_bytes();
                let result = runtime_provider_stream_event_conformance_result(
                    RuntimeProviderBridgeKind::Gemini,
                    &body,
                );
                runtime_provider_log_stream_conformance(
                    &shared,
                    request_id,
                    RuntimeProviderBridgeKind::Gemini,
                    &result,
                );
            })
        };
        let body: Box<dyn Read + Send> =
            Box::new(RuntimeGeminiGenerateSseReader::new_with_observer(
                body,
                request_id,
                conversation_messages,
                shared.gemini_conversations.clone(),
                binding_recorder,
                Some(observer),
            ));
        let body = runtime_gateway_spend_stream_body(
            runtime_gateway_guardrail_stream_body(body, request_id, shared),
            request_id,
            status,
            captured,
            shared,
        );
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
        let _ = write_runtime_streaming_response(writer, streaming);
        return;
    }

    let response_started_at = Instant::now();
    let response = runtime_gemini_generate_buffered_response_parts(
        status,
        response,
        request_id,
        conversation_messages,
        &shared.gemini_conversations,
        &shared.runtime_shared,
    )
    .map(|mut parts| {
        runtime_gemini_remember_bindings_from_responses_body(
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
        runtime_local_rewrite_response_with_call_id(parts, request_id, shared)
    })
    .unwrap_or_else(|err| build_runtime_proxy_text_response(502, &err.to_string()));
    let _ = request.respond(response);
}

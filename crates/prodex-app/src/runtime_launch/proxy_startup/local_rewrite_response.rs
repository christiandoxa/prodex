use super::deepseek_rewrite::{
    RuntimeDeepSeekChatSseReader, runtime_deepseek_chat_buffered_response_parts,
    runtime_deepseek_take_pending_messages,
};
use super::gemini_rewrite::runtime_gemini_generate_buffered_response_parts;
use super::gemini_sse::RuntimeGeminiGenerateSseReader;
use super::local_rewrite::{
    RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult,
};
use super::local_rewrite_gemini::{
    RuntimeGeminiRequestContext, runtime_gemini_remember_bindings_from_responses_body,
};
use crate::{
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, RuntimeStreamingResponse,
    build_runtime_proxy_response_from_parts, build_runtime_proxy_text_response,
    write_runtime_streaming_response,
};
use anyhow::{Context, Result};
use runtime_proxy_crate::path_without_query;
use std::io::Read;
use std::sync::Arc;

pub(super) fn runtime_local_rewrite_buffered_response_from_response(
    response: reqwest::blocking::Response,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let status = response.status().as_u16();
    let headers = runtime_proxy_crate::runtime_forward_binary_response_headers(
        response
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_bytes())),
    );
    runtime_local_rewrite_buffered_response_parts(status, headers, response)
}

pub(super) fn respond_runtime_local_rewrite_proxy_request(
    request_id: u64,
    request: tiny_http::Request,
    response: RuntimeLocalRewriteUpstreamResult,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let RuntimeLocalRewriteUpstreamResult {
        response,
        gemini_context,
    } = response;
    let response = match response {
        RuntimeLocalRewriteUpstreamResponse::Live(response) => response,
        RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => {
            let _ = request.respond(build_runtime_proxy_response_from_parts(parts));
            return;
        }
    };
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
    let deepseek_responses_route = matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::DeepSeek { .. }
    ) && path_without_query(&captured.path_and_query)
        .ends_with("/responses");
    let gemini_responses_route = matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::Gemini { .. }
    ) && path_without_query(&captured.path_and_query)
        .ends_with("/responses");
    if deepseek_responses_route && (200..300).contains(&status) {
        respond_runtime_deepseek_rewrite(
            request_id,
            request,
            response,
            status,
            &content_type,
            shared,
        );
        return;
    }

    if gemini_responses_route && (200..300).contains(&status) {
        respond_runtime_gemini_rewrite(
            request_id,
            request,
            response,
            status,
            &content_type,
            shared,
            gemini_context,
        );
        return;
    }

    if content_type.contains("text/event-stream") {
        let writer = request.into_writer();
        let streaming = RuntimeStreamingResponse {
            status,
            headers: text_headers,
            body: Box::new(response),
            request_id,
            profile_name: RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
            log_path: shared.runtime_shared.log_path.clone(),
            shared: shared.runtime_shared.clone(),
            _inflight_guard: None,
        };
        let _ = write_runtime_streaming_response(writer, streaming);
        return;
    }

    let response = runtime_local_rewrite_buffered_response_parts(status, headers, response)
        .map(build_runtime_proxy_response_from_parts)
        .unwrap_or_else(|err| build_runtime_proxy_text_response(502, &err.to_string()));
    let _ = request.respond(response);
}

fn respond_runtime_deepseek_rewrite(
    request_id: u64,
    request: tiny_http::Request,
    response: reqwest::blocking::Response,
    status: u16,
    content_type: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let conversation_messages =
        runtime_deepseek_take_pending_messages(&shared.deepseek_pending_messages, request_id);
    if content_type.contains("text/event-stream") {
        let writer = request.into_writer();
        let streaming = RuntimeStreamingResponse {
            status,
            headers: vec![(
                "content-type".to_string(),
                "text/event-stream; charset=utf-8".to_string(),
            )],
            body: Box::new(RuntimeDeepSeekChatSseReader::new(
                response,
                request_id,
                conversation_messages,
                Arc::clone(&shared.deepseek_conversations),
            )),
            request_id,
            profile_name: RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
            log_path: shared.runtime_shared.log_path.clone(),
            shared: shared.runtime_shared.clone(),
            _inflight_guard: None,
        };
        let _ = write_runtime_streaming_response(writer, streaming);
        return;
    }

    let response = runtime_deepseek_chat_buffered_response_parts(
        status,
        response,
        request_id,
        conversation_messages,
        &shared.deepseek_conversations,
    )
    .map(build_runtime_proxy_response_from_parts)
    .unwrap_or_else(|err| build_runtime_proxy_text_response(502, &err.to_string()));
    let _ = request.respond(response);
}

fn respond_runtime_gemini_rewrite(
    request_id: u64,
    request: tiny_http::Request,
    response: reqwest::blocking::Response,
    status: u16,
    content_type: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    gemini_context: Option<RuntimeGeminiRequestContext>,
) {
    let RuntimeGeminiRequestContext {
        profile_name,
        conversation_messages,
        binding_recorder,
    } = gemini_context.unwrap_or_else(|| RuntimeGeminiRequestContext {
        profile_name: RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
        conversation_messages: Vec::new(),
        binding_recorder: None,
    });
    if content_type.contains("text/event-stream") {
        let writer = request.into_writer();
        let streaming = RuntimeStreamingResponse {
            status,
            headers: vec![(
                "content-type".to_string(),
                "text/event-stream; charset=utf-8".to_string(),
            )],
            body: Box::new(RuntimeGeminiGenerateSseReader::new(
                response,
                request_id,
                conversation_messages,
                Arc::clone(&shared.gemini_conversations),
                binding_recorder,
            )),
            request_id,
            profile_name,
            log_path: shared.runtime_shared.log_path.clone(),
            shared: shared.runtime_shared.clone(),
            _inflight_guard: None,
        };
        let _ = write_runtime_streaming_response(writer, streaming);
        return;
    }

    let response = runtime_gemini_generate_buffered_response_parts(
        status,
        response,
        request_id,
        conversation_messages,
        &shared.gemini_conversations,
    )
    .map(|parts| {
        runtime_gemini_remember_bindings_from_responses_body(
            binding_recorder.as_ref(),
            &parts.body,
        );
        build_runtime_proxy_response_from_parts(parts)
    })
    .unwrap_or_else(|err| build_runtime_proxy_text_response(502, &err.to_string()));
    let _ = request.respond(response);
}

fn runtime_local_rewrite_buffered_response_parts(
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    mut response: reqwest::blocking::Response,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let mut body = Vec::new();
    response
        .read_to_end(&mut body)
        .context("failed to read local provider response body")?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers,
        body: body.into(),
    })
}

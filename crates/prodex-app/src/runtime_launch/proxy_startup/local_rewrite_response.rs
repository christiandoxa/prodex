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
use super::local_rewrite_copilot::{
    RuntimeCopilotBindingRecorder, RuntimeCopilotResponsesSseBindingReader,
    runtime_copilot_remember_bindings_from_responses_body,
};
use super::local_rewrite_gemini::{
    RuntimeGeminiRequestContext, runtime_gemini_remember_bindings_from_responses_body,
};
use super::local_rewrite_rate_limits::{
    append_binary_rate_limit_headers, append_text_rate_limit_headers,
    runtime_deepseek_codex_rate_limit_headers, runtime_openai_style_codex_rate_limit_headers,
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

#[derive(Clone, Copy)]
struct RuntimeChatCompatibleProvider {
    prefix: &'static str,
    label: &'static str,
}

struct RuntimeChatCompatibleRewriteContext<'a> {
    status: u16,
    content_type: &'a str,
    shared: &'a RuntimeLocalRewriteProxyShared,
    provider: RuntimeChatCompatibleProvider,
    profile_name: Option<String>,
    binding_recorder: Option<RuntimeCopilotBindingRecorder>,
}

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
        copilot_context,
    } = response;
    let provider_profile_name = gemini_context
        .as_ref()
        .map(|context| context.profile_name.clone())
        .or_else(|| {
            copilot_context
                .as_ref()
                .map(|context| context.profile_name.clone())
        })
        .unwrap_or_else(|| RUNTIME_LOCAL_REWRITE_PROFILE.to_string());
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
    let anthropic_responses_route = matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::Anthropic { .. }
    ) && path_without_query(&captured.path_and_query)
        .ends_with("/responses");
    let gemini_responses_route = matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::Gemini { .. }
    ) && path_without_query(&captured.path_and_query)
        .ends_with("/responses");
    let copilot_responses_route = matches!(
        &shared.provider,
        RuntimeLocalRewriteProviderOptions::Copilot { .. }
    ) && path_without_query(&captured.path_and_query)
        .ends_with("/responses");
    if deepseek_responses_route && (200..300).contains(&status) {
        respond_runtime_chat_compatible_rewrite(
            request_id,
            request,
            response,
            RuntimeChatCompatibleRewriteContext {
                status,
                content_type: &content_type,
                shared,
                provider: RuntimeChatCompatibleProvider {
                    prefix: "deepseek",
                    label: "DeepSeek",
                },
                profile_name: None,
                binding_recorder: None,
            },
        );
        return;
    }

    if anthropic_responses_route && (200..300).contains(&status) {
        respond_runtime_chat_compatible_rewrite(
            request_id,
            request,
            response,
            RuntimeChatCompatibleRewriteContext {
                status,
                content_type: &content_type,
                shared,
                provider: RuntimeChatCompatibleProvider {
                    prefix: "anthropic",
                    label: "Anthropic",
                },
                profile_name: None,
                binding_recorder: None,
            },
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

    if copilot_responses_route && (200..300).contains(&status) {
        let profile_name = copilot_context
            .as_ref()
            .map(|context| context.profile_name.clone());
        let binding_recorder = copilot_context
            .as_ref()
            .and_then(|context| context.binding_recorder.clone());
        respond_runtime_chat_compatible_rewrite(
            request_id,
            request,
            response,
            RuntimeChatCompatibleRewriteContext {
                status,
                content_type: &content_type,
                shared,
                provider: RuntimeChatCompatibleProvider {
                    prefix: "copilot",
                    label: "GitHub Copilot",
                },
                profile_name,
                binding_recorder,
            },
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
            profile_name: provider_profile_name,
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

fn respond_runtime_chat_compatible_rewrite(
    request_id: u64,
    request: tiny_http::Request,
    response: reqwest::blocking::Response,
    context: RuntimeChatCompatibleRewriteContext<'_>,
) {
    let RuntimeChatCompatibleRewriteContext {
        status,
        content_type,
        shared,
        provider,
        profile_name,
        binding_recorder,
    } = context;
    let profile_name = profile_name.unwrap_or_else(|| RUNTIME_LOCAL_REWRITE_PROFILE.to_string());
    let rate_limit_headers = if provider.prefix == "deepseek" {
        runtime_deepseek_codex_rate_limit_headers(response.headers())
    } else {
        runtime_openai_style_codex_rate_limit_headers(
            response.headers(),
            provider.prefix,
            provider.label,
        )
    };
    let conversation_messages =
        runtime_deepseek_take_pending_messages(&shared.deepseek_pending_messages, request_id);
    if content_type.contains("text/event-stream") {
        let writer = request.into_writer();
        let mut headers = vec![(
            "content-type".to_string(),
            "text/event-stream; charset=utf-8".to_string(),
        )];
        append_text_rate_limit_headers(&mut headers, rate_limit_headers);
        let reader = RuntimeDeepSeekChatSseReader::new(
            response,
            request_id,
            conversation_messages,
            Arc::clone(&shared.deepseek_conversations),
        );
        let body: Box<dyn Read + Send> = if let Some(binding_recorder) = binding_recorder {
            Box::new(RuntimeCopilotResponsesSseBindingReader::new(
                reader,
                Some(binding_recorder),
            ))
        } else {
            Box::new(reader)
        };
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

    let response = runtime_deepseek_chat_buffered_response_parts(
        status,
        response,
        request_id,
        conversation_messages,
        &shared.deepseek_conversations,
    )
    .map(|mut parts| {
        runtime_copilot_remember_bindings_from_responses_body(
            binding_recorder.as_ref(),
            &parts.body,
        );
        append_binary_rate_limit_headers(&mut parts.headers, rate_limit_headers);
        build_runtime_proxy_response_from_parts(parts)
    })
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
    let mut rate_limit_headers = shared
        .gemini_oauth_pool
        .as_ref()
        .map(|pool| pool.quota_headers_for_profile(&profile_name))
        .unwrap_or_default();
    if rate_limit_headers.is_empty() {
        rate_limit_headers =
            runtime_openai_style_codex_rate_limit_headers(response.headers(), "gemini", "Gemini");
    }
    if content_type.contains("text/event-stream") {
        let writer = request.into_writer();
        let mut headers = vec![(
            "content-type".to_string(),
            "text/event-stream; charset=utf-8".to_string(),
        )];
        append_text_rate_limit_headers(&mut headers, rate_limit_headers);
        let streaming = RuntimeStreamingResponse {
            status,
            headers,
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
    .map(|mut parts| {
        runtime_gemini_remember_bindings_from_responses_body(
            binding_recorder.as_ref(),
            &parts.body,
        );
        append_binary_rate_limit_headers(&mut parts.headers, rate_limit_headers);
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

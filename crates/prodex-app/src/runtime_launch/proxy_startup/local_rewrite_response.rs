use super::deepseek_rewrite::{
    RuntimeDeepSeekChatSseReader, runtime_deepseek_chat_buffered_response_parts,
    runtime_deepseek_take_pending_messages,
};
use super::gemini_rewrite::runtime_gemini_generate_buffered_response_parts;
use super::gemini_sse::RuntimeGeminiGenerateSseReader;
use super::local_rewrite::{
    RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, runtime_gateway_guardrail_webhook_block,
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
use super::local_rewrite_transport::emit_runtime_gateway_spend_event;
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_gateway_cost_for_request,
    runtime_provider_gateway_response_spend_event,
    runtime_provider_gateway_response_spend_event_from_tokens, runtime_provider_model_from_body,
};
use crate::{
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, RuntimeRotationProxyShared,
    RuntimeStreamingResponse, build_runtime_proxy_json_error_response,
    build_runtime_proxy_response_from_parts, build_runtime_proxy_text_response,
    write_runtime_streaming_response,
};
use anyhow::{Context, Result};
use prodex_provider_core::{ProviderModelCost, estimate_text_tokens, extract_usage_tokens};
use runtime_proxy_crate::path_without_query;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io::Read;
use std::sync::Arc;
use std::time::Instant;

const RUNTIME_GATEWAY_SPEND_STREAM_CAPTURE_MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy)]
struct RuntimeChatCompatibleProvider {
    prefix: &'static str,
    label: &'static str,
}

struct RuntimeChatCompatibleRewriteContext<'a> {
    status: u16,
    content_type: &'a str,
    shared: &'a RuntimeLocalRewriteProxyShared,
    captured: &'a RuntimeProxyRequest,
    provider: RuntimeChatCompatibleProvider,
    profile_name: Option<String>,
    binding_recorder: Option<RuntimeCopilotBindingRecorder>,
}

struct RuntimeGeminiRewriteContext<'a> {
    prefix: Vec<u8>,
    status: u16,
    content_type: &'a str,
    shared: &'a RuntimeLocalRewriteProxyShared,
    captured: &'a RuntimeProxyRequest,
    gemini_context: Option<RuntimeGeminiRequestContext>,
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

pub(super) fn runtime_local_rewrite_response_with_call_id(
    mut parts: RuntimeHeapTrimmedBufferedResponseParts,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
) -> tiny_http::ResponseBox {
    if (200..300).contains(&parts.status)
        && let Some(block) = runtime_proxy_crate::runtime_gateway_response_guardrail_block(
            &parts.body,
            &shared.gateway_guardrails,
        )
    {
        crate::runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_response_blocked",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("reason", block.kind.as_str()),
                    runtime_proxy_log_field("value", block.value.as_str()),
                ],
            ),
        );
        return build_runtime_proxy_json_error_response(
            403,
            "policy_violation",
            "gateway guardrail blocked this response",
        );
    }
    if (200..300).contains(&parts.status)
        && let Some(block) =
            runtime_gateway_guardrail_webhook_block("post", request_id, &parts.body, shared)
    {
        crate::runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_webhook_blocked",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("phase", "post"),
                    runtime_proxy_log_field("reason", block.reason.as_str()),
                    runtime_proxy_log_field("value", block.value.as_str()),
                ],
            ),
        );
        return build_runtime_proxy_json_error_response(
            403,
            "policy_violation",
            "gateway guardrail webhook blocked this response",
        );
    }
    if let Some(header_name) = shared.gateway_call_id_header.as_deref() {
        parts.headers.push((
            header_name.to_string(),
            runtime_local_rewrite_call_id(request_id).into_bytes(),
        ));
    }
    build_runtime_proxy_response_from_parts(parts)
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
    let live_response = match response {
        RuntimeLocalRewriteUpstreamResponse::Live(live_response) => live_response,
        RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => {
            emit_runtime_gateway_response_spend_event_for_body(
                request_id,
                captured,
                shared,
                parts.status,
                0,
                parts.body.as_slice(),
            );
            let _ = request.respond(runtime_local_rewrite_response_with_call_id(
                parts, request_id, shared,
            ));
            return;
        }
    };
    let prefix = live_response.prefix;
    let response = live_response.response;
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
                captured,
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
                captured,
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
                    provider: RuntimeChatCompatibleProvider {
                        prefix: "gemini",
                        label: "Google Gemini",
                    },
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
                captured,
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
        let mut headers = text_headers;
        runtime_local_rewrite_append_call_id_header(&mut headers, request_id, shared);
        let body = runtime_gateway_spend_stream_body(
            runtime_gateway_guardrail_stream_body(Box::new(response), request_id, shared),
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
            profile_name: provider_profile_name,
            log_path: shared.runtime_shared.log_path.clone(),
            shared: shared.runtime_shared.clone(),
            _inflight_guard: None,
        };
        let _ = write_runtime_streaming_response(writer, streaming);
        return;
    }

    let response_started_at = Instant::now();
    let response = runtime_local_rewrite_buffered_response_parts(status, headers, response)
        .map(|parts| {
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

fn runtime_local_rewrite_call_id(request_id: u64) -> String {
    format!("prodex-{request_id}")
}

fn runtime_local_rewrite_append_call_id_header(
    headers: &mut Vec<(String, String)>,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    if let Some(header_name) = shared.gateway_call_id_header.as_deref() {
        headers.push((
            header_name.to_string(),
            runtime_local_rewrite_call_id(request_id),
        ));
    }
}

fn emit_runtime_gateway_response_spend_event_for_body(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    status: u16,
    elapsed_ms: u128,
    response_body: &[u8],
) {
    let provider_kind = shared.provider.bridge_kind();
    let model = runtime_provider_model_from_body(&captured.body);
    let cost = runtime_gateway_response_cost(
        request_id,
        provider_kind,
        captured,
        shared,
        model.as_deref(),
    );
    emit_runtime_gateway_spend_event(
        shared,
        runtime_provider_gateway_response_spend_event(
            request_id,
            provider_kind,
            &captured.path_and_query,
            model.as_deref(),
            status,
            elapsed_ms,
            &captured.body,
            response_body,
            cost,
        ),
    );
}

fn runtime_gateway_response_cost(
    request_id: u64,
    provider_kind: RuntimeProviderBridgeKind,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    model: Option<&str>,
) -> ProviderModelCost {
    let route_load = shared
        .gateway_route_load
        .lock()
        .map(|load| load.clone())
        .unwrap_or_default();
    runtime_provider_gateway_cost_for_request(
        provider_kind,
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &captured.body,
        model.unwrap_or("unknown"),
    )
}

fn runtime_gateway_spend_stream_body(
    body: Box<dyn Read + Send>,
    request_id: u64,
    status: u16,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Box<dyn Read + Send> {
    let provider_kind = shared.provider.bridge_kind();
    let model = runtime_provider_model_from_body(&captured.body);
    let cost = runtime_gateway_response_cost(
        request_id,
        provider_kind,
        captured,
        shared,
        model.as_deref(),
    );
    Box::new(RuntimeGatewaySpendStreamReader {
        inner: body,
        shared: shared.clone(),
        request_id,
        provider_kind,
        path_and_query: captured.path_and_query.clone(),
        model,
        status,
        started_at: Instant::now(),
        request_body: captured.body.clone(),
        response_bytes: 0,
        estimated_output_tokens: 0,
        captured_response: Vec::new(),
        captured_complete: true,
        cost,
        emitted: false,
    })
}

struct RuntimeGatewaySpendStreamReader {
    inner: Box<dyn Read + Send>,
    shared: RuntimeLocalRewriteProxyShared,
    request_id: u64,
    provider_kind: RuntimeProviderBridgeKind,
    path_and_query: String,
    model: Option<String>,
    status: u16,
    started_at: Instant,
    request_body: Vec<u8>,
    response_bytes: usize,
    estimated_output_tokens: u64,
    captured_response: Vec<u8>,
    captured_complete: bool,
    cost: ProviderModelCost,
    emitted: bool,
}

impl RuntimeGatewaySpendStreamReader {
    fn observe_chunk(&mut self, chunk: &[u8]) {
        self.response_bytes = self.response_bytes.saturating_add(chunk.len());
        self.estimated_output_tokens = self
            .estimated_output_tokens
            .saturating_add(estimate_text_tokens(&String::from_utf8_lossy(chunk)));
        if self.captured_response.len() < RUNTIME_GATEWAY_SPEND_STREAM_CAPTURE_MAX_BYTES {
            let remaining =
                RUNTIME_GATEWAY_SPEND_STREAM_CAPTURE_MAX_BYTES - self.captured_response.len();
            let take = remaining.min(chunk.len());
            self.captured_response.extend_from_slice(&chunk[..take]);
            if take < chunk.len() {
                self.captured_complete = false;
            }
        } else if !chunk.is_empty() {
            self.captured_complete = false;
        }
    }

    fn emit_once(&mut self) {
        if self.emitted {
            return;
        }
        self.emitted = true;
        let usage = if self.captured_complete {
            extract_usage_tokens(&self.captured_response)
        } else {
            prodex_provider_core::ProviderTokenUsage::default()
        };
        let observed_output_tokens = usage
            .output_tokens
            .or_else(|| (self.estimated_output_tokens > 0).then_some(self.estimated_output_tokens));
        emit_runtime_gateway_spend_event(
            &self.shared,
            runtime_provider_gateway_response_spend_event_from_tokens(
                self.request_id,
                self.provider_kind,
                &self.path_and_query,
                self.model.as_deref(),
                self.status,
                self.started_at.elapsed().as_millis(),
                &self.request_body,
                self.response_bytes,
                usage.input_tokens,
                observed_output_tokens,
                self.cost,
            ),
        );
    }
}

impl Read for RuntimeGatewaySpendStreamReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.inner.read(buf) {
            Ok(0) => {
                self.emit_once();
                Ok(0)
            }
            Ok(read) => {
                self.observe_chunk(&buf[..read]);
                Ok(read)
            }
            Err(err) => {
                self.emit_once();
                Err(err)
            }
        }
    }
}

impl Drop for RuntimeGatewaySpendStreamReader {
    fn drop(&mut self) {
        self.emit_once();
    }
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
        captured,
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
        runtime_local_rewrite_append_call_id_header(&mut headers, request_id, shared);
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

fn respond_runtime_gemini_rewrite(
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
        headers.push(("x-reasoning-included".to_string(), "true".to_string()));
        runtime_local_rewrite_append_call_id_header(&mut headers, request_id, shared);
        let body: Box<dyn Read + Send> = if prefix.is_empty() {
            Box::new(response)
        } else {
            Box::new(std::io::Cursor::new(prefix).chain(response))
        };
        let body: Box<dyn Read + Send> = Box::new(RuntimeGeminiGenerateSseReader::new(
            body,
            request_id,
            conversation_messages,
            Arc::clone(&shared.gemini_conversations),
            binding_recorder,
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

fn runtime_gateway_guardrail_stream_body(
    body: Box<dyn Read + Send>,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Box<dyn Read + Send> {
    let keywords = shared
        .gateway_guardrails
        .blocked_output_keywords
        .iter()
        .map(|keyword| keyword.trim().to_ascii_lowercase())
        .filter(|keyword| !keyword.is_empty())
        .collect::<Vec<_>>();
    if keywords.is_empty() {
        return body;
    }
    Box::new(RuntimeGatewayGuardrailStreamReader {
        inner: body,
        request_id,
        runtime_shared: shared.runtime_shared.clone(),
        keywords,
        tail: String::new(),
        blocked: false,
    })
}

struct RuntimeGatewayGuardrailStreamReader {
    inner: Box<dyn Read + Send>,
    request_id: u64,
    runtime_shared: RuntimeRotationProxyShared,
    keywords: Vec<String>,
    tail: String,
    blocked: bool,
}

impl Read for RuntimeGatewayGuardrailStreamReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.blocked {
            return Ok(0);
        }
        let read = self.inner.read(buf)?;
        if read == 0 {
            return Ok(0);
        }
        let text = String::from_utf8_lossy(&buf[..read]).to_ascii_lowercase();
        let combined = format!("{}{}", self.tail, text);
        if let Some(keyword) = self
            .keywords
            .iter()
            .find(|keyword| combined.contains(keyword.as_str()))
        {
            self.blocked = true;
            crate::runtime_proxy_log(
                &self.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_guardrail_stream_blocked",
                    [
                        runtime_proxy_log_field("request", self.request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("reason", "blocked_output_keyword"),
                        runtime_proxy_log_field("value", keyword.as_str()),
                    ],
                ),
            );
            return Ok(0);
        }
        let keep = self
            .keywords
            .iter()
            .map(|keyword| keyword.len().saturating_sub(1))
            .max()
            .unwrap_or_default()
            .min(256);
        self.tail = combined
            .chars()
            .rev()
            .take(keep)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        Ok(read)
    }
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

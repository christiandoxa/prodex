use self::dispatch::respond_runtime_local_rewrite_live_response;
use super::local_rewrite::{
    RuntimeLocalRewriteAsyncResponse, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
};
use super::local_rewrite_copilot::RuntimeCopilotResponsesSseBindingReader;
use super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::local_rewrite_upstream::runtime_local_rewrite_remember_accepted_binding;
use crate::{
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, RuntimeHeapTrimmedBufferedResponseParts,
    RuntimeProxyRequest, RuntimeStreamingResponse, build_runtime_proxy_response_from_parts,
    build_runtime_proxy_text_response, read_blocking_response_body_with_limit, runtime_proxy_log,
};
use anyhow::Result;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io;
use std::time::Duration;

#[path = "local_rewrite_response_dispatch.rs"]
mod dispatch;
#[path = "local_rewrite_response_anthropic_messages.rs"]
mod local_rewrite_response_anthropic_messages;
#[path = "local_rewrite_response_chat_compatible.rs"]
mod local_rewrite_response_chat_compatible;
#[path = "local_rewrite_response_copilot.rs"]
mod local_rewrite_response_copilot;
#[path = "local_rewrite_response_gemini.rs"]
mod local_rewrite_response_gemini;
#[path = "local_rewrite_response_passthrough.rs"]
mod local_rewrite_response_passthrough;

#[derive(Clone, Copy, Default)]
pub(super) struct RuntimeGatewayResponseGovernance;

fn runtime_local_rewrite_invalid_response(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    _error: &dyn std::fmt::Display,
) -> tiny_http::ResponseBox {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "provider_response_translation_failed",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("error", "translation_failed"),
            ],
        ),
    );
    build_runtime_proxy_text_response(502, "provider response could not be processed")
}

pub(super) fn runtime_local_rewrite_buffered_response_from_response(
    response: RuntimeLocalRewriteAsyncResponse,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let status = response.status().as_u16();
    let headers = runtime_proxy_crate::runtime_forward_binary_response_headers(
        response
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_bytes())),
    );
    let RuntimeLocalRewriteAsyncResponse {
        response,
        async_runtime,
        stream_idle_timeout_ms,
        ..
    } = response;
    let mut response = response
        .ok_or_else(|| anyhow::anyhow!("runtime upstream response body was already handed off"))?;
    let body = async_runtime.block_on(async move {
        let mut body = Vec::new();
        let timeout = Duration::from_millis(stream_idle_timeout_ms.max(1));
        loop {
            let next = tokio::time::timeout(timeout, response.chunk())
                .await
                .map_err(|_| {
                    anyhow::Error::new(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "runtime upstream stream idle timed out",
                    ))
                })??;
            let Some(chunk) = next else {
                break;
            };
            if chunk.len() > RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES.saturating_sub(body.len()) {
                return Err(anyhow::Error::new(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "buffered response exceeded safe size limit ({})",
                        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES
                    ),
                )));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    })?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers,
        body: body.into(),
    })
}

pub(super) fn runtime_local_rewrite_response_with_call_id(
    parts: RuntimeHeapTrimmedBufferedResponseParts,
    _request_id: u64,
    _shared: &RuntimeLocalRewriteProxyShared,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(parts)
}

pub(super) fn runtime_local_rewrite_governed_response_with_call_id(
    parts: RuntimeHeapTrimmedBufferedResponseParts,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    _governance: RuntimeGatewayResponseGovernance,
) -> tiny_http::ResponseBox {
    runtime_local_rewrite_response_with_call_id(parts, request_id, shared)
}

pub(super) fn respond_runtime_local_rewrite_stream(
    request: RuntimeLocalRewriteRequest,
    streaming: RuntimeStreamingResponse,
    _captured: &RuntimeProxyRequest,
    _shared: &RuntimeLocalRewriteProxyShared,
    _governance: RuntimeGatewayResponseGovernance,
) {
    let _ = request.stream(streaming, None);
}

pub(super) fn respond_runtime_local_rewrite_proxy_request(
    request_id: u64,
    request: RuntimeLocalRewriteRequest,
    response: RuntimeLocalRewriteUpstreamResult,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    governance: RuntimeGatewayResponseGovernance,
) {
    let RuntimeLocalRewriteUpstreamResult {
        response,
        gemini_context,
        copilot_context,
    } = response;
    match response {
        RuntimeLocalRewriteUpstreamResponse::Streaming(mut streaming_response) => {
            if let Some(binding) = streaming_response.accepted_binding.as_ref() {
                let _ = runtime_local_rewrite_remember_accepted_binding(
                    shared,
                    &binding.identity,
                    binding.previous_response_id.as_deref(),
                    binding.turn_state.as_deref(),
                    binding.session_id.as_deref(),
                );
            }
            if let Some(recorder) = streaming_response.accepted_binding_recorder.take() {
                streaming_response.body = Box::new(RuntimeCopilotResponsesSseBindingReader::new(
                    streaming_response.body,
                    Some(recorder),
                ));
            }
            let streaming = RuntimeStreamingResponse {
                status: streaming_response.status,
                headers: streaming_response.headers,
                body: streaming_response.body,
                request_id,
                profile_name: streaming_response.profile_name,
                log_path: shared.runtime_shared.log_path.clone(),
                shared: shared.runtime_shared.clone(),
                _inflight_guard: None,
            };
            respond_runtime_local_rewrite_stream(request, streaming, captured, shared, governance);
        }
        RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => {
            let _ = request.respond(runtime_local_rewrite_governed_response_with_call_id(
                parts, request_id, shared, governance,
            ));
        }
        RuntimeLocalRewriteUpstreamResponse::Live(live_response) => {
            respond_runtime_local_rewrite_live_response(
                request_id,
                request,
                live_response,
                gemini_context,
                copilot_context,
                captured,
                shared,
                governance,
            );
        }
    }
}

pub(super) fn runtime_local_rewrite_append_call_id_header(
    _headers: &mut Vec<(String, String)>,
    _request_id: u64,
    _shared: &RuntimeLocalRewriteProxyShared,
) {
}

pub(super) fn runtime_local_rewrite_buffered_response_parts(
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    response: impl std::io::Read,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let body = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read local provider response body",
    )?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers,
        body: body.into(),
    })
}

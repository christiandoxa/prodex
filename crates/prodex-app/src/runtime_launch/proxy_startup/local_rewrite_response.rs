use self::dispatch::respond_runtime_local_rewrite_live_response;
use super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, runtime_gateway_guardrail_webhook_block,
};
use super::local_rewrite_response_guardrails::runtime_gateway_guardrail_stream_body;
use super::local_rewrite_response_spend::{
    emit_runtime_gateway_response_spend_event_for_body, runtime_gateway_spend_stream_body,
};
use crate::{
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, RuntimeStreamingResponse,
    build_runtime_proxy_json_error_response, build_runtime_proxy_response_from_parts,
    write_runtime_streaming_response,
};
use anyhow::{Context, Result};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io::Read;

#[path = "local_rewrite_response_dispatch.rs"]
mod dispatch;
#[path = "local_rewrite_response_chat_compatible.rs"]
mod local_rewrite_response_chat_compatible;
#[path = "local_rewrite_response_copilot.rs"]
mod local_rewrite_response_copilot;
#[path = "local_rewrite_response_gemini.rs"]
mod local_rewrite_response_gemini;
#[path = "local_rewrite_response_passthrough.rs"]
mod local_rewrite_response_passthrough;

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
    if let RuntimeLocalRewriteUpstreamResponse::Streaming(streaming_response) = response {
        let writer = request.into_writer();
        let mut headers = streaming_response.headers;
        runtime_local_rewrite_append_call_id_header(&mut headers, request_id, shared);
        let body = runtime_gateway_spend_stream_body(
            runtime_gateway_guardrail_stream_body(streaming_response.body, request_id, shared),
            request_id,
            streaming_response.status,
            captured,
            shared,
        );
        let streaming = RuntimeStreamingResponse {
            status: streaming_response.status,
            headers,
            body,
            request_id,
            profile_name: streaming_response.profile_name,
            log_path: shared.runtime_shared.log_path.clone(),
            shared: shared.runtime_shared.clone(),
            _inflight_guard: None,
        };
        let _ = write_runtime_streaming_response(writer, streaming);
        return;
    }
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
        RuntimeLocalRewriteUpstreamResponse::Streaming(_) => unreachable!(),
    };
    respond_runtime_local_rewrite_live_response(
        request_id,
        request,
        live_response,
        gemini_context,
        copilot_context,
        captured,
        shared,
    );
}

fn runtime_local_rewrite_call_id(request_id: u64) -> String {
    format!("prodex-{request_id}")
}

pub(super) fn runtime_local_rewrite_append_call_id_header(
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

pub(super) fn runtime_local_rewrite_buffered_response_parts(
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

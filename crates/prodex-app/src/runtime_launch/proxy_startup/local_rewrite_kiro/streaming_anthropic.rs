use super::runtime_kiro_json_parts;
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use crate::runtime_anthropic::{
    RuntimeAnthropicMessagesRequest, build_runtime_anthropic_error_parts,
    translate_runtime_responses_reply_to_anthropic,
};
use crate::runtime_launch::proxy_startup::local_rewrite::RuntimeLocalRewriteUpstreamResponse;
use crate::runtime_launch::proxy_startup::local_rewrite_upstream::RuntimeLocalRewriteStreamingResponse;
use crate::runtime_proxy_shared::{RuntimeResponsesReply, RuntimeStreamingResponse};
use anyhow::Result;
use serde_json::Value;

pub(super) fn runtime_kiro_anthropic_message_parts_from_response(
    response: &RuntimeLocalRewriteUpstreamResponse,
    anthropic_request: &RuntimeAnthropicMessagesRequest,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let RuntimeLocalRewriteUpstreamResponse::Buffered(parts) = response else {
        return build_runtime_anthropic_error_parts(
            500,
            "api_error",
            "Kiro Anthropic messages translation expected a buffered response",
        );
    };
    let value: Value = match serde_json::from_slice(&parts.body) {
        Ok(value) => value,
        Err(_) => {
            return build_runtime_anthropic_error_parts(
                502,
                "api_error",
                "Kiro provider returned an invalid JSON response",
            );
        }
    };
    runtime_kiro_json_parts(
        200,
        prodex_provider_core::kiro_provider_core_anthropic_message_value_from_response(
            &value,
            &anthropic_request.requested_model,
        ),
    )
}

pub(super) fn runtime_kiro_anthropic_streaming_local_response(
    response: RuntimeLocalRewriteUpstreamResponse,
    anthropic_request: &RuntimeAnthropicMessagesRequest,
    request_id: u64,
    runtime_shared: &crate::RuntimeRotationProxyShared,
) -> Result<RuntimeLocalRewriteUpstreamResponse> {
    let RuntimeLocalRewriteUpstreamResponse::Streaming(streaming) = response else {
        return Ok(RuntimeLocalRewriteUpstreamResponse::Buffered(
            build_runtime_anthropic_error_parts(
                500,
                "api_error",
                "Kiro Anthropic messages streaming translation expected a streaming response",
            ),
        ));
    };
    let translated = translate_runtime_responses_reply_to_anthropic(
        RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
            status: streaming.status,
            headers: streaming.headers,
            body: streaming.body,
            request_id,
            profile_name: streaming.profile_name,
            log_path: runtime_shared.log_path.clone(),
            shared: runtime_shared.clone(),
            _inflight_guard: None,
        }),
        anthropic_request,
        request_id,
        runtime_shared,
    )?;
    Ok(match translated {
        RuntimeResponsesReply::Buffered(parts) => {
            RuntimeLocalRewriteUpstreamResponse::Buffered(parts)
        }
        RuntimeResponsesReply::Streaming(streaming) => {
            RuntimeLocalRewriteUpstreamResponse::Streaming(RuntimeLocalRewriteStreamingResponse {
                status: streaming.status,
                headers: streaming.headers,
                body: streaming.body,
                profile_name: streaming.profile_name,
            })
        }
    })
}

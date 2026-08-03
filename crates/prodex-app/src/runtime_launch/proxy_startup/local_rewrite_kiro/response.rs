use super::{
    RuntimeAnthropicMessagesRequest, RuntimeHeapTrimmedBufferedResponseParts,
    RuntimeLocalRewriteUpstreamResponse,
};
use crate::runtime_anthropic::{
    build_runtime_anthropic_error_parts, runtime_anthropic_validate_response_terminal_state,
};
use serde_json::Value;

pub(super) fn runtime_kiro_json_parts(
    status: u16,
    body: Value,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    }
}

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
    if let Err(error) = runtime_anthropic_validate_response_terminal_state(&value) {
        return build_runtime_anthropic_error_parts(502, "api_error", &error.to_string());
    }
    runtime_kiro_json_parts(
        200,
        prodex_provider_core::kiro_provider_core_anthropic_message_value_from_response(
            &value,
            &anthropic_request.requested_model,
        ),
    )
}

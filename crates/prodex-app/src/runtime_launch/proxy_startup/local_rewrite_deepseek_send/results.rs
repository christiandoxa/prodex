//! DeepSeek transport result materialization and bounded recovery telemetry.
use super::{
    RuntimeDeepSeekPendingRequest, RuntimeDeepSeekResponseAttemptContext,
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_label,
};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, runtime_proxy_log};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};

pub(super) fn runtime_deepseek_log_auth_rotate(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    api_key_label: &str,
    status: u16,
    class: RuntimeProviderErrorClass,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_auth_rotate",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::DeepSeek),
                ),
                runtime_proxy_log_field("auth", api_key_label),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("class", format!("{class:?}")),
            ],
        ),
    );
}

pub(super) fn runtime_deepseek_log_model_fallback(
    context: &RuntimeDeepSeekResponseAttemptContext<'_>,
    api_key_label: &str,
    from_model: &str,
    to_model: &str,
    status: u16,
    class: RuntimeProviderErrorClass,
) {
    runtime_proxy_log(
        &context.shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_model_fallback",
            [
                runtime_proxy_log_field("request", context.request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::DeepSeek),
                ),
                runtime_proxy_log_field("auth", api_key_label),
                runtime_proxy_log_field("from_model", from_model),
                runtime_proxy_log_field("to_model", to_model),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("class", format!("{class:?}")),
            ],
        ),
    );
}

pub(super) fn runtime_deepseek_buffered_result(
    parts: RuntimeHeapTrimmedBufferedResponseParts,
) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
        gemini_context: None,
        copilot_context: None,
    }
}

pub(super) fn runtime_deepseek_live_result(
    response: RuntimeLocalRewriteLiveResponse,
    pending_request: RuntimeDeepSeekPendingRequest,
) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Live(
            response.with_chat_compatible_request(pending_request),
        ),
        gemini_context: None,
        copilot_context: None,
    }
}

pub(super) fn runtime_deepseek_log_native_search_fallback(
    context: &RuntimeDeepSeekResponseAttemptContext<'_>,
    model: &str,
) {
    runtime_proxy_log(
        &context.shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_web_search_options_fallback",
            [
                runtime_proxy_log_field("request", context.request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::DeepSeek),
                ),
                runtime_proxy_log_field("model", model),
                runtime_proxy_log_field("route", "chat"),
                runtime_proxy_log_field("degradation", "web_search_unavailable"),
                runtime_proxy_log_field("phase", "precommit"),
            ],
        ),
    );
}

pub(super) fn runtime_deepseek_native_translation_incompatible() -> RuntimeLocalRewriteUpstreamResult
{
    runtime_deepseek_buffered_result(
        super::super::super::local_rewrite_upstream::runtime_local_rewrite_json_parts(
            400,
            serde_json::json!({
                "error": {
                    "message": "request is incompatible with DeepSeek native Anthropic web-search translation",
                    "type": "invalid_request_error",
                    "code": "invalid_request",
                }
            }),
        ),
    )
}

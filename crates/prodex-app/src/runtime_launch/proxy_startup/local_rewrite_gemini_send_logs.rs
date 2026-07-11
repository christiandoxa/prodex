use super::super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_label,
};
use crate::runtime_proxy_log;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::fmt::Display;

pub(super) fn runtime_gemini_log_model_unavailable(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    endpoint: &str,
    model: &str,
    status: u16,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_model_unavailable",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("endpoint", endpoint),
                runtime_proxy_log_field("model", model),
                runtime_proxy_log_field("status", status.to_string()),
            ],
        ),
    );
}

pub(super) fn runtime_gemini_log_builtin_tool_fallback(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    model: &str,
    status: u16,
    tool_name: &str,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_builtin_tool_fallback",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("model", model),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("tool", tool_name),
            ],
        ),
    );
}

pub(super) fn runtime_gemini_log_quota_rotate(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    status: u16,
    reason: &str,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_quota_rotate",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
}

pub(super) fn runtime_gemini_log_provider_model_fallback(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    from_model: &str,
    to_model: &str,
    status: u16,
    class: RuntimeProviderErrorClass,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_model_fallback",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::Gemini),
                ),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("from_model", from_model),
                runtime_proxy_log_field("to_model", to_model),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("class", format!("{class:?}")),
            ],
        ),
    );
}

pub(super) fn runtime_gemini_log_provider_auth_failure(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    status: u16,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_auth_failure",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::Gemini),
                ),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("status", status.to_string()),
            ],
        ),
    );
}

pub(super) fn runtime_gemini_log_provider_auth_refresh(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    status: u16,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_auth_refresh",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::Gemini),
                ),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("status", status.to_string()),
            ],
        ),
    );
}

pub(super) fn runtime_gemini_log_provider_auth_refresh_failed(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    error: impl Display,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_auth_refresh_failed",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::Gemini),
                ),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("error", error.to_string()),
            ],
        ),
    );
}

pub(super) fn runtime_gemini_log_rate_limit_retry(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    status: u16,
    retry: usize,
    delay_ms: u64,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_rate_limit_retry",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("retry", retry.to_string()),
                runtime_proxy_log_field("delay_ms", delay_ms.to_string()),
            ],
        ),
    );
}

pub(super) fn runtime_gemini_log_rate_limit_retry_fail_fast(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    status: u16,
    retry: usize,
    delay_ms: u64,
    max_inline_delay_ms: u64,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_rate_limit_retry_fail_fast",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("retry", retry.to_string()),
                runtime_proxy_log_field("delay_ms", delay_ms.to_string()),
                runtime_proxy_log_field("max_inline_delay_ms", max_inline_delay_ms.to_string()),
            ],
        ),
    );
}

pub(super) fn runtime_gemini_log_invalid_stream_retry(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    model: &str,
    retry: usize,
    reason: &str,
    delay_ms: u64,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_invalid_stream_retry",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("model", model),
                runtime_proxy_log_field("retry", retry.to_string()),
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("delay_ms", delay_ms.to_string()),
            ],
        ),
    );
}

pub(super) fn runtime_gemini_log_invalid_stream_model_fallback(
    shared: &RuntimeLocalRewriteProxyShared,
    request_id: u64,
    profile: &str,
    from_model: &str,
    to_model: &str,
    reason: &str,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_invalid_stream_model_fallback",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", profile),
                runtime_proxy_log_field("from_model", from_model),
                runtime_proxy_log_field("to_model", to_model),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
}

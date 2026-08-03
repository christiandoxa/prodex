use super::{
    ProviderRetryCause, RUNTIME_GEMINI_INVALID_STREAM_RETRY_LIMIT,
    RUNTIME_GEMINI_LOCAL_RETRY_LIMIT, RUNTIME_GEMINI_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS,
    RuntimeGeminiModelAttemptContext, RuntimeGeminiPrecommitPeek,
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeProviderBridgeKind, RuntimeProviderErrorClass,
    gemini_provider_core_unsupported_tool_fallback_body,
    runtime_gateway_application_provider_retry_precommit, runtime_gemini_body_has_terminal_quota,
    runtime_gemini_buffered_parts_are_quota_blocked, runtime_gemini_error_log_value,
    runtime_gemini_invalid_stream_retry_delay_ms, runtime_gemini_log_builtin_tool_fallback,
    runtime_gemini_log_invalid_stream_model_fallback, runtime_gemini_log_invalid_stream_retry,
    runtime_gemini_log_model_unavailable, runtime_gemini_log_provider_auth_failure,
    runtime_gemini_log_provider_auth_refresh, runtime_gemini_log_provider_auth_refresh_failed,
    runtime_gemini_log_provider_model_fallback, runtime_gemini_log_quota_rotate,
    runtime_gemini_log_rate_limit_retry, runtime_gemini_log_rate_limit_retry_fail_fast,
    runtime_gemini_normalized_error_parts, runtime_gemini_peek_stream_for_retry,
    runtime_gemini_response_retryable_quota, runtime_gemini_retry_delay_ms,
    runtime_gemini_should_inline_rate_limit_retry,
    runtime_local_rewrite_buffered_response_from_response, runtime_provider_error_class,
    runtime_provider_error_cooldown_ms,
};
use crate::build_runtime_proxy_text_response_parts;
use anyhow::Result;
use runtime_proxy_crate::runtime_forward_binary_response_headers;
use std::thread;
use std::time::Duration;

pub(super) enum RuntimeGeminiErrorAction {
    RetryRequest,
    RetryAuth,
    RetryModel,
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
}

pub(super) enum RuntimeGeminiStreamAction {
    Retry,
    RetryModel,
    Committed {
        response: reqwest::blocking::Response,
        prefix: Vec<u8>,
    },
}

pub(super) fn runtime_gemini_handle_error(
    context: &mut RuntimeGeminiModelAttemptContext<'_, '_>,
    response: reqwest::blocking::Response,
    status: u16,
    rate_limit_retry_index: &mut usize,
    auth_refresh_attempted: &mut bool,
) -> Result<RuntimeGeminiErrorAction> {
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let response_headers = runtime_forward_binary_response_headers(
        response
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_bytes())),
    );
    let parts = match runtime_local_rewrite_buffered_response_from_response(response) {
        Ok(parts) => parts,
        Err(_) => {
            let mut parts = build_runtime_proxy_text_response_parts(
                status,
                "provider response could not be processed",
            );
            parts.headers = response_headers;
            return Ok(RuntimeGeminiErrorAction::Buffered(parts));
        }
    };
    let class =
        runtime_provider_error_class(RuntimeProviderBridgeKind::Gemini, status, &parts.body);
    let quota_blocked = runtime_gemini_buffered_parts_are_quota_blocked(status, &parts);
    let delay_ms =
        runtime_gemini_retry_delay_ms(retry_after.as_deref(), &parts.body, *rate_limit_retry_index)
            .unwrap_or_else(|| {
                runtime_provider_error_cooldown_ms(
                    RuntimeProviderBridgeKind::Gemini,
                    status,
                    &parts.body,
                )
            });
    runtime_gemini_record_error_state(context, status, class, delay_ms);
    if runtime_gemini_apply_tool_fallback(context, status, &parts) {
        return Ok(RuntimeGeminiErrorAction::RetryRequest);
    }
    if let Some(action) = runtime_gemini_quota_retry(context, status, class, quota_blocked) {
        return Ok(action);
    }
    if let Some(action) = runtime_gemini_model_retry(context, status, class) {
        return Ok(action);
    }
    if let Some(action) = runtime_gemini_auth_retry(context, status, class, auth_refresh_attempted)
    {
        return Ok(action);
    }
    if runtime_gemini_rate_limit_retry(
        context,
        status,
        &parts.body,
        delay_ms,
        rate_limit_retry_index,
    ) {
        return Ok(RuntimeGeminiErrorAction::RetryRequest);
    }
    Ok(RuntimeGeminiErrorAction::Buffered(
        runtime_gemini_normalized_error_parts(status, parts),
    ))
}

fn runtime_gemini_quota_retry(
    context: &RuntimeGeminiModelAttemptContext<'_, '_>,
    status: u16,
    class: RuntimeProviderErrorClass,
    quota_blocked: bool,
) -> Option<RuntimeGeminiErrorAction> {
    if !quota_blocked
        || !runtime_gemini_response_retryable_quota(status)
        || (context.selected.hard_affinity && !context.selected.quota_fallback_allowed)
        || !runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::RotateCredential,
            class,
            context.attempt.attempt_index,
            context.attempt.attempt_count,
        )
    {
        return None;
    }
    runtime_gemini_log_quota_rotate(
        context.attempt.shared,
        context.attempt.request_id,
        context.selected.profile_name.as_str(),
        status,
        if status == 429 {
            "rate_limit"
        } else {
            "quota_body"
        },
    );
    Some(RuntimeGeminiErrorAction::RetryAuth)
}

fn runtime_gemini_model_retry(
    context: &RuntimeGeminiModelAttemptContext<'_, '_>,
    status: u16,
    class: RuntimeProviderErrorClass,
) -> Option<RuntimeGeminiErrorAction> {
    if context.model_index + 1 >= context.model_chain.len()
        || !runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::NextModel,
            class,
            context.model_index,
            context.model_chain.len(),
        )
    {
        return None;
    }
    runtime_gemini_log_provider_model_fallback(
        context.attempt.shared,
        context.attempt.request_id,
        context.selected.profile_name.as_str(),
        context.translated.model.as_str(),
        context.model_chain[context.model_index + 1].as_str(),
        status,
        class,
    );
    Some(RuntimeGeminiErrorAction::RetryModel)
}

fn runtime_gemini_auth_retry(
    context: &mut RuntimeGeminiModelAttemptContext<'_, '_>,
    status: u16,
    class: RuntimeProviderErrorClass,
    auth_refresh_attempted: &mut bool,
) -> Option<RuntimeGeminiErrorAction> {
    if class != RuntimeProviderErrorClass::Auth {
        return None;
    }
    runtime_gemini_log_provider_auth_failure(
        context.attempt.shared,
        context.attempt.request_id,
        context.selected.profile_name.as_str(),
        status,
    );
    if runtime_gemini_refresh_auth(context, status, auth_refresh_attempted) {
        return Some(RuntimeGeminiErrorAction::RetryRequest);
    }
    if !context.selected.hard_affinity
        && runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::RotateCredential,
            class,
            context.attempt.attempt_index,
            context.attempt.attempt_count,
        )
    {
        return Some(RuntimeGeminiErrorAction::RetryAuth);
    }
    None
}

fn runtime_gemini_record_error_state(
    context: &RuntimeGeminiModelAttemptContext<'_, '_>,
    status: u16,
    class: RuntimeProviderErrorClass,
    delay_ms: u64,
) {
    if matches!(
        class,
        RuntimeProviderErrorClass::Quota
            | RuntimeProviderErrorClass::RateLimit
            | RuntimeProviderErrorClass::Transient
    ) && let Some(pool) = context.attempt.shared.gemini_oauth_pool.as_ref()
    {
        pool.remember_model_cooldown(
            &context.selected.profile_name,
            &context.translated.model,
            delay_ms,
        );
    }
    if class == RuntimeProviderErrorClass::NotFound
        && let Some(pool) = context.attempt.shared.gemini_oauth_pool.as_ref()
    {
        pool.remember_model_unavailable(
            &context.selected.profile_name,
            context.model_cache_endpoint,
            &context.translated.model,
        );
        runtime_gemini_log_model_unavailable(
            context.attempt.shared,
            context.attempt.request_id,
            context.selected.profile_name.as_str(),
            context.model_cache_endpoint,
            context.translated.model.as_str(),
            status,
        );
    }
}

fn runtime_gemini_apply_tool_fallback(
    context: &mut RuntimeGeminiModelAttemptContext<'_, '_>,
    status: u16,
    parts: &RuntimeHeapTrimmedBufferedResponseParts,
) -> bool {
    let Some((tool_name, fallback_body)) = (status == 400)
        .then(|| {
            gemini_provider_core_unsupported_tool_fallback_body(
                &context.translated.body,
                &parts.body,
            )
        })
        .flatten()
    else {
        return false;
    };
    runtime_gemini_log_builtin_tool_fallback(
        context.attempt.shared,
        context.attempt.request_id,
        context.selected.profile_name.as_str(),
        context.translated.model.as_str(),
        status,
        tool_name,
    );
    context.translated.body = fallback_body;
    true
}

fn runtime_gemini_refresh_auth(
    context: &mut RuntimeGeminiModelAttemptContext<'_, '_>,
    status: u16,
    attempted: &mut bool,
) -> bool {
    if *attempted {
        return false;
    }
    let Some(pool) = context.attempt.shared.gemini_oauth_pool.as_ref() else {
        return false;
    };
    *attempted = true;
    match pool.refresh_profile_auth(
        &context.selected.profile_name,
        context.selected.hard_affinity,
        context.selected.quota_fallback_allowed,
    ) {
        Ok(Some(refreshed)) => {
            *context.selected = refreshed;
            runtime_gemini_log_provider_auth_refresh(
                context.attempt.shared,
                context.attempt.request_id,
                context.selected.profile_name.as_str(),
                status,
            );
            true
        }
        Ok(None) => false,
        Err(err) => {
            runtime_gemini_log_provider_auth_refresh_failed(
                context.attempt.shared,
                context.attempt.request_id,
                context.selected.profile_name.as_str(),
                runtime_gemini_error_log_value(&err.to_string()),
            );
            false
        }
    }
}

fn runtime_gemini_rate_limit_retry(
    context: &RuntimeGeminiModelAttemptContext<'_, '_>,
    status: u16,
    body: &[u8],
    delay_ms: u64,
    retry_index: &mut usize,
) -> bool {
    if status != 429
        || runtime_gemini_body_has_terminal_quota(body)
        || delay_ms == 0
        || *retry_index >= RUNTIME_GEMINI_LOCAL_RETRY_LIMIT
    {
        return false;
    }
    if runtime_gemini_should_inline_rate_limit_retry(delay_ms) {
        runtime_gemini_log_rate_limit_retry(
            context.attempt.shared,
            context.attempt.request_id,
            context.selected.profile_name.as_str(),
            status,
            *retry_index,
            delay_ms,
        );
        *retry_index += 1;
        thread::sleep(Duration::from_millis(delay_ms));
        return true;
    }
    runtime_gemini_log_rate_limit_retry_fail_fast(
        context.attempt.shared,
        context.attempt.request_id,
        context.selected.profile_name.as_str(),
        status,
        *retry_index,
        delay_ms,
        RUNTIME_GEMINI_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS,
    );
    false
}

pub(super) fn runtime_gemini_peek_stream_attempt(
    context: &RuntimeGeminiModelAttemptContext<'_, '_>,
    response: reqwest::blocking::Response,
    retry_index: &mut usize,
) -> Result<RuntimeGeminiStreamAction> {
    match runtime_gemini_peek_stream_for_retry(response, &context.translated.messages)? {
        RuntimeGeminiPrecommitPeek::Committed { response, prefix } => {
            Ok(RuntimeGeminiStreamAction::Committed { response, prefix })
        }
        RuntimeGeminiPrecommitPeek::RetryableInvalid {
            response,
            prefix,
            reason,
        } => {
            if *retry_index < RUNTIME_GEMINI_INVALID_STREAM_RETRY_LIMIT {
                let delay_ms = runtime_gemini_invalid_stream_retry_delay_ms(*retry_index);
                runtime_gemini_log_invalid_stream_retry(
                    context.attempt.shared,
                    context.attempt.request_id,
                    context.selected.profile_name.as_str(),
                    context.model_chain[context.model_index].as_str(),
                    *retry_index,
                    reason.as_str(),
                    delay_ms,
                );
                *retry_index += 1;
                thread::sleep(Duration::from_millis(delay_ms));
                return Ok(RuntimeGeminiStreamAction::Retry);
            }
            if context.model_index + 1 < context.model_chain.len()
                && runtime_gateway_application_provider_retry_precommit(
                    ProviderRetryCause::NextModel,
                    RuntimeProviderErrorClass::Transient,
                    context.model_index,
                    context.model_chain.len(),
                )
            {
                runtime_gemini_log_invalid_stream_model_fallback(
                    context.attempt.shared,
                    context.attempt.request_id,
                    context.selected.profile_name.as_str(),
                    context.model_chain[context.model_index].as_str(),
                    context.model_chain[context.model_index + 1].as_str(),
                    reason.as_str(),
                );
                drop(response);
                return Ok(RuntimeGeminiStreamAction::RetryModel);
            }
            Ok(RuntimeGeminiStreamAction::Committed { response, prefix })
        }
    }
}

use super::super::gemini_rewrite::{
    RuntimeGeminiAuth, RuntimeGeminiProviderAuth, RuntimeGeminiTranslatedRequest,
    runtime_gemini_generate_request_body_with_config, runtime_gemini_native_request_body,
    runtime_gemini_project_id, runtime_gemini_request_upstream_url,
};
use super::super::local_rewrite::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
    runtime_local_rewrite_model_selection,
};
use super::super::local_rewrite_application_data_plane::runtime_gateway_application_provider_retry_precommit;
use super::super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::super::local_rewrite_search_fallback::runtime_local_rewrite_remember_accepted_model;
use super::super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, send_runtime_local_rewrite_prepared_request,
};
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
    runtime_provider_error_cooldown_ms, runtime_provider_log_request_conformance,
    runtime_provider_model_from_body, runtime_provider_request_body_with_model,
    runtime_provider_request_conformance_result,
};
use super::super::{
    local_rewrite_gemini_quota::{
        runtime_gemini_buffered_parts_are_quota_blocked, runtime_gemini_normalized_error_parts,
    },
    local_rewrite_gemini_thought_signatures::runtime_gemini_harden_translated_thoughts as harden_thoughts,
};
#[path = "local_rewrite_gemini_send_logs.rs"]
mod local_rewrite_gemini_send_logs;
#[path = "local_rewrite_gemini_send_model_chain.rs"]
mod local_rewrite_gemini_send_model_chain;
#[path = "local_rewrite_gemini_send_short_circuit.rs"]
mod local_rewrite_gemini_send_short_circuit;
use super::{
    local_rewrite_gemini_oauth_pool::{
        RuntimeGeminiRequestContext, runtime_gemini_auth_attempts, runtime_gemini_binding_recorder,
    },
    local_rewrite_gemini_precommit::{
        RuntimeGeminiPrecommitPeek, runtime_gemini_peek_stream_for_retry,
        runtime_gemini_response_is_sse,
    },
};
use crate::RuntimeProxyRequest;
use anyhow::{Result, bail};
use local_rewrite_gemini_send_logs::{
    runtime_gemini_log_builtin_tool_fallback, runtime_gemini_log_invalid_stream_model_fallback,
    runtime_gemini_log_invalid_stream_retry, runtime_gemini_log_model_unavailable,
    runtime_gemini_log_provider_auth_failure, runtime_gemini_log_provider_auth_refresh,
    runtime_gemini_log_provider_auth_refresh_failed, runtime_gemini_log_provider_model_fallback,
    runtime_gemini_log_quota_rotate, runtime_gemini_log_rate_limit_retry,
    runtime_gemini_log_rate_limit_retry_fail_fast,
};
use local_rewrite_gemini_send_model_chain::{
    runtime_gemini_model_chain_for_selected_auth, runtime_gemini_remember_sticky_model_preference,
};
use local_rewrite_gemini_send_short_circuit::{
    runtime_gemini_exact_output_short_circuit, runtime_gemini_thinking_budget_tokens,
};
use prodex_provider_core::PRODEX_GEMINI_DEFAULT_MODEL as GEMINI_DEFAULT_MODEL;
use prodex_provider_core::{
    GEMINI_PROVIDER_CORE_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS as RUNTIME_GEMINI_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS,
    ProviderEndpoint,
    gemini_provider_core_body_has_terminal_quota as runtime_gemini_body_has_terminal_quota,
    gemini_provider_core_invalid_stream_retry_delay_ms as runtime_gemini_invalid_stream_retry_delay_ms,
    gemini_provider_core_request_body,
    gemini_provider_core_response_retryable_quota as runtime_gemini_response_retryable_quota,
    gemini_provider_core_retry_delay_ms as runtime_gemini_retry_delay_ms,
    gemini_provider_core_should_inline_rate_limit_retry as runtime_gemini_should_inline_rate_limit_retry,
    gemini_provider_core_simple_request, gemini_provider_core_unsupported_tool_fallback_body,
};
use prodex_provider_spi::{ProviderRetryCause, ProviderStreamMode};
use redaction::redaction_redact_secret_like_text;
use std::thread;
use std::time::Duration;

const RUNTIME_GEMINI_LOCAL_RETRY_LIMIT: usize = 9;
const RUNTIME_GEMINI_INVALID_STREAM_RETRY_LIMIT: usize = 3;

pub(in super::super) fn send_runtime_gemini_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeGeminiProviderAuth,
    endpoint: ProviderEndpoint,
    stream_mode: ProviderStreamMode,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let responses_route = endpoint == ProviderEndpoint::Responses;
    let application_streaming = stream_mode == ProviderStreamMode::Streaming;
    if responses_route {
        match auth {
            RuntimeGeminiProviderAuth::ApiKeys { api_keys } => {
                return super::local_rewrite_gemini_openai::send_runtime_gemini_openai_compatible_request(
                    request_id, request, shared, body, api_keys,
                );
            }
            RuntimeGeminiProviderAuth::Projected => {
                return super::local_rewrite_gemini_openai::send_runtime_gemini_openai_compatible_request(
                    request_id, request, shared, body, &[],
                );
            }
            RuntimeGeminiProviderAuth::OAuthProfiles { .. } => {}
        }
    }
    let conversations = shared.gemini_conversations_for_request(request);
    let thinking_budget_tokens = runtime_gemini_thinking_budget_tokens(&shared.provider);
    let model_scope = shared
        .gemini_oauth_pool
        .as_ref()
        .and_then(|pool| pool.model_scope_for_request(request, &body))
        .or_else(|| responses_route.then(|| format!("request:{request_id}")));
    let attempts = runtime_gemini_auth_attempts(auth, shared, &body, model_scope.as_deref())?;
    let common_model_selection = runtime_local_rewrite_model_selection(
        shared,
        RuntimeProviderBridgeKind::Gemini,
        request,
        &body,
        GEMINI_DEFAULT_MODEL,
    );
    let original_requested_model =
        runtime_provider_model_from_body(&body).unwrap_or_else(|| GEMINI_DEFAULT_MODEL.to_string());
    let requested_model = shared
        .gemini_oauth_pool
        .as_ref()
        .and_then(|pool| pool.selected_model_for_scope(model_scope.as_deref()))
        .filter(|_| {
            matches!(
                original_requested_model
                    .trim()
                    .to_ascii_lowercase()
                    .as_str(),
                "" | "auto" | "default"
            )
        })
        .unwrap_or_else(|| common_model_selection.model.clone());
    if responses_route
        && let Some(pool) = shared.gemini_oauth_pool.as_ref()
        && original_requested_model != "auto"
    {
        pool.remember_selected_model(model_scope.as_deref(), &original_requested_model);
    }
    let base_body = if requested_model != common_model_selection.model {
        runtime_provider_request_body_with_model(&common_model_selection.body, &requested_model)
    } else {
        common_model_selection.body
    };
    let attempt_count = attempts.len();
    'auth_attempts: for (attempt_index, mut selected) in attempts.into_iter().enumerate() {
        let (model_chain, model_cache_endpoint) = runtime_gemini_model_chain_for_selected_auth(
            request_id,
            shared,
            &selected,
            model_scope.as_deref(),
            &requested_model,
            responses_route,
        );
        for (model_index, model) in model_chain.iter().enumerate() {
            let model_body = if responses_route {
                runtime_provider_request_body_with_model(&base_body, model)
            } else {
                base_body.clone()
            };
            let conformance = if responses_route {
                runtime_provider_request_conformance_result(
                    RuntimeProviderBridgeKind::Gemini,
                    request,
                    &model_body,
                )
            } else {
                None
            };
            if let Some(result) = conformance.as_ref() {
                runtime_provider_log_request_conformance(
                    &shared.runtime_shared,
                    request_id,
                    RuntimeProviderBridgeKind::Gemini,
                    result,
                );
            }
            let mut translated = if responses_route {
                runtime_gemini_generate_request_body_with_config(
                    &model_body,
                    &conversations,
                    matches!(selected.auth, RuntimeGeminiAuth::OAuth { .. }),
                    runtime_gemini_project_id(&selected.auth),
                    thinking_budget_tokens,
                    &shared.runtime_shared.runtime_config.gemini,
                )?
            } else {
                RuntimeGeminiTranslatedRequest {
                    body: runtime_gemini_native_request_body(&body, &selected.auth)?,
                    messages: Vec::new(),
                    model: model.to_string(),
                    stream: false,
                }
            };
            if responses_route {
                translated.stream = application_streaming;
            }
            if responses_route
                && gemini_provider_core_simple_request(&model_body)
                && let Some(body) = conformance
                    .as_ref()
                    .and_then(|result| gemini_provider_core_request_body(result, &translated.body))
            {
                translated.body = body;
            }
            harden_thoughts(
                shared,
                request_id,
                selected.profile_name.as_str(),
                &mut translated,
            )?;
            if let Some(result) = runtime_gemini_exact_output_short_circuit(
                request_id,
                shared,
                &conversations,
                &selected,
                &translated,
            )? {
                return Ok(result);
            }
            let upstream_url = runtime_gemini_request_upstream_url(
                &shared.upstream_base_url,
                &selected.auth,
                &request.path_and_query,
                &translated.model,
                translated.stream,
                responses_route,
            );
            let mut rate_limit_retry_index = 0;
            let mut invalid_stream_retry_index = 0;
            let mut auth_refresh_attempted = false;
            loop {
                let mut response = send_runtime_local_rewrite_prepared_request(
                    request_id,
                    request,
                    shared,
                    &upstream_url,
                    translated.body.clone(),
                    RuntimeLocalRewritePreparedAuth::Gemini {
                        auth: &selected.auth,
                    },
                )?;
                let status = response.status().as_u16();
                if status >= 400 {
                    let retry_after = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let parts = runtime_local_rewrite_buffered_response_from_response(response)?;
                    let class = runtime_provider_error_class(
                        RuntimeProviderBridgeKind::Gemini,
                        status,
                        &parts.body,
                    );
                    let quota_blocked =
                        runtime_gemini_buffered_parts_are_quota_blocked(status, &parts);
                    let delay_ms = runtime_gemini_retry_delay_ms(
                        retry_after.as_deref(),
                        &parts.body,
                        rate_limit_retry_index,
                    )
                    .unwrap_or_else(|| {
                        runtime_provider_error_cooldown_ms(
                            RuntimeProviderBridgeKind::Gemini,
                            status,
                            &parts.body,
                        )
                    });
                    if matches!(
                        class,
                        RuntimeProviderErrorClass::Quota
                            | RuntimeProviderErrorClass::RateLimit
                            | RuntimeProviderErrorClass::Transient
                    ) && let Some(pool) = shared.gemini_oauth_pool.as_ref()
                    {
                        pool.remember_model_cooldown(
                            &selected.profile_name,
                            &translated.model,
                            delay_ms,
                        );
                    }
                    if class == RuntimeProviderErrorClass::NotFound
                        && let Some(pool) = shared.gemini_oauth_pool.as_ref()
                    {
                        pool.remember_model_unavailable(
                            &selected.profile_name,
                            &model_cache_endpoint,
                            &translated.model,
                        );
                        runtime_gemini_log_model_unavailable(
                            shared,
                            request_id,
                            selected.profile_name.as_str(),
                            model_cache_endpoint.as_str(),
                            translated.model.as_str(),
                            status,
                        );
                    }
                    if status == 400
                        && let Some((tool_name, fallback_body)) =
                            gemini_provider_core_unsupported_tool_fallback_body(&translated.body)
                    {
                        runtime_gemini_log_builtin_tool_fallback(
                            shared,
                            request_id,
                            selected.profile_name.as_str(),
                            translated.model.as_str(),
                            status,
                            tool_name,
                        );
                        translated.body = fallback_body;
                        continue;
                    }
                    if quota_blocked
                        && runtime_gemini_response_retryable_quota(status)
                        && (!selected.hard_affinity || selected.quota_fallback_allowed)
                        && runtime_gateway_application_provider_retry_precommit(
                            ProviderRetryCause::RotateCredential,
                            class,
                            attempt_index,
                            attempt_count,
                        )
                    {
                        runtime_gemini_log_quota_rotate(
                            shared,
                            request_id,
                            selected.profile_name.as_str(),
                            status,
                            if status == 429 {
                                "rate_limit"
                            } else {
                                "quota_body"
                            },
                        );
                        continue 'auth_attempts;
                    }
                    if model_index + 1 < model_chain.len()
                        && runtime_gateway_application_provider_retry_precommit(
                            ProviderRetryCause::NextModel,
                            class,
                            model_index,
                            model_chain.len(),
                        )
                    {
                        runtime_gemini_log_provider_model_fallback(
                            shared,
                            request_id,
                            selected.profile_name.as_str(),
                            translated.model.as_str(),
                            model_chain[model_index + 1].as_str(),
                            status,
                            class,
                        );
                        break;
                    }
                    if class == RuntimeProviderErrorClass::Auth {
                        runtime_gemini_log_provider_auth_failure(
                            shared,
                            request_id,
                            selected.profile_name.as_str(),
                            status,
                        );
                        if !auth_refresh_attempted
                            && let Some(pool) = shared.gemini_oauth_pool.as_ref()
                        {
                            auth_refresh_attempted = true;
                            match pool.refresh_profile_auth(
                                &selected.profile_name,
                                selected.hard_affinity,
                                selected.quota_fallback_allowed,
                            ) {
                                Ok(Some(refreshed)) => {
                                    selected = refreshed;
                                    runtime_gemini_log_provider_auth_refresh(
                                        shared,
                                        request_id,
                                        selected.profile_name.as_str(),
                                        status,
                                    );
                                    continue;
                                }
                                Ok(None) => {}
                                Err(err) => {
                                    runtime_gemini_log_provider_auth_refresh_failed(
                                        shared,
                                        request_id,
                                        selected.profile_name.as_str(),
                                        runtime_gemini_error_log_value(&err.to_string()),
                                    );
                                }
                            }
                        }
                        if !selected.hard_affinity
                            && runtime_gateway_application_provider_retry_precommit(
                                ProviderRetryCause::RotateCredential,
                                class,
                                attempt_index,
                                attempt_count,
                            )
                        {
                            continue 'auth_attempts;
                        }
                    }
                    if status == 429
                        && !runtime_gemini_body_has_terminal_quota(&parts.body)
                        && delay_ms > 0
                        && rate_limit_retry_index < RUNTIME_GEMINI_LOCAL_RETRY_LIMIT
                    {
                        if runtime_gemini_should_inline_rate_limit_retry(delay_ms) {
                            runtime_gemini_log_rate_limit_retry(
                                shared,
                                request_id,
                                selected.profile_name.as_str(),
                                status,
                                rate_limit_retry_index,
                                delay_ms,
                            );
                            rate_limit_retry_index += 1;
                            thread::sleep(Duration::from_millis(delay_ms));
                            continue;
                        }
                        runtime_gemini_log_rate_limit_retry_fail_fast(
                            shared,
                            request_id,
                            selected.profile_name.as_str(),
                            status,
                            rate_limit_retry_index,
                            delay_ms,
                            RUNTIME_GEMINI_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS,
                        );
                    }

                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Buffered(
                            runtime_gemini_normalized_error_parts(status, parts),
                        ),
                        gemini_context: None,
                        copilot_context: None,
                    });
                }

                let mut stream_prefix = Vec::new();
                if responses_route && translated.stream && runtime_gemini_response_is_sse(&response)
                {
                    match runtime_gemini_peek_stream_for_retry(response, &translated.messages)? {
                        RuntimeGeminiPrecommitPeek::Committed {
                            response: next_response,
                            prefix,
                        } => {
                            response = next_response;
                            stream_prefix = prefix;
                        }
                        RuntimeGeminiPrecommitPeek::RetryableInvalid {
                            response: next_response,
                            prefix,
                            reason,
                        } => {
                            if invalid_stream_retry_index
                                < RUNTIME_GEMINI_INVALID_STREAM_RETRY_LIMIT
                            {
                                let delay_ms = runtime_gemini_invalid_stream_retry_delay_ms(
                                    invalid_stream_retry_index,
                                );
                                runtime_gemini_log_invalid_stream_retry(
                                    shared,
                                    request_id,
                                    selected.profile_name.as_str(),
                                    translated.model.as_str(),
                                    invalid_stream_retry_index,
                                    reason.as_str(),
                                    delay_ms,
                                );
                                invalid_stream_retry_index += 1;
                                thread::sleep(Duration::from_millis(delay_ms));
                                continue;
                            }
                            if model_index + 1 < model_chain.len()
                                && runtime_gateway_application_provider_retry_precommit(
                                    ProviderRetryCause::NextModel,
                                    RuntimeProviderErrorClass::Transient,
                                    model_index,
                                    model_chain.len(),
                                )
                            {
                                runtime_gemini_log_invalid_stream_model_fallback(
                                    shared,
                                    request_id,
                                    selected.profile_name.as_str(),
                                    translated.model.as_str(),
                                    model_chain[model_index + 1].as_str(),
                                    reason.as_str(),
                                );
                                drop(next_response);
                                break;
                            }
                            response = next_response;
                            stream_prefix = prefix;
                        }
                    }
                }

                let binding_recorder = shared.gemini_oauth_pool.as_ref().map(|pool| {
                    runtime_gemini_binding_recorder(
                        pool,
                        selected.profile_name.clone(),
                        model_scope.clone(),
                    )
                });
                runtime_gemini_remember_sticky_model_preference(
                    request_id,
                    shared,
                    &selected,
                    model_scope.as_deref(),
                    &requested_model,
                    &translated.model,
                    model_index,
                );
                runtime_local_rewrite_remember_accepted_model(
                    shared,
                    RuntimeProviderBridgeKind::Gemini,
                    request,
                    &translated.model,
                );
                let gemini_context = responses_route.then(|| RuntimeGeminiRequestContext {
                    profile_name: selected.profile_name.clone(),
                    model: translated.model.clone(),
                    conversation_messages: translated.messages,
                    binding_recorder,
                });
                return Ok(RuntimeLocalRewriteUpstreamResult {
                    response: RuntimeLocalRewriteUpstreamResponse::Live(
                        RuntimeLocalRewriteLiveResponse::with_prefix(response, stream_prefix),
                    ),
                    gemini_context,
                    copilot_context: None,
                });
            }
        }
    }

    bail!("no Gemini auth attempts were available")
}

fn runtime_gemini_error_log_value(error: &str) -> String {
    redaction_redact_secret_like_text(error).replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_error_log_value_redacts_secret_like_material() {
        let message = runtime_gemini_error_log_value(
            "Gemini auth refresh failed\nAuthorization: Bearer gemini-token\napi_key=gemini-key",
        );

        assert!(!message.contains('\n'));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("gemini-token"));
        assert!(!message.contains("gemini-key"));
    }
}

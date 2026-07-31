use super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
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
        RuntimeGeminiRequestContext, RuntimeGeminiSelectedAuth, runtime_gemini_auth_attempts,
        runtime_gemini_binding_recorder,
    },
    local_rewrite_gemini_precommit::{
        RuntimeGeminiPrecommitPeek, runtime_gemini_peek_stream_for_retry,
        runtime_gemini_response_is_sse,
    },
};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest};
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

struct RuntimeGeminiAttemptContext<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeLocalRewriteProxyShared,
    body: &'a [u8],
    base_body: &'a [u8],
    conversations: &'a RuntimeDeepSeekConversationStore,
    model_scope: Option<&'a str>,
    requested_model: &'a str,
    responses_route: bool,
    application_streaming: bool,
    thinking_budget_tokens: Option<u64>,
    attempt_index: usize,
    attempt_count: usize,
}

struct RuntimeGeminiModelAttemptContext<'a, 'context> {
    attempt: &'a RuntimeGeminiAttemptContext<'context>,
    selected: &'a mut RuntimeGeminiSelectedAuth,
    translated: &'a mut RuntimeGeminiTranslatedRequest,
    model_cache_endpoint: &'a str,
    model_index: usize,
    model_chain: &'a [String],
}

fn runtime_gemini_responses_route_result(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeGeminiProviderAuth,
    endpoint: ProviderEndpoint,
) -> Option<Result<RuntimeLocalRewriteUpstreamResult>> {
    if endpoint != ProviderEndpoint::Responses {
        return None;
    }
    match auth {
        RuntimeGeminiProviderAuth::ApiKeys { api_keys } => Some(
            super::local_rewrite_gemini_openai::send_runtime_gemini_openai_compatible_request(
                request_id, request, shared, body, api_keys,
            ),
        ),
        RuntimeGeminiProviderAuth::Projected => Some(
            super::local_rewrite_gemini_openai::send_runtime_gemini_openai_compatible_request(
                request_id,
                request,
                shared,
                body,
                &[],
            ),
        ),
        RuntimeGeminiProviderAuth::OAuthProfiles { .. } => None,
    }
}

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
    if let Some(result) = runtime_gemini_responses_route_result(
        request_id,
        request,
        shared,
        body.clone(),
        auth,
        endpoint,
    ) {
        return result;
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
    for (attempt_index, selected) in attempts.into_iter().enumerate() {
        let context = RuntimeGeminiAttemptContext {
            request_id,
            request,
            shared,
            body: &body,
            base_body: &base_body,
            conversations: &conversations,
            model_scope: model_scope.as_deref(),
            requested_model: &requested_model,
            responses_route,
            application_streaming,
            thinking_budget_tokens,
            attempt_index,
            attempt_count,
        };
        if let Some(result) = runtime_gemini_attempt_selected_auth(&context, selected)? {
            return Ok(result);
        }
    }

    bail!("no Gemini auth attempts were available")
}

fn runtime_gemini_attempt_selected_auth(
    context: &RuntimeGeminiAttemptContext<'_>,
    mut selected: RuntimeGeminiSelectedAuth,
) -> Result<Option<RuntimeLocalRewriteUpstreamResult>> {
    let (model_chain, model_cache_endpoint) = runtime_gemini_model_chain_for_selected_auth(
        context.request_id,
        context.shared,
        &selected,
        context.model_scope,
        context.requested_model,
        context.responses_route,
    );
    for (model_index, model) in model_chain.iter().enumerate() {
        let model_body = if context.responses_route {
            runtime_provider_request_body_with_model(context.base_body, model)
        } else {
            context.base_body.to_vec()
        };
        let mut translated =
            runtime_gemini_translate_attempt(context, &model_body, &selected, model)?;
        if let Some(result) = runtime_gemini_exact_output_short_circuit(
            context.request_id,
            context.shared,
            context.conversations,
            &selected,
            &translated,
        )? {
            return Ok(Some(result));
        }
        let model_attempt = {
            let mut model_context = RuntimeGeminiModelAttemptContext {
                attempt: context,
                selected: &mut selected,
                translated: &mut translated,
                model_cache_endpoint: model_cache_endpoint.as_str(),
                model_index,
                model_chain: &model_chain,
            };
            runtime_gemini_send_model_attempt(&mut model_context)?
        };
        match model_attempt {
            RuntimeGeminiModelAttempt::RetryAuth => return Ok(None),
            RuntimeGeminiModelAttempt::RetryModel => continue,
            RuntimeGeminiModelAttempt::Buffered(parts) => {
                return Ok(Some(RuntimeLocalRewriteUpstreamResult {
                    response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                    gemini_context: None,
                    copilot_context: None,
                }));
            }
            RuntimeGeminiModelAttempt::Live {
                response,
                stream_prefix,
            } => {
                let binding_recorder = context.shared.gemini_oauth_pool.as_ref().map(|pool| {
                    runtime_gemini_binding_recorder(
                        pool,
                        selected.profile_name.clone(),
                        context.model_scope.map(str::to_string),
                    )
                });
                runtime_gemini_remember_sticky_model_preference(
                    context.request_id,
                    context.shared,
                    &selected,
                    context.model_scope,
                    context.requested_model,
                    &translated.model,
                    model_index,
                );
                runtime_local_rewrite_remember_accepted_model(
                    context.shared,
                    RuntimeProviderBridgeKind::Gemini,
                    context.request,
                    &translated.model,
                );
                let gemini_context = context
                    .responses_route
                    .then(|| RuntimeGeminiRequestContext {
                        profile_name: selected.profile_name.clone(),
                        model: translated.model.clone(),
                        conversation_messages: translated.messages,
                        binding_recorder,
                    });
                return Ok(Some(RuntimeLocalRewriteUpstreamResult {
                    response: RuntimeLocalRewriteUpstreamResponse::Live(
                        RuntimeLocalRewriteLiveResponse::with_prefix(response, stream_prefix),
                    ),
                    gemini_context,
                    copilot_context: None,
                }));
            }
        }
    }
    Ok(None)
}

fn runtime_gemini_error_log_value(error: &str) -> String {
    redaction_redact_secret_like_text(error).replace('\n', " ")
}

fn runtime_gemini_translate_attempt(
    context: &RuntimeGeminiAttemptContext<'_>,
    model_body: &[u8],
    selected: &RuntimeGeminiSelectedAuth,
    model: &str,
) -> Result<RuntimeGeminiTranslatedRequest> {
    let conformance = context.responses_route.then(|| {
        runtime_provider_request_conformance_result(
            RuntimeProviderBridgeKind::Gemini,
            context.request,
            model_body,
        )
    });
    if let Some(Some(result)) = conformance.as_ref() {
        runtime_provider_log_request_conformance(
            &context.shared.runtime_shared,
            context.request_id,
            RuntimeProviderBridgeKind::Gemini,
            result,
        );
    }
    let mut translated = if context.responses_route {
        runtime_gemini_generate_request_body_with_config(
            model_body,
            context.conversations,
            matches!(selected.auth, RuntimeGeminiAuth::OAuth { .. }),
            runtime_gemini_project_id(&selected.auth),
            context.thinking_budget_tokens,
            context.shared.allow_local_file_access,
            &context.shared.runtime_shared.runtime_config.gemini,
        )?
    } else {
        RuntimeGeminiTranslatedRequest {
            body: runtime_gemini_native_request_body(context.body, &selected.auth)?,
            messages: Vec::new(),
            model: model.to_string(),
            stream: false,
        }
    };
    if context.responses_route {
        translated.stream = context.application_streaming;
    }
    if context.responses_route
        && gemini_provider_core_simple_request(model_body)
        && let Some(body) = conformance
            .as_ref()
            .and_then(|result| result.as_ref())
            .and_then(|result| gemini_provider_core_request_body(result, &translated.body))
    {
        translated.body = body;
    }
    harden_thoughts(
        context.shared,
        context.request_id,
        selected.profile_name.as_str(),
        &mut translated,
    )?;
    Ok(translated)
}

enum RuntimeGeminiModelAttempt {
    RetryAuth,
    RetryModel,
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
    Live {
        response: reqwest::blocking::Response,
        stream_prefix: Vec<u8>,
    },
}

enum RuntimeGeminiErrorAction {
    RetryRequest,
    RetryAuth,
    RetryModel,
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
}

enum RuntimeGeminiStreamAction {
    Retry,
    RetryModel,
    Committed {
        response: reqwest::blocking::Response,
        prefix: Vec<u8>,
    },
}

fn runtime_gemini_send_model_attempt(
    context: &mut RuntimeGeminiModelAttemptContext<'_, '_>,
) -> Result<RuntimeGeminiModelAttempt> {
    let upstream_url = runtime_gemini_request_upstream_url(
        &context.attempt.shared.upstream_base_url,
        &context.selected.auth,
        &context.attempt.request.path_and_query,
        &context.translated.model,
        context.translated.stream,
        context.attempt.responses_route,
    );
    let mut rate_limit_retry_index = 0;
    let mut invalid_stream_retry_index = 0;
    let mut auth_refresh_attempted = false;
    loop {
        let response = send_runtime_local_rewrite_prepared_request(
            context.attempt.request_id,
            context.attempt.request,
            context.attempt.shared,
            &upstream_url,
            context.translated.body.clone(),
            RuntimeLocalRewritePreparedAuth::Gemini {
                auth: &context.selected.auth,
            },
        )?;
        let status = response.status().as_u16();
        if status >= 400 {
            match runtime_gemini_handle_error(
                context,
                response,
                status,
                &mut rate_limit_retry_index,
                &mut auth_refresh_attempted,
            )? {
                RuntimeGeminiErrorAction::RetryRequest => continue,
                RuntimeGeminiErrorAction::RetryAuth => {
                    return Ok(RuntimeGeminiModelAttempt::RetryAuth);
                }
                RuntimeGeminiErrorAction::RetryModel => {
                    return Ok(RuntimeGeminiModelAttempt::RetryModel);
                }
                RuntimeGeminiErrorAction::Buffered(parts) => {
                    return Ok(RuntimeGeminiModelAttempt::Buffered(parts));
                }
            }
        }
        let (response, stream_prefix) = if context.attempt.responses_route
            && context.translated.stream
            && runtime_gemini_response_is_sse(&response)
        {
            match runtime_gemini_peek_stream_attempt(
                context,
                response,
                &mut invalid_stream_retry_index,
            )? {
                RuntimeGeminiStreamAction::Retry => continue,
                RuntimeGeminiStreamAction::RetryModel => {
                    return Ok(RuntimeGeminiModelAttempt::RetryModel);
                }
                RuntimeGeminiStreamAction::Committed { response, prefix } => (response, prefix),
            }
        } else {
            (response, Vec::new())
        };
        return Ok(RuntimeGeminiModelAttempt::Live {
            response,
            stream_prefix,
        });
    }
}

fn runtime_gemini_handle_error(
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
    let parts = runtime_local_rewrite_buffered_response_from_response(response)?;
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

fn runtime_gemini_peek_stream_attempt(
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

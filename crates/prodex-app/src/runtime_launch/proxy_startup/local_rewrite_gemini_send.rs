use super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
use super::super::gemini_rewrite::{
    RuntimeGeminiAuth, RuntimeGeminiProviderAuth, RuntimeGeminiTranslatedRequest,
    runtime_gemini_generate_request_body_with_config, runtime_gemini_native_request_body,
    runtime_gemini_project_id, runtime_gemini_request_upstream_url,
};
use super::super::local_rewrite::{
    RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteAsyncResponse,
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
use super::super::local_rewrite_upstream::{
    RuntimeLocalRewriteBindingContext, runtime_local_rewrite_attach_accepted_binding,
    runtime_local_rewrite_binding_context, runtime_local_rewrite_raw_binding_identity,
};
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
    runtime_provider_error_cooldown_ms, runtime_provider_log_request_conformance,
    runtime_provider_model_from_body, runtime_provider_request_body_with_model,
    runtime_provider_request_conformance_result,
};
use super::super::{
    local_rewrite_gemini_quota::{
        runtime_gemini_429_is_structured, runtime_gemini_buffered_parts_are_quota_blocked,
        runtime_gemini_normalized_error_parts,
    },
    local_rewrite_gemini_thought_signatures::runtime_gemini_harden_translated_thoughts as harden_thoughts,
};
#[path = "local_rewrite_gemini_send_logs.rs"]
mod local_rewrite_gemini_send_logs;
#[path = "local_rewrite_gemini_send_model_chain.rs"]
mod local_rewrite_gemini_send_model_chain;
#[path = "local_rewrite_gemini_send_short_circuit.rs"]
mod local_rewrite_gemini_send_short_circuit;
#[path = "local_rewrite_gemini_send/retry.rs"]
mod retry;
use super::{
    local_rewrite_gemini_oauth_pool::{
        RuntimeGeminiRequestContext, RuntimeGeminiSelectedAuth, runtime_gemini_auth_attempts,
        runtime_gemini_binding_recorder, runtime_gemini_model_cache_endpoint,
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
    ProviderEndpoint, ProviderId, RuntimeProviderBindingIdentity,
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
use retry::{
    RuntimeGeminiErrorAction, RuntimeGeminiStreamAction, runtime_gemini_handle_error,
    runtime_gemini_peek_stream_attempt,
};

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
    binding: &'a RuntimeLocalRewriteBindingContext,
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
    if let Some(reason) = super::runtime_gemini_request_validation_error(&body) {
        return Ok(RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Buffered(
                crate::build_runtime_proxy_json_error_parts(400, "invalid_request_error", &reason),
            ),
            gemini_context: None,
            copilot_context: None,
        });
    }
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
    let binding = runtime_local_rewrite_binding_context(shared, request)?;
    let conversations = shared.gemini_conversations_for_request(request);
    let thinking_budget_tokens = runtime_gemini_thinking_budget_tokens(&shared.provider);
    let model_scope = shared
        .gemini_oauth_pool
        .as_ref()
        .and_then(|pool| pool.model_scope_for_request(request, &body))
        .or_else(|| responses_route.then(|| format!("request:{request_id}")));
    let mut attempts = runtime_gemini_auth_attempts(auth, shared, &body, model_scope.as_deref())?;
    attempts.retain(|selected| {
        let endpoint =
            runtime_gemini_model_cache_endpoint(&selected.auth, &shared.upstream_base_url);
        runtime_gemini_binding_identity(shared, selected, &endpoint)
            .as_ref()
            .is_some_and(|identity| binding.candidate_allowed(Some(identity)))
    });
    if attempts.is_empty() {
        if binding.bound.is_some() {
            bail!("Gemini continuation binding is unavailable or unauthorized");
        }
        bail!("no Gemini auth attempts were available");
    }
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
            binding: &binding,
        };
        if let Some(result) = runtime_gemini_attempt_selected_auth(&context, selected)? {
            return Ok(result);
        }
    }

    bail!("no Gemini auth attempts were available")
}

fn runtime_gemini_binding_identity(
    shared: &RuntimeLocalRewriteProxyShared,
    selected: &RuntimeGeminiSelectedAuth,
    endpoint: &str,
) -> Option<RuntimeProviderBindingIdentity> {
    let (credential, profile) = match &selected.auth {
        RuntimeGeminiAuth::ApiKey { api_key } => {
            (Some(api_key.as_str()), Some(selected.profile_name.as_str()))
        }
        RuntimeGeminiAuth::OAuth { access_token, .. } => (
            Some(access_token.as_str()),
            Some(selected.profile_name.as_str()),
        ),
        RuntimeGeminiAuth::Projected => (None, Some(RUNTIME_LOCAL_REWRITE_PROFILE)),
    };
    runtime_local_rewrite_raw_binding_identity(
        shared,
        ProviderId::Gemini,
        credential,
        endpoint,
        profile,
    )
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
        let Some(binding_identity) =
            runtime_gemini_binding_identity(context.shared, &selected, &model_cache_endpoint)
        else {
            bail!("Gemini accepted binding identity is unavailable");
        };
        if !context.binding.candidate_allowed(Some(&binding_identity)) {
            bail!("Gemini continuation binding changed before commit");
        }
        if let Some(result) = runtime_gemini_exact_output_short_circuit(
            context.request_id,
            context.shared,
            context.conversations,
            &selected,
            &translated,
            context.binding,
            &binding_identity,
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
                let mut live_response =
                    RuntimeLocalRewriteLiveResponse::with_prefix(response, stream_prefix);
                runtime_local_rewrite_attach_accepted_binding(
                    context.shared,
                    &mut live_response,
                    context.binding,
                    binding_identity,
                );
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
                    response: RuntimeLocalRewriteUpstreamResponse::Live(live_response),
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

#[allow(clippy::large_enum_variant)]
enum RuntimeGeminiModelAttempt {
    RetryAuth,
    RetryModel,
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
    Live {
        response: RuntimeLocalRewriteAsyncResponse,
        stream_prefix: Vec<u8>,
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

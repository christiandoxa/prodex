use super::gemini_rewrite::RuntimeGeminiOAuthProfileAuth;
#[cfg(test)]
use super::gemini_rewrite::runtime_gemini_generate_request_body;
use super::gemini_rewrite::{
    RuntimeGeminiAuth, RuntimeGeminiProviderAuth, RuntimeGeminiTranslatedRequest,
    runtime_gemini_generate_request_body_with_local_file_access,
    runtime_gemini_native_request_body, runtime_gemini_project_id,
    runtime_gemini_request_upstream_url,
};
#[cfg(test)]
use super::gemini_sse::RuntimeGeminiBindingRecorder;
use super::gemini_sse::{RuntimeGeminiGenerateSseReader, runtime_gemini_forced_command_output};
use super::local_rewrite::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, runtime_local_rewrite_model_selection,
};
pub(super) use super::local_rewrite_gemini_bindings::runtime_gemini_remember_bindings_from_responses_body;
use super::local_rewrite_gemini_models::{
    runtime_gemini_model_fallback_chain, runtime_gemini_retain_code_assist_models,
    runtime_gemini_unsupported_tool_fallback_body,
};
use super::local_rewrite_gemini_quota::{
    runtime_gemini_body_has_rate_limit, runtime_gemini_body_has_terminal_quota,
    runtime_gemini_buffered_parts_are_quota_blocked, runtime_gemini_normalized_error_parts,
    runtime_gemini_retry_delay_ms,
};
use super::local_rewrite_gemini_retry::{
    RUNTIME_GEMINI_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS,
    runtime_gemini_invalid_stream_retry_delay_ms, runtime_gemini_should_inline_rate_limit_retry,
    runtime_gemini_should_rotate_after_quota_response,
};
use super::local_rewrite_gemini_thought_signatures::runtime_gemini_harden_translated_thoughts as harden_thoughts;
use super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
    runtime_provider_error_cooldown_ms, runtime_provider_log_request_conformance,
    runtime_provider_model_from_body, runtime_provider_request_body_with_model,
    runtime_provider_request_conformance_result, runtime_provider_should_retry_with_next_model,
};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Context, Result, bail};
use prodex_provider_core::ProviderTransformLoss;
use prodex_runtime_gemini::GEMINI_DEFAULT_MODEL;
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::io::Read;
use std::thread;
use std::time::Duration;

const RUNTIME_GEMINI_LOCAL_RETRY_LIMIT: usize = 9;
const RUNTIME_GEMINI_INVALID_STREAM_RETRY_LIMIT: usize = 3;

#[path = "local_rewrite_gemini_auth.rs"]
mod local_rewrite_gemini_auth;
#[path = "local_rewrite_gemini_oauth_pool.rs"]
mod local_rewrite_gemini_oauth_pool;
pub(super) use local_rewrite_gemini_oauth_pool::runtime_gemini_live_auth_attempts;
use local_rewrite_gemini_oauth_pool::{
    RUNTIME_GEMINI_MODEL_PREFERENCE_TTL_MS, RuntimeGeminiSelectedAuth,
    runtime_gemini_auth_attempts, runtime_gemini_binding_recorder,
    runtime_gemini_model_cache_endpoint,
};
pub(super) use local_rewrite_gemini_oauth_pool::{
    RuntimeGeminiOAuthPool, RuntimeGeminiRequestContext, runtime_gemini_oauth_pool_from_provider,
};
#[cfg(test)]
use local_rewrite_gemini_oauth_pool::{
    RuntimeGeminiOAuthPoolState, runtime_gemini_initial_oauth_pool_index, runtime_gemini_now_ms,
};
#[path = "local_rewrite_gemini_openai.rs"]
mod local_rewrite_gemini_openai;
use local_rewrite_gemini_openai::send_runtime_gemini_openai_compatible_request;
#[path = "local_rewrite_gemini_precommit.rs"]
mod local_rewrite_gemini_precommit;
#[cfg(test)]
use local_rewrite_gemini_precommit::{
    RuntimeGeminiPrecommitDecision, RuntimeGeminiPrecommitProbe,
    runtime_gemini_precommit_decision_for_data_lines,
};
use local_rewrite_gemini_precommit::{
    RuntimeGeminiPrecommitPeek, runtime_gemini_peek_stream_for_retry,
    runtime_gemini_response_is_sse,
};

pub(super) fn send_runtime_gemini_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeGeminiProviderAuth,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let responses_route = path_without_query(&request.path_and_query).ends_with("/responses");
    if responses_route && let RuntimeGeminiProviderAuth::ApiKeys { api_keys } = auth {
        return send_runtime_gemini_openai_compatible_request(
            request_id, request, shared, body, api_keys,
        );
    }
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
        let mut model_chain = if responses_route {
            runtime_gemini_model_fallback_chain(&shared.provider, &requested_model)
        } else {
            vec![GEMINI_DEFAULT_MODEL.to_string()]
        };
        // Gemini Code Assist (OAuth) does not serve customtools models; filter them from the fallback chain.
        if matches!(selected.auth, RuntimeGeminiAuth::OAuth { .. }) {
            runtime_gemini_retain_code_assist_models(&mut model_chain);
            if model_chain.is_empty() {
                model_chain.push(GEMINI_DEFAULT_MODEL.to_string());
            }
        }
        if let Some(pool) = shared.gemini_oauth_pool.as_ref() {
            model_chain = pool.preferred_model_chain_for_profile(
                model_scope.as_deref(),
                &selected.profile_name,
                &requested_model,
                &model_chain,
            );
        }
        let model_cache_endpoint =
            runtime_gemini_model_cache_endpoint(&selected.auth, &shared.upstream_base_url);
        if let Some(pool) = shared.gemini_oauth_pool.as_ref() {
            let available_chain = pool.available_model_chain_for_profile(
                &selected.profile_name,
                &model_cache_endpoint,
                &model_chain,
            );
            if !available_chain.is_empty() && available_chain.len() < model_chain.len() {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_gemini_model_unavailable_skip",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("profile", selected.profile_name.as_str()),
                            runtime_proxy_log_field("endpoint", model_cache_endpoint.as_str()),
                            runtime_proxy_log_field("models_before", model_chain.len().to_string()),
                            runtime_proxy_log_field(
                                "models_after",
                                available_chain.len().to_string(),
                            ),
                        ],
                    ),
                );
                model_chain = available_chain;
            }
        }
        let conversations = shared.gemini_conversations_for_request(request);
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
                runtime_gemini_generate_request_body_with_local_file_access(
                    &model_body,
                    &conversations,
                    matches!(selected.auth, RuntimeGeminiAuth::OAuth { .. }),
                    runtime_gemini_project_id(&selected.auth),
                    thinking_budget_tokens,
                    shared.allow_local_file_access,
                )?
            } else {
                RuntimeGeminiTranslatedRequest {
                    body: runtime_gemini_native_request_body(&body, &selected.auth)?,
                    messages: Vec::new(),
                    model: model.clone(),
                    stream: false,
                }
            };
            if responses_route
                && runtime_gemini_provider_core_simple_request(&model_body)
                && let Some(body) = conformance.as_ref().and_then(|result| {
                    runtime_gemini_provider_core_request_body(result, &translated.body)
                })
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
                &selected,
                &translated,
                &conversations,
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
                        runtime_provider_error_cooldown_ms(class, status, &parts.body)
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
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_gemini_model_unavailable",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field(
                                        "endpoint",
                                        model_cache_endpoint.as_str(),
                                    ),
                                    runtime_proxy_log_field("model", translated.model.as_str()),
                                    runtime_proxy_log_field("status", status.to_string()),
                                ],
                            ),
                        );
                    }
                    if status == 400
                        && let Some((tool_name, fallback_body)) =
                            runtime_gemini_unsupported_tool_fallback_body(&translated.body)
                    {
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_gemini_builtin_tool_fallback",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field("model", translated.model.as_str()),
                                    runtime_proxy_log_field("status", status.to_string()),
                                    runtime_proxy_log_field("tool", tool_name),
                                ],
                            ),
                        );
                        translated.body = fallback_body;
                        continue;
                    }
                    if runtime_gemini_should_rotate_after_quota_response(
                        status,
                        quota_blocked,
                        selected.hard_affinity,
                        selected.quota_fallback_allowed,
                        attempt_index,
                        attempt_count,
                    ) {
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_gemini_quota_rotate",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field("status", status.to_string()),
                                    runtime_proxy_log_field(
                                        "reason",
                                        if status == 429 {
                                            "rate_limit"
                                        } else {
                                            "quota_body"
                                        },
                                    ),
                                ],
                            ),
                        );
                        continue 'auth_attempts;
                    }
                    if runtime_provider_should_retry_with_next_model(class)
                        && model_index + 1 < model_chain.len()
                    {
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_provider_model_fallback",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field("provider", "gemini"),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field(
                                        "from_model",
                                        translated.model.as_str(),
                                    ),
                                    runtime_proxy_log_field(
                                        "to_model",
                                        model_chain[model_index + 1].as_str(),
                                    ),
                                    runtime_proxy_log_field("status", status.to_string()),
                                    runtime_proxy_log_field("class", format!("{class:?}")),
                                ],
                            ),
                        );
                        break;
                    }
                    if class == RuntimeProviderErrorClass::Auth {
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_provider_auth_failure",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field("provider", "gemini"),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field("status", status.to_string()),
                                ],
                            ),
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
                                    runtime_proxy_log(
                                        &shared.runtime_shared,
                                        runtime_proxy_structured_log_message(
                                            "local_rewrite_provider_auth_refresh",
                                            [
                                                runtime_proxy_log_field(
                                                    "request",
                                                    request_id.to_string(),
                                                ),
                                                runtime_proxy_log_field("provider", "gemini"),
                                                runtime_proxy_log_field(
                                                    "profile",
                                                    selected.profile_name.as_str(),
                                                ),
                                                runtime_proxy_log_field(
                                                    "status",
                                                    status.to_string(),
                                                ),
                                            ],
                                        ),
                                    );
                                    continue;
                                }
                                Ok(None) => {}
                                Err(err) => {
                                    runtime_proxy_log(
                                        &shared.runtime_shared,
                                        runtime_proxy_structured_log_message(
                                            "local_rewrite_provider_auth_refresh_failed",
                                            [
                                                runtime_proxy_log_field(
                                                    "request",
                                                    request_id.to_string(),
                                                ),
                                                runtime_proxy_log_field("provider", "gemini"),
                                                runtime_proxy_log_field(
                                                    "profile",
                                                    selected.profile_name.as_str(),
                                                ),
                                                runtime_proxy_log_field("error", err.to_string()),
                                            ],
                                        ),
                                    );
                                }
                            }
                        }
                        if !selected.hard_affinity && attempt_index + 1 < attempt_count {
                            continue 'auth_attempts;
                        }
                    }
                    if status == 429
                        && runtime_gemini_body_has_rate_limit(&parts.body)
                        && !runtime_gemini_body_has_terminal_quota(&parts.body)
                        && delay_ms > 0
                        && rate_limit_retry_index < RUNTIME_GEMINI_LOCAL_RETRY_LIMIT
                    {
                        if runtime_gemini_should_inline_rate_limit_retry(delay_ms) {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_gemini_rate_limit_retry",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field(
                                            "profile",
                                            selected.profile_name.as_str(),
                                        ),
                                        runtime_proxy_log_field("status", status.to_string()),
                                        runtime_proxy_log_field(
                                            "retry",
                                            rate_limit_retry_index.to_string(),
                                        ),
                                        runtime_proxy_log_field("delay_ms", delay_ms.to_string()),
                                    ],
                                ),
                            );
                            rate_limit_retry_index += 1;
                            thread::sleep(Duration::from_millis(delay_ms));
                            continue;
                        }
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_gemini_rate_limit_retry_fail_fast",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field("status", status.to_string()),
                                    runtime_proxy_log_field(
                                        "retry",
                                        rate_limit_retry_index.to_string(),
                                    ),
                                    runtime_proxy_log_field("delay_ms", delay_ms.to_string()),
                                    runtime_proxy_log_field(
                                        "max_inline_delay_ms",
                                        RUNTIME_GEMINI_MAX_INLINE_RATE_LIMIT_RETRY_DELAY_MS
                                            .to_string(),
                                    ),
                                ],
                            ),
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
                                runtime_proxy_log(
                                    &shared.runtime_shared,
                                    runtime_proxy_structured_log_message(
                                        "local_rewrite_gemini_invalid_stream_retry",
                                        [
                                            runtime_proxy_log_field(
                                                "request",
                                                request_id.to_string(),
                                            ),
                                            runtime_proxy_log_field(
                                                "profile",
                                                selected.profile_name.as_str(),
                                            ),
                                            runtime_proxy_log_field(
                                                "model",
                                                translated.model.as_str(),
                                            ),
                                            runtime_proxy_log_field(
                                                "retry",
                                                invalid_stream_retry_index.to_string(),
                                            ),
                                            runtime_proxy_log_field("reason", reason.as_str()),
                                            runtime_proxy_log_field(
                                                "delay_ms",
                                                delay_ms.to_string(),
                                            ),
                                        ],
                                    ),
                                );
                                invalid_stream_retry_index += 1;
                                thread::sleep(Duration::from_millis(delay_ms));
                                continue;
                            }
                            if model_index + 1 < model_chain.len() {
                                runtime_proxy_log(
                                    &shared.runtime_shared,
                                    runtime_proxy_structured_log_message(
                                        "local_rewrite_gemini_invalid_stream_model_fallback",
                                        [
                                            runtime_proxy_log_field(
                                                "request",
                                                request_id.to_string(),
                                            ),
                                            runtime_proxy_log_field(
                                                "profile",
                                                selected.profile_name.as_str(),
                                            ),
                                            runtime_proxy_log_field(
                                                "from_model",
                                                translated.model.as_str(),
                                            ),
                                            runtime_proxy_log_field(
                                                "to_model",
                                                model_chain[model_index + 1].as_str(),
                                            ),
                                            runtime_proxy_log_field("reason", reason.as_str()),
                                        ],
                                    ),
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
                if model_index > 0
                    && let Some(pool) = shared.gemini_oauth_pool.as_ref()
                {
                    pool.remember_model_preference(
                        model_scope.as_deref(),
                        &selected.profile_name,
                        &requested_model,
                        &translated.model,
                    );
                    runtime_proxy_log(
                        &shared.runtime_shared,
                        runtime_proxy_structured_log_message(
                            "local_rewrite_gemini_sticky_model",
                            [
                                runtime_proxy_log_field("request", request_id.to_string()),
                                runtime_proxy_log_field("profile", selected.profile_name.as_str()),
                                runtime_proxy_log_field(
                                    "scope",
                                    model_scope.as_deref().unwrap_or("-"),
                                ),
                                runtime_proxy_log_field(
                                    "requested_model",
                                    requested_model.as_str(),
                                ),
                                runtime_proxy_log_field("model", translated.model.as_str()),
                                runtime_proxy_log_field(
                                    "ttl_ms",
                                    RUNTIME_GEMINI_MODEL_PREFERENCE_TTL_MS.to_string(),
                                ),
                            ],
                        ),
                    );
                }
                let gemini_context = responses_route.then(|| RuntimeGeminiRequestContext {
                    profile_name: selected.profile_name.clone(),
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

fn runtime_gemini_provider_core_simple_request(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    if let Some(tools) = object.get("tools") {
        let Some(tools) = tools.as_array() else {
            return false;
        };
        if !tools.iter().all(runtime_gemini_provider_core_builtin_tool) {
            return false;
        }
        if object
            .get("tool_choice")
            .is_some_and(|value| !runtime_gemini_provider_core_builtin_tool_choice(value))
        {
            return false;
        }
    }
    match object.get("input") {
        Some(serde_json::Value::String(_)) => true,
        Some(serde_json::Value::Array(items)) => items.iter().all(runtime_gemini_simple_input_item),
        _ => false,
    }
}

fn runtime_gemini_simple_input_item(item: &serde_json::Value) -> bool {
    let Some(object) = item.as_object() else {
        return false;
    };
    if object
        .get("type")
        .is_some_and(|value| value.as_str() != Some("message"))
    {
        return false;
    }
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("user");
    if !matches!(role, "system" | "user" | "assistant" | "tool") {
        return false;
    }
    match object.get("content") {
        Some(serde_json::Value::String(_)) | None => {}
        Some(serde_json::Value::Array(items))
            if items.iter().all(runtime_gemini_simple_content_item) => {}
        _ => return false,
    }
    if object.get("gemini_native_parts").is_some() {
        return false;
    }
    if role == "assistant" {
        return object
            .get("tool_calls")
            .is_none_or(runtime_gemini_simple_tool_calls);
    }
    if role == "tool" {
        return object
            .get("tool_call_id")
            .is_none_or(serde_json::Value::is_string)
            && object.get("name").is_none_or(serde_json::Value::is_string);
    }
    object.get("tool_calls").is_none()
}

fn runtime_gemini_simple_content_item(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object
        .get("text")
        .and_then(serde_json::Value::as_str)
        .is_some()
        || object
            .get("content")
            .and_then(serde_json::Value::as_str)
            .is_some()
    {
        return object.get("type").is_none_or(|value| {
            matches!(value.as_str(), Some("input_text" | "output_text" | "text"))
        });
    }
    false
}

fn runtime_gemini_simple_tool_calls(value: &serde_json::Value) -> bool {
    let Some(tool_calls) = value.as_array() else {
        return false;
    };
    tool_calls.iter().all(runtime_gemini_simple_tool_call)
}

fn runtime_gemini_simple_tool_call(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object
        .get("id")
        .is_some_and(|value| !serde_json::Value::is_string(value))
    {
        return false;
    }
    let Some(function) = object
        .get("function")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    function
        .get("name")
        .is_some_and(serde_json::Value::is_string)
        && function
            .get("arguments")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|arguments| serde_json::from_str::<serde_json::Value>(arguments).is_ok())
}

fn runtime_gemini_provider_core_builtin_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    tool_type == "web_search"
        || tool_type == "web_search_preview"
        || tool_type.starts_with("web_search_preview_")
        || matches!(
            tool_type,
            "code_interpreter" | "code_execution" | "codeExecution"
        )
        || matches!(
            tool_type,
            "computer" | "computer_use" | "computerUse" | "computer_use_preview"
        )
        || tool_type.starts_with("computer_")
        || matches!(
            tool_type,
            "web_fetch" | "url_context" | "urlContext" | "web_fetch_preview"
        )
        || tool_type.starts_with("web_fetch_preview_")
        || tool.as_object().is_some_and(|object| {
            object.contains_key("computerUse")
                || object.contains_key("codeExecution")
                || object.contains_key("urlContext")
        })
}

fn runtime_gemini_provider_core_builtin_tool_choice(value: &serde_json::Value) -> bool {
    value.is_null() || matches!(value.as_str(), None | Some("auto") | Some("none"))
}

fn runtime_gemini_provider_core_request_body(
    result: &prodex_provider_core::ProviderTransformResult,
    translated_body: &[u8],
) -> Option<Vec<u8>> {
    match result.loss {
        ProviderTransformLoss::Lossless | ProviderTransformLoss::DegradedButSafe { .. } => {}
        ProviderTransformLoss::Rejected { .. }
        | ProviderTransformLoss::UnsupportedUpstream { .. } => {
            return None;
        }
    }
    let base = result.body.as_ref()?;
    let mut base_value = serde_json::from_slice::<serde_json::Value>(base).ok()?;
    let translated_value = serde_json::from_slice::<serde_json::Value>(translated_body).ok()?;
    let base_object = base_value.as_object_mut()?;
    let translated_object = translated_value.as_object()?;
    if let Some(project) = translated_object.get("project") {
        base_object.insert("project".to_string(), project.clone());
    }
    let translated_request = translated_object.get("request")?.as_object()?;
    let base_request = base_object.get_mut("request")?.as_object_mut()?;
    if let Some(generation_config) = translated_request.get("generationConfig") {
        base_request.insert("generationConfig".to_string(), generation_config.clone());
    }
    serde_json::to_vec(&base_value).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        runtime_gemini_generate_request_body, runtime_gemini_provider_core_request_body,
        runtime_gemini_provider_core_simple_request,
    };
    use prodex_provider_core::{
        ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformLoss,
        ProviderTransformResult, ProviderWireFormat, provider_translator,
    };

    fn conversation_store() -> super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore {
        super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore::default()
    }

    #[test]
    fn gemini_provider_core_simple_request_accepts_plain_text_and_context_blocks() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": [
                {"role": "system", "content": "You are precise."},
                {"role": "user", "content": "# AGENTS.md instructions for /repo"},
                {"role": "user", "content": "Fix the test"}
            ]
        }))
        .unwrap();
        assert!(runtime_gemini_provider_core_simple_request(&body));
    }

    #[test]
    fn gemini_provider_core_simple_request_rejects_tools_and_unsupported_assistant_history() {
        let tools = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": "hello",
            "tools": [{"type":"function","function":{"name":"search"}}]
        }))
        .unwrap();
        assert!(!runtime_gemini_provider_core_simple_request(&tools));

        let assistant = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": [
                {
                    "role":"assistant",
                    "content":"hi",
                    "gemini_native_parts":[{"text":"native"}]
                }
            ]
        }))
        .unwrap();
        assert!(!runtime_gemini_provider_core_simple_request(&assistant));
    }

    #[test]
    fn gemini_provider_core_simple_request_accepts_builtin_tools() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": "hello",
            "tools": [
                {"type":"web_search_preview"},
                {"type":"code_interpreter"},
                {
                    "type":"computer_use_preview",
                    "environment":"ENVIRONMENT_BROWSER"
                },
                {"type":"web_fetch_preview"}
            ],
            "tool_choice": "auto"
        }))
        .unwrap();
        assert!(runtime_gemini_provider_core_simple_request(&body));
    }

    #[test]
    fn gemini_provider_core_simple_request_accepts_text_format_and_cache_fields() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": "hello",
            "cached_content": "cachedContents/abc123",
            "text": {
                "format": {
                    "type": "json_object"
                }
            }
        }))
        .unwrap();
        assert!(runtime_gemini_provider_core_simple_request(&body));
    }

    #[test]
    fn gemini_provider_core_simple_request_accepts_assistant_and_tool_history() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": [
                {"role":"user","content":"find it"},
                {
                    "role":"assistant",
                    "content":"",
                    "tool_calls":[
                        {
                            "id":"call_1",
                            "type":"function",
                            "function":{
                                "name":"grep",
                                "arguments":"{\"pattern\":\"x\"}"
                            }
                        }
                    ]
                },
                {
                    "role":"tool",
                    "tool_call_id":"call_1",
                    "content":"{\"match_count\":1}"
                }
            ]
        }))
        .unwrap();
        assert!(runtime_gemini_provider_core_simple_request(&body));
    }

    #[test]
    fn gemini_provider_core_simple_request_accepts_message_content_arrays() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "# AGENTS.md instructions for /repo"},
                        {"type": "input_text", "text": "<environment_context><cwd>/repo</cwd></environment_context>"}
                    ]
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": ""}],
                    "tool_calls": [
                        {
                            "id":"call_1",
                            "type":"function",
                            "function":{
                                "name":"grep",
                                "arguments":"{\"pattern\":\"x\"}"
                            }
                        }
                    ]
                },
                {
                    "type": "message",
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": [{"type": "output_text", "text": "{\"match_count\":1}"}]
                }
            ]
        }))
        .unwrap();
        assert!(runtime_gemini_provider_core_simple_request(&body));
    }

    #[test]
    fn gemini_provider_core_request_body_preserves_project_and_generation_config() {
        let result = ProviderTransformResult {
            provider: ProviderId::Gemini,
            endpoint: ProviderEndpoint::Responses,
            from_format: ProviderWireFormat::OpenAiResponses,
            to_format: ProviderWireFormat::GeminiGenerateContent,
            body: Some(
                serde_json::to_vec(&serde_json::json!({
                    "model": "gemini-2.5-pro",
                    "request": {
                        "contents": [{"role":"user","parts":[{"text":"hello"}]}]
                    }
                }))
                .unwrap(),
            ),
            headers: Default::default(),
            metadata: Default::default(),
            loss: ProviderTransformLoss::Lossless,
        };
        let translated = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "project": "project-1",
            "request": {
                "contents": [{"role":"user","parts":[{"text":"hello"}]}],
                "generationConfig": {"includeThoughts": true, "thinkingBudget": 8192}
            }
        }))
        .unwrap();
        let merged = runtime_gemini_provider_core_request_body(&result, &translated).unwrap();
        let merged: serde_json::Value = serde_json::from_slice(&merged).unwrap();
        assert_eq!(merged["project"], "project-1");
        assert_eq!(
            merged["request"]["generationConfig"]["thinkingBudget"],
            8192
        );
    }

    #[test]
    fn gemini_provider_core_body_overrides_legacy_simple_history_translation() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": [
                {"role":"user","content":"find it"},
                {
                    "role":"assistant",
                    "content":"",
                    "tool_calls":[
                        {
                            "id":"call_1",
                            "type":"function",
                            "function":{
                                "name":"grep",
                                "arguments":"{\"pattern\":\"x\"}"
                            }
                        }
                    ]
                },
                {
                    "role":"tool",
                    "tool_call_id":"call_1",
                    "content":"{\"match_count\":1}"
                }
            ]
        }))
        .unwrap();
        assert!(runtime_gemini_provider_core_simple_request(&body));

        let translated =
            runtime_gemini_generate_request_body(&body, &conversation_store(), true, None, None)
                .unwrap();
        let result = provider_translator(ProviderId::Gemini).transform_request(
            ProviderTransformInput::new(ProviderEndpoint::Responses, body),
        );
        let merged = runtime_gemini_provider_core_request_body(&result, &translated.body).unwrap();

        let provider_core_json: serde_json::Value = serde_json::from_slice(&merged).unwrap();
        let app_json: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        assert_ne!(provider_core_json, app_json);
        assert_eq!(
            provider_core_json["request"]["contents"][0]["parts"][0]["text"],
            "find it"
        );
        assert_eq!(
            provider_core_json["request"]["contents"][1]["parts"][0]["functionCall"]["name"],
            "grep"
        );
        assert_eq!(
            provider_core_json["request"]["contents"][2]["parts"][0]["functionResponse"]["name"],
            "grep"
        );
        assert!(
            provider_core_json["request"]
                .get("systemInstruction")
                .is_none()
        );
        assert!(app_json["request"].get("systemInstruction").is_some());
    }
}

fn runtime_gemini_exact_output_short_circuit(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    selected: &RuntimeGeminiSelectedAuth,
    translated: &RuntimeGeminiTranslatedRequest,
    conversations: &super::deepseek_rewrite::RuntimeDeepSeekConversationStore,
) -> Result<Option<RuntimeLocalRewriteUpstreamResult>> {
    if !translated.stream {
        return Ok(None);
    }
    let Some(output) = runtime_gemini_forced_command_output(&translated.messages) else {
        return Ok(None);
    };
    let binding_recorder = shared
        .gemini_oauth_pool
        .as_ref()
        .map(|pool| runtime_gemini_binding_recorder(pool, selected.profile_name.clone(), None));
    let generate_chunk = serde_json::json!({
        "responseId": format!("resp_gemini_exact_{request_id}"),
        "modelVersion": translated.model,
        "candidates": [{
            "content": {
                "parts": [{"text": output}]
            },
            "finishReason": "STOP"
        }]
    });
    let fake_stream = format!("data: {generate_chunk}\n\ndata: [DONE]\n\n");
    let mut reader = RuntimeGeminiGenerateSseReader::new(
        std::io::Cursor::new(fake_stream.into_bytes()),
        request_id,
        translated.messages.clone(),
        conversations.clone(),
        binding_recorder,
    );
    let mut body = String::new();
    reader
        .read_to_string(&mut body)
        .context("failed to build Gemini exact-output SSE response")?;
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_exact_output_short_circuit",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", selected.profile_name.as_str()),
                runtime_proxy_log_field("model", translated.model.as_str()),
                runtime_proxy_log_field("bytes", body.len().to_string()),
            ],
        ),
    );
    let mut headers = vec![
        (
            "content-type".to_string(),
            b"text/event-stream; charset=utf-8".to_vec(),
        ),
        ("x-reasoning-included".to_string(), b"true".to_vec()),
    ];
    if let Some(pool) = shared.gemini_oauth_pool.as_ref() {
        headers.extend(
            pool.quota_headers_for_profile(&selected.profile_name)
                .into_iter()
                .map(|(name, value)| (name, value.into_bytes())),
        );
    }
    Ok(Some(RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Buffered(
            RuntimeHeapTrimmedBufferedResponseParts {
                status: 200,
                headers,
                body: body.into_bytes().into(),
            },
        ),
        gemini_context: None,
        copilot_context: None,
    }))
}

fn runtime_gemini_thinking_budget_tokens(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Option<u64> {
    match provider {
        RuntimeLocalRewriteProviderOptions::Gemini {
            thinking_budget_tokens,
            ..
        } => *thinking_budget_tokens,
        _ => None,
    }
}

#[cfg(test)]
#[path = "local_rewrite_gemini_precommit_tests.rs"]
mod local_rewrite_gemini_precommit_tests;

#[cfg(test)]
#[path = "local_rewrite_gemini_tests.rs"]
mod local_rewrite_gemini_tests;

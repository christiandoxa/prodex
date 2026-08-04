#[cfg(test)]
use super::super::copilot_instructions::{
    runtime_copilot_apply_custom_instructions, runtime_copilot_cached_workspace_custom_instructions,
};
#[cfg(test)]
use super::chat_compatible_rewrite::{
    RuntimeChatCompatibleConversationStore, RuntimeDeepSeekRewriteOptions,
    runtime_provider_chat_compatible_request_body,
};
#[cfg(test)]
use super::deepseek_rewrite::RuntimeDeepSeekTranslatedRequest;
use super::local_rewrite::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
    runtime_local_rewrite_model_selection,
};
use super::local_rewrite_application_data_plane::runtime_gateway_application_provider_retry_precommit;
pub(super) use super::local_rewrite_copilot_bindings::{
    RuntimeCopilotBindingRecorder, RuntimeCopilotResponsesSseBindingReader,
    runtime_copilot_remember_bindings_from_responses_body,
};
use super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_local_rewrite_upstream_url,
};
use super::local_rewrite_transport_copilot::{
    runtime_copilot_request_body_with_canonical_model,
    runtime_copilot_request_body_without_encrypted_content,
};
use super::local_rewrite_upstream::runtime_local_rewrite_retryable_429_body;
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_label,
    runtime_provider_model_fallback_chain, runtime_provider_request_body_with_model,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Result, bail};
use prodex_provider_core::ProviderEndpoint;
use prodex_provider_spi::ProviderRetryCause;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};

struct RuntimeCopilotModelRequestContext<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeLocalRewriteProxyShared,
    upstream_url: &'a str,
    model: &'a str,
    selected: &'a RuntimeCopilotSelectedAuth,
}
#[cfg(test)]
use std::sync::{Arc, Mutex};

#[path = "local_rewrite_copilot/auth.rs"]
mod auth;
#[path = "local_rewrite_copilot/state.rs"]
mod state;
use self::auth::{
    runtime_copilot_auth_attempts_for_request, runtime_copilot_binding_recorder,
    runtime_copilot_public_endpoint, runtime_copilot_upstream_base_url,
};
use self::state::RuntimeCopilotSelectedAuth;
pub(super) use self::state::{
    RuntimeCopilotOAuthPool, RuntimeCopilotRequestContext,
    runtime_copilot_model_catalog_from_provider, runtime_copilot_oauth_pool_from_provider,
};
pub(crate) use self::state::{RuntimeCopilotProfileAuth, RuntimeCopilotProviderAuth};

pub(super) fn send_runtime_copilot_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeCopilotProviderAuth,
    endpoint: ProviderEndpoint,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let responses_route = endpoint == ProviderEndpoint::Responses;
    if responses_route {
        return send_runtime_copilot_responses_request(request_id, request, shared, body, auth);
    }

    let model_selection = runtime_local_rewrite_model_selection(
        shared,
        RuntimeProviderBridgeKind::Copilot,
        request,
        &body,
        prodex_cli::SUPER_COPILOT_DEFAULT_MODEL,
    );
    let model_chain = runtime_provider_model_fallback_chain(
        RuntimeProviderBridgeKind::Copilot,
        &model_selection.model,
    );
    let attempts = runtime_copilot_auth_attempts_for_request(auth, shared, &body, request)?;
    let attempt_count = attempts.len();
    for (attempt_index, selected) in attempts.into_iter().enumerate() {
        let upstream_url = runtime_local_rewrite_upstream_url(
            runtime_copilot_upstream_base_url(shared, &selected),
            &shared.mount_path,
            &request.path_and_query,
        );
        for (model_index, model) in model_chain.iter().enumerate() {
            let model_body = runtime_provider_request_body_with_model(&model_selection.body, model);
            let send_context = RuntimeCopilotModelRequestContext {
                request_id,
                request,
                shared,
                upstream_url: &upstream_url,
                model,
                selected: &selected,
            };
            let send_result = send_runtime_copilot_model_request(&send_context, model_body, true)?;
            let (status, parts, class) = match send_result {
                RuntimeLocalRewritePreparedSendResult::Live(response) => {
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(
                            RuntimeLocalRewriteLiveResponse::new(response),
                        ),
                        gemini_context: None,
                        copilot_context: Some(runtime_copilot_request_context(
                            shared, request, &selected, &body,
                        )),
                    });
                }
                RuntimeLocalRewritePreparedSendResult::Error {
                    status,
                    parts,
                    class,
                } => (status, parts, class),
            };
            if model_index + 1 < model_chain.len()
                && runtime_copilot_provider_retry_precommit(
                    ProviderRetryCause::NextModel,
                    class,
                    status,
                    &parts.body,
                    model_index,
                    model_chain.len(),
                )
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_model_fallback",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field(
                                "provider",
                                runtime_provider_label(RuntimeProviderBridgeKind::Copilot),
                            ),
                            runtime_proxy_log_field("auth", selected.profile_name.as_str()),
                            runtime_proxy_log_field("from_model", model.as_str()),
                            runtime_proxy_log_field(
                                "to_model",
                                model_chain[model_index + 1].as_str(),
                            ),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("class", format!("{class:?}")),
                        ],
                    ),
                );
                continue;
            }
            if !selected.hard_affinity
                && runtime_copilot_provider_retry_precommit(
                    ProviderRetryCause::RotateCredential,
                    class,
                    status,
                    &parts.body,
                    attempt_index,
                    attempt_count,
                )
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_copilot_profile_rotate",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("profile", selected.profile_name.as_str()),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("class", format!("{class:?}")),
                        ],
                    ),
                );
                break;
            }
            return Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                gemini_context: None,
                copilot_context: None,
            });
        }
    }
    bail!("no Copilot auth attempts were available")
}

fn send_runtime_copilot_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeCopilotProviderAuth,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let attempts = runtime_copilot_auth_attempts_for_request(auth, shared, &body, request)?;
    let attempt_count = attempts.len();
    let model_selection = runtime_local_rewrite_model_selection(
        shared,
        RuntimeProviderBridgeKind::Copilot,
        request,
        &body,
        prodex_cli::SUPER_COPILOT_DEFAULT_MODEL,
    );
    let model_chain = runtime_provider_model_fallback_chain(
        RuntimeProviderBridgeKind::Copilot,
        &model_selection.model,
    );

    for (attempt_index, selected) in attempts.into_iter().enumerate() {
        let upstream_url = runtime_local_rewrite_upstream_url(
            runtime_copilot_upstream_base_url(shared, &selected),
            &shared.mount_path,
            &request.path_and_query,
        );
        for (model_index, model) in model_chain.iter().enumerate() {
            let model_body = runtime_provider_request_body_with_model(&model_selection.body, model);
            let send_context = RuntimeCopilotModelRequestContext {
                request_id,
                request,
                shared,
                upstream_url: &upstream_url,
                model,
                selected: &selected,
            };
            let send_result = send_runtime_copilot_model_request(&send_context, model_body, false)?;
            let (status, parts, class) = match send_result {
                RuntimeLocalRewritePreparedSendResult::Live(response) => {
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(
                            RuntimeLocalRewriteLiveResponse::new(response),
                        ),
                        gemini_context: None,
                        copilot_context: Some(runtime_copilot_request_context(
                            shared, request, &selected, &body,
                        )),
                    });
                }
                RuntimeLocalRewritePreparedSendResult::Error {
                    status,
                    parts,
                    class,
                } => (status, parts, class),
            };
            if model_index + 1 < model_chain.len()
                && runtime_copilot_provider_retry_precommit(
                    ProviderRetryCause::NextModel,
                    class,
                    status,
                    &parts.body,
                    model_index,
                    model_chain.len(),
                )
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_model_fallback",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field(
                                "provider",
                                runtime_provider_label(RuntimeProviderBridgeKind::Copilot),
                            ),
                            runtime_proxy_log_field("auth", selected.profile_name.as_str()),
                            runtime_proxy_log_field("from_model", model.as_str()),
                            runtime_proxy_log_field(
                                "to_model",
                                model_chain[model_index + 1].as_str(),
                            ),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("class", format!("{class:?}")),
                        ],
                    ),
                );
                continue;
            }
            if !selected.hard_affinity
                && runtime_copilot_provider_retry_precommit(
                    ProviderRetryCause::RotateCredential,
                    class,
                    status,
                    &parts.body,
                    attempt_index,
                    attempt_count,
                )
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_copilot_profile_rotate",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("profile", selected.profile_name.as_str()),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("class", format!("{class:?}")),
                        ],
                    ),
                );
                break;
            }
            return Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                gemini_context: None,
                copilot_context: None,
            });
        }
    }
    bail!("no Copilot model attempts were available")
}

fn runtime_copilot_provider_retry_precommit(
    cause: ProviderRetryCause,
    class: RuntimeProviderErrorClass,
    status: u16,
    body: &[u8],
    attempt_index: usize,
    candidate_count: usize,
) -> bool {
    (status != 429 || runtime_local_rewrite_retryable_429_body(body))
        && runtime_gateway_application_provider_retry_precommit(
            cause,
            class,
            attempt_index,
            candidate_count,
        )
}

fn send_runtime_copilot_model_request(
    context: &RuntimeCopilotModelRequestContext<'_>,
    model_body: Vec<u8>,
    canonical_model: bool,
) -> Result<RuntimeLocalRewritePreparedSendResult> {
    let model_body = if canonical_model {
        runtime_copilot_request_body_with_canonical_model(&model_body)
    } else {
        model_body
    };
    let (model_body, stripped_encrypted_content) =
        runtime_copilot_request_body_without_encrypted_content(&model_body);
    if stripped_encrypted_content {
        runtime_proxy_log(
            &context.shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_copilot_encrypted_content_stripped",
                [
                    runtime_proxy_log_field("request", context.request_id.to_string()),
                    runtime_proxy_log_field("profile", context.selected.profile_name.as_str()),
                    runtime_proxy_log_field("model", context.model),
                ],
            ),
        );
    }
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
        RuntimeLocalRewriteSearchFallbackRequest {
            request_id: context.request_id,
            request: context.request,
            shared: context.shared,
            upstream_url: context.upstream_url,
            body: model_body,
            provider_kind: RuntimeProviderBridgeKind::Copilot,
            auth_label: context.selected.profile_name.as_str(),
            model: context.model,
            auth_factory: || RuntimeLocalRewritePreparedAuth::Copilot {
                api_key: (!context.selected.projected).then_some(context.selected.api_key.as_str()),
            },
        },
    )
}

#[cfg(test)]
fn runtime_copilot_responses_chat_request_body(
    body: &[u8],
    conversations: &RuntimeChatCompatibleConversationStore,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    let mut translated = runtime_provider_chat_compatible_request_body(
        body,
        conversations,
        RuntimeProviderBridgeKind::Copilot,
        prodex_cli::SUPER_COPILOT_DEFAULT_MODEL,
        false,
        RuntimeDeepSeekRewriteOptions::default(),
    )?;
    if let Some(instructions) = runtime_copilot_cached_workspace_custom_instructions() {
        runtime_copilot_apply_custom_instructions(&mut translated, instructions)?;
    }
    Ok(translated)
}

fn runtime_copilot_request_context(
    shared: &RuntimeLocalRewriteProxyShared,
    request: &RuntimeProxyRequest,
    selected: &RuntimeCopilotSelectedAuth,
    body: &[u8],
) -> RuntimeCopilotRequestContext {
    let profile_name = selected.profile_name.clone();
    let turn_state = runtime_proxy_crate::runtime_request_turn_state(request);
    let session_id = runtime_proxy_crate::runtime_request_session_id(request);
    let response_id =
        super::local_rewrite_copilot::auth::runtime_copilot_previous_response_id(body);
    let endpoint = runtime_copilot_public_endpoint(shared, selected);
    let binding_recorder = shared.copilot_oauth_pool.as_ref().map(|pool| {
        runtime_copilot_binding_recorder(
            pool,
            &shared.runtime_shared,
            selected.clone(),
            endpoint.clone(),
            turn_state.clone(),
            session_id.clone(),
        )
    });
    let binding_acceptance = shared.copilot_oauth_pool.as_ref().map(|pool| {
        super::local_rewrite_copilot::auth::runtime_copilot_binding_acceptance(
            pool,
            &shared.runtime_shared,
            selected.clone(),
            endpoint.clone(),
            response_id,
            turn_state.clone(),
            session_id.clone(),
        )
    });
    RuntimeCopilotRequestContext {
        profile_name,
        binding_recorder,
        binding_acceptance,
    }
}

#[cfg(test)]
#[path = "local_rewrite_copilot_tests.rs"]
mod local_rewrite_copilot_tests;

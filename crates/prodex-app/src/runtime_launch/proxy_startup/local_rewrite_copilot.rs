use super::super::copilot_instructions::{
    runtime_copilot_apply_custom_instructions, runtime_copilot_cached_workspace_custom_instructions,
};
use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekTranslatedRequest,
    runtime_chat_compatible_request_body,
};
use super::local_rewrite::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, runtime_local_rewrite_model_selection,
};
pub(super) use super::local_rewrite_copilot_bindings::{
    RuntimeCopilotBindingRecorder, RuntimeCopilotResponsesSseBindingReader,
    runtime_copilot_remember_bindings_from_responses_body,
};
use super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_local_rewrite_api_key_attempts,
    runtime_local_rewrite_upstream_url, runtime_openai_standard_provider_upstream_url,
};
use super::local_rewrite_transport_copilot::runtime_copilot_request_body_with_canonical_model;
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_model_fallback_chain,
    runtime_provider_request_body_with_model, runtime_provider_should_retry_with_next_model,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Context, Result, bail};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

const RUNTIME_COPILOT_PROVIDER_BINDING_LIMIT: usize = 4096;

#[derive(Clone)]
pub(crate) enum RuntimeCopilotProviderAuth {
    ApiKeys {
        api_keys: Vec<String>,
    },
    Profiles {
        profiles: Vec<RuntimeCopilotProfileAuth>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeCopilotProfileAuth {
    pub(crate) profile_name: String,
    pub(crate) api_key: String,
    pub(crate) api_url: String,
    pub(crate) model_catalog: Vec<serde_json::Value>,
}

#[derive(Clone)]
pub(super) struct RuntimeCopilotOAuthPool {
    state: Arc<Mutex<RuntimeCopilotOAuthPoolState>>,
}

#[derive(Debug)]
struct RuntimeCopilotOAuthPoolState {
    profiles: Vec<RuntimeCopilotProfileAuth>,
    next_index: usize,
    response_profile_bindings: BTreeMap<String, String>,
}

#[derive(Clone)]
struct RuntimeCopilotSelectedAuth {
    profile_name: String,
    api_key: String,
    api_url: Option<String>,
    hard_affinity: bool,
}

#[derive(Clone)]
pub(super) struct RuntimeCopilotRequestContext {
    pub(super) profile_name: String,
    pub(super) binding_recorder: Option<RuntimeCopilotBindingRecorder>,
}

pub(super) fn runtime_copilot_model_catalog_from_provider(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Vec<serde_json::Value> {
    let RuntimeLocalRewriteProviderOptions::Copilot {
        auth: RuntimeCopilotProviderAuth::Profiles { profiles },
    } = provider
    else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut catalog = Vec::new();
    for profile in profiles {
        for model in &profile.model_catalog {
            let Some(id) = model.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let id = id.trim();
            if id.is_empty() || !seen.insert(id.to_ascii_lowercase()) {
                continue;
            }
            catalog.push(model.clone());
        }
    }
    catalog
}

pub(super) fn runtime_copilot_oauth_pool_from_provider(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Option<RuntimeCopilotOAuthPool> {
    let RuntimeLocalRewriteProviderOptions::Copilot {
        auth: RuntimeCopilotProviderAuth::Profiles { profiles },
    } = provider
    else {
        return None;
    };
    Some(RuntimeCopilotOAuthPool {
        state: Arc::new(Mutex::new(RuntimeCopilotOAuthPoolState {
            profiles: profiles.clone(),
            next_index: 0,
            response_profile_bindings: BTreeMap::new(),
        })),
    })
}

pub(super) fn send_runtime_copilot_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeCopilotProviderAuth,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let responses_route = path_without_query(&request.path_and_query).ends_with("/responses");
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
    let attempts = runtime_copilot_auth_attempts(auth, shared, &body)?;
    let attempt_count = attempts.len();
    for (attempt_index, selected) in attempts.into_iter().enumerate() {
        let upstream_url = runtime_local_rewrite_upstream_url(
            runtime_copilot_upstream_base_url(shared, &selected),
            &shared.mount_path,
            &request.path_and_query,
        );
        for (model_index, model) in model_chain.iter().enumerate() {
            let model_body = runtime_provider_request_body_with_model(&model_selection.body, model);
            let model_body = runtime_copilot_request_body_with_canonical_model(&model_body);
            let send_result =
                send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
                    RuntimeLocalRewriteSearchFallbackRequest {
                        request_id,
                        request,
                        shared,
                        upstream_url: upstream_url.as_str(),
                        body: model_body,
                        provider_kind: RuntimeProviderBridgeKind::Copilot,
                        auth_label: selected.profile_name.as_str(),
                        model,
                        auth_factory: || RuntimeLocalRewritePreparedAuth::Copilot {
                            api_key: selected.api_key.as_str(),
                        },
                    },
                )?;
            let (status, parts, class) = match send_result {
                RuntimeLocalRewritePreparedSendResult::Live(response) => {
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(
                            RuntimeLocalRewriteLiveResponse::new(response),
                        ),
                        gemini_context: None,
                        copilot_context: None,
                    });
                }
                RuntimeLocalRewritePreparedSendResult::Error {
                    status,
                    parts,
                    class,
                } => (status, parts, class),
            };
            if model_index + 1 < model_chain.len()
                && runtime_provider_should_retry_with_next_model(class)
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_model_fallback",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("provider", "copilot"),
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
                && attempt_index + 1 < attempt_count
                && runtime_copilot_should_rotate_after_response(class)
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
    let attempts = runtime_copilot_auth_attempts(auth, shared, &body)?;
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
        let upstream_url = runtime_openai_standard_provider_upstream_url(
            RuntimeProviderBridgeKind::Copilot,
            runtime_copilot_upstream_base_url(shared, &selected),
            &shared.mount_path,
            &request.path_and_query,
        );
        for (model_index, model) in model_chain.iter().enumerate() {
            let model_body = runtime_provider_request_body_with_model(&model_selection.body, model);
            let translated = runtime_copilot_responses_chat_request_body(
                &model_body,
                &shared.deepseek_conversations,
            )?;
            if let Ok(mut pending) = shared.deepseek_pending_messages.lock() {
                pending.insert(request_id, translated.messages);
            }
            let send_result =
                send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
                    RuntimeLocalRewriteSearchFallbackRequest {
                        request_id,
                        request,
                        shared,
                        upstream_url: upstream_url.as_str(),
                        body: translated.body,
                        provider_kind: RuntimeProviderBridgeKind::Copilot,
                        auth_label: selected.profile_name.as_str(),
                        model,
                        auth_factory: || RuntimeLocalRewritePreparedAuth::Copilot {
                            api_key: selected.api_key.as_str(),
                        },
                    },
                )?;
            let (status, parts, class) = match send_result {
                RuntimeLocalRewritePreparedSendResult::Live(response) => {
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(
                            RuntimeLocalRewriteLiveResponse::new(response),
                        ),
                        gemini_context: None,
                        copilot_context: Some(runtime_copilot_request_context(
                            shared,
                            selected.profile_name.clone(),
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
                && runtime_provider_should_retry_with_next_model(class)
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_model_fallback",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("provider", "copilot"),
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
                && attempt_index + 1 < attempt_count
                && runtime_copilot_should_rotate_after_response(class)
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

fn runtime_copilot_responses_chat_request_body(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    let mut translated = runtime_chat_compatible_request_body(
        body,
        conversations,
        RuntimeProviderBridgeKind::Copilot,
        prodex_cli::SUPER_COPILOT_DEFAULT_MODEL,
        false,
    )?;
    if let Some(instructions) = runtime_copilot_cached_workspace_custom_instructions() {
        runtime_copilot_apply_custom_instructions(&mut translated, instructions)?;
    }
    Ok(translated)
}

fn runtime_copilot_request_context(
    shared: &RuntimeLocalRewriteProxyShared,
    profile_name: String,
) -> RuntimeCopilotRequestContext {
    let binding_recorder = shared
        .copilot_oauth_pool
        .as_ref()
        .map(|pool| runtime_copilot_binding_recorder(pool, profile_name.clone()));
    RuntimeCopilotRequestContext {
        profile_name,
        binding_recorder,
    }
}

fn runtime_copilot_auth_attempts(
    auth: &RuntimeCopilotProviderAuth,
    shared: &RuntimeLocalRewriteProxyShared,
    body: &[u8],
) -> Result<Vec<RuntimeCopilotSelectedAuth>> {
    match auth {
        RuntimeCopilotProviderAuth::ApiKeys { api_keys } => {
            let attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys)
                .into_iter()
                .map(|(label, api_key)| RuntimeCopilotSelectedAuth {
                    profile_name: label,
                    api_key: api_key.to_string(),
                    api_url: None,
                    hard_affinity: api_keys.len() <= 1,
                })
                .collect::<Vec<_>>();
            if attempts.is_empty() {
                bail!("Copilot API-key pool is empty");
            }
            Ok(attempts)
        }
        RuntimeCopilotProviderAuth::Profiles { profiles } => {
            let pool = shared
                .copilot_oauth_pool
                .as_ref()
                .context("Copilot OAuth pool was not initialized")?;
            pool.select_attempts(body, profiles)
        }
    }
}

impl RuntimeCopilotOAuthPool {
    fn select_attempts(
        &self,
        body: &[u8],
        fallback_profiles: &[RuntimeCopilotProfileAuth],
    ) -> Result<Vec<RuntimeCopilotSelectedAuth>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Copilot OAuth pool lock poisoned"))?;
        if let Some(profile_name) = state.affinity_profile_for_body(body)
            && let Some(profile) = state.profile_by_name(&profile_name)
        {
            return Ok(vec![RuntimeCopilotSelectedAuth {
                profile_name,
                api_key: profile.api_key,
                api_url: Some(profile.api_url),
                hard_affinity: true,
            }]);
        }
        let profiles = if state.profiles.is_empty() {
            fallback_profiles.to_vec()
        } else {
            state.profiles.clone()
        };
        if profiles.is_empty() {
            bail!("Copilot OAuth pool is empty");
        }
        let start = state.next_index.min(profiles.len().saturating_sub(1));
        state.next_index = (start + 1) % profiles.len();
        Ok((0..profiles.len())
            .map(|offset| {
                let profile = profiles[(start + offset) % profiles.len()].clone();
                RuntimeCopilotSelectedAuth {
                    profile_name: profile.profile_name,
                    api_key: profile.api_key,
                    api_url: Some(profile.api_url),
                    hard_affinity: false,
                }
            })
            .collect())
    }
}

fn runtime_copilot_upstream_base_url<'a>(
    shared: &'a RuntimeLocalRewriteProxyShared,
    selected: &'a RuntimeCopilotSelectedAuth,
) -> &'a str {
    selected
        .api_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .unwrap_or(shared.upstream_base_url.as_str())
}

impl RuntimeCopilotOAuthPoolState {
    fn profile_by_name(&self, profile_name: &str) -> Option<RuntimeCopilotProfileAuth> {
        self.profiles
            .iter()
            .find(|profile| profile.profile_name == profile_name)
            .cloned()
    }

    fn affinity_profile_for_body(&self, body: &[u8]) -> Option<String> {
        let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
        value
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|response_id| self.response_profile_bindings.get(response_id))
            .cloned()
    }

    fn remember_response_binding(&mut self, profile_name: &str, response_id: &str) {
        if response_id.trim().is_empty() {
            return;
        }
        self.response_profile_bindings
            .insert(response_id.to_string(), profile_name.to_string());
        while self.response_profile_bindings.len() > RUNTIME_COPILOT_PROVIDER_BINDING_LIMIT {
            let Some(key) = self.response_profile_bindings.keys().next().cloned() else {
                break;
            };
            self.response_profile_bindings.remove(&key);
        }
    }
}

fn runtime_copilot_binding_recorder(
    pool: &RuntimeCopilotOAuthPool,
    profile_name: String,
) -> RuntimeCopilotBindingRecorder {
    let pool = pool.clone();
    Arc::new(move |response_id| {
        if let Ok(mut state) = pool.state.lock() {
            state.remember_response_binding(&profile_name, &response_id);
        }
    })
}

fn runtime_copilot_should_rotate_after_response(class: RuntimeProviderErrorClass) -> bool {
    matches!(
        class,
        RuntimeProviderErrorClass::Auth
            | RuntimeProviderErrorClass::Quota
            | RuntimeProviderErrorClass::RateLimit
            | RuntimeProviderErrorClass::Transient
    )
}

#[cfg(test)]
#[path = "local_rewrite_copilot_tests.rs"]
mod local_rewrite_copilot_tests;

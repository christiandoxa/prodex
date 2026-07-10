use super::super::gemini_rewrite::{
    RuntimeGeminiAuth, RuntimeGeminiOAuthProfileAuth, RuntimeGeminiProviderAuth,
};
use super::super::gemini_sse::RuntimeGeminiBindingRecorder;
use super::super::local_rewrite::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared,
};
use super::super::local_rewrite_transport::runtime_local_rewrite_api_key_attempts;
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_model_fallback_chain,
    runtime_provider_model_from_body,
};
use super::local_rewrite_gemini_auth::{
    runtime_gemini_oauth_affinity_attempts, runtime_gemini_oauth_attempt_from_profile,
};
use crate::{RuntimeProxyRequest, gemini_code_assist_endpoint};
use anyhow::{Context, Result, bail};
use prodex_runtime_gemini::GEMINI_DEFAULT_MODEL;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const RUNTIME_GEMINI_MODEL_UNAVAILABLE_TTL_MS: u64 = 60 * 60 * 1_000;
pub(super) const RUNTIME_GEMINI_MODEL_PREFERENCE_TTL_MS: u64 = 10 * 60 * 1_000;

#[path = "local_rewrite_gemini_oauth_pool/quota.rs"]
mod quota;
#[path = "local_rewrite_gemini_oauth_pool/state.rs"]
mod state;

#[derive(Clone)]
pub(in super::super) struct RuntimeGeminiOAuthPool {
    pub(in super::super) state: Arc<Mutex<RuntimeGeminiOAuthPoolState>>,
}

#[derive(Debug)]
pub(in super::super) struct RuntimeGeminiOAuthPoolState {
    pub(in super::super) profiles: Vec<RuntimeGeminiOAuthProfileAuth>,
    pub(in super::super) next_index: usize,
    pub(in super::super) response_profile_bindings: BTreeMap<String, String>,
    pub(in super::super) tool_call_profile_bindings: BTreeMap<String, String>,
    pub(in super::super) session_profile_bindings: BTreeMap<String, String>,
    pub(in super::super) response_model_scope_bindings: BTreeMap<String, String>,
    pub(in super::super) tool_call_model_scope_bindings: BTreeMap<String, String>,
    pub(in super::super) quota_headers: BTreeMap<String, Vec<(String, String)>>,
    pub(in super::super) model_cooldowns_until: BTreeMap<String, u64>,
    pub(in super::super) model_unavailable_until: BTreeMap<String, u64>,
    pub(in super::super) model_preferences: BTreeMap<String, RuntimeGeminiModelPreference>,
    pub(in super::super) selected_model_preferences: BTreeMap<String, RuntimeGeminiModelPreference>,
}

#[derive(Clone, Debug)]
pub(in super::super) struct RuntimeGeminiModelPreference {
    model: String,
    until_ms: u64,
}

#[derive(Clone)]
pub(in super::super) struct RuntimeGeminiSelectedAuth {
    pub(in super::super) profile_name: String,
    pub(in super::super) auth: RuntimeGeminiAuth,
    pub(in super::super) hard_affinity: bool,
    pub(in super::super) quota_fallback_allowed: bool,
}

#[derive(Clone)]
pub(in super::super) struct RuntimeGeminiRequestContext {
    pub(in super::super) profile_name: String,
    pub(in super::super) conversation_messages: Vec<serde_json::Value>,
    pub(in super::super) binding_recorder: Option<RuntimeGeminiBindingRecorder>,
}

pub(in super::super) fn runtime_gemini_oauth_pool_from_provider(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Option<RuntimeGeminiOAuthPool> {
    let RuntimeLocalRewriteProviderOptions::Gemini {
        auth: RuntimeGeminiProviderAuth::OAuthProfiles { profiles },
        ..
    } = provider
    else {
        return None;
    };
    Some(RuntimeGeminiOAuthPool {
        state: Arc::new(Mutex::new(RuntimeGeminiOAuthPoolState {
            profiles: profiles.clone(),
            next_index: runtime_gemini_initial_oauth_pool_index(profiles.len()),
            response_profile_bindings: BTreeMap::new(),
            tool_call_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
            response_model_scope_bindings: BTreeMap::new(),
            tool_call_model_scope_bindings: BTreeMap::new(),
            quota_headers: BTreeMap::new(),
            model_cooldowns_until: BTreeMap::new(),
            model_unavailable_until: BTreeMap::new(),
            model_preferences: BTreeMap::new(),
            selected_model_preferences: BTreeMap::new(),
        })),
    })
}

pub(super) fn runtime_gemini_auth_attempts(
    auth: &RuntimeGeminiProviderAuth,
    shared: &RuntimeLocalRewriteProxyShared,
    body: &[u8],
    model_scope: Option<&str>,
) -> Result<Vec<RuntimeGeminiSelectedAuth>> {
    match auth {
        RuntimeGeminiProviderAuth::ApiKeys { api_keys } => {
            let attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys)
                .into_iter()
                .map(|(label, api_key)| RuntimeGeminiSelectedAuth {
                    profile_name: label,
                    auth: RuntimeGeminiAuth::ApiKey {
                        api_key: api_key.to_string(),
                    },
                    hard_affinity: api_keys.len() <= 1,
                    quota_fallback_allowed: false,
                })
                .collect::<Vec<_>>();
            if attempts.is_empty() {
                bail!("Gemini API-key pool is empty");
            }
            Ok(attempts)
        }
        RuntimeGeminiProviderAuth::OAuthProfiles { profiles } => {
            let pool = shared
                .gemini_oauth_pool
                .as_ref()
                .context("Gemini OAuth pool was not initialized")?;
            pool.select_attempts(body, profiles, model_scope)
        }
    }
}

pub(in super::super) fn runtime_gemini_live_auth_attempts(
    auth: &RuntimeGeminiProviderAuth,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<Vec<(String, RuntimeGeminiAuth)>> {
    Ok(runtime_gemini_auth_attempts(auth, shared, b"{}", None)?
        .into_iter()
        .map(|selected| (selected.profile_name, selected.auth))
        .collect())
}

impl RuntimeGeminiOAuthPool {
    pub(super) fn remember_model_cooldown(
        &self,
        profile_name: &str,
        model: &str,
        cooldown_ms: u64,
    ) {
        if profile_name.trim().is_empty() || model.trim().is_empty() || cooldown_ms == 0 {
            return;
        }
        let until = runtime_gemini_now_ms().saturating_add(cooldown_ms);
        if let Ok(mut state) = self.state.lock() {
            state.remember_model_cooldown_until(profile_name, model, until);
        }
    }

    pub(super) fn remember_model_unavailable(
        &self,
        profile_name: &str,
        endpoint: &str,
        model: &str,
    ) {
        if profile_name.trim().is_empty() || endpoint.trim().is_empty() || model.trim().is_empty() {
            return;
        }
        let until = runtime_gemini_now_ms().saturating_add(RUNTIME_GEMINI_MODEL_UNAVAILABLE_TTL_MS);
        if let Ok(mut state) = self.state.lock() {
            state.remember_model_unavailable_until(profile_name, endpoint, model, until);
        }
    }

    pub(super) fn model_scope_for_request(
        &self,
        request: &RuntimeProxyRequest,
        body: &[u8],
    ) -> Option<String> {
        let Ok(state) = self.state.lock() else {
            return runtime_gemini_explicit_session_scope(request, body);
        };
        runtime_gemini_explicit_session_scope(request, body)
            .or_else(|| state.model_scope_for_body(body))
    }

    pub(in super::super) fn remember_selected_model(&self, model_scope: Option<&str>, model: &str) {
        let Some(model_scope) = model_scope.filter(|scope| !scope.trim().is_empty()) else {
            return;
        };
        if model.trim().is_empty() {
            return;
        }
        let until = runtime_gemini_now_ms().saturating_add(RUNTIME_GEMINI_MODEL_PREFERENCE_TTL_MS);
        if let Ok(mut state) = self.state.lock() {
            state.remember_selected_model_until(model_scope, model, until);
        }
    }

    pub(in super::super) fn selected_model_for_scope(
        &self,
        model_scope: Option<&str>,
    ) -> Option<String> {
        let model_scope = model_scope.filter(|scope| !scope.trim().is_empty())?;
        let Ok(mut state) = self.state.lock() else {
            return None;
        };
        state.selected_model_for_scope(model_scope, runtime_gemini_now_ms())
    }

    pub(super) fn remember_model_preference(
        &self,
        model_scope: Option<&str>,
        profile_name: &str,
        requested_model: &str,
        model: &str,
    ) {
        let Some(model_scope) = model_scope.filter(|scope| !scope.trim().is_empty()) else {
            return;
        };
        if profile_name.trim().is_empty()
            || requested_model.trim().is_empty()
            || model.trim().is_empty()
        {
            return;
        }
        let until = runtime_gemini_now_ms().saturating_add(RUNTIME_GEMINI_MODEL_PREFERENCE_TTL_MS);
        if let Ok(mut state) = self.state.lock() {
            state.remember_model_preference_until(
                model_scope,
                profile_name,
                requested_model,
                model,
                until,
            );
        }
    }

    pub(in super::super) fn preferred_model_chain_for_profile(
        &self,
        model_scope: Option<&str>,
        profile_name: &str,
        requested_model: &str,
        models: &[String],
    ) -> Vec<String> {
        let Some(model_scope) = model_scope.filter(|scope| !scope.trim().is_empty()) else {
            return models.to_vec();
        };
        let Ok(mut state) = self.state.lock() else {
            return models.to_vec();
        };
        state.preferred_model_chain_for_profile(
            model_scope,
            profile_name,
            requested_model,
            models,
            runtime_gemini_now_ms(),
        )
    }

    pub(in super::super) fn available_model_chain_for_profile(
        &self,
        profile_name: &str,
        endpoint: &str,
        models: &[String],
    ) -> Vec<String> {
        let Ok(state) = self.state.lock() else {
            return models.to_vec();
        };
        state.available_model_chain_for_profile(
            profile_name,
            endpoint,
            models,
            runtime_gemini_now_ms(),
        )
    }

    pub(in super::super) fn select_attempts(
        &self,
        body: &[u8],
        fallback_profiles: &[RuntimeGeminiOAuthProfileAuth],
        model_scope: Option<&str>,
    ) -> Result<Vec<RuntimeGeminiSelectedAuth>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Gemini OAuth pool lock poisoned"))?;
        let profiles = if state.profiles.is_empty() {
            fallback_profiles.to_vec()
        } else {
            state.profiles.clone()
        };
        if profiles.is_empty() {
            bail!("Gemini OAuth pool is empty");
        }
        if let Some(profile_name) = state.affinity_profile_for_body(body)
            && let Some(attempts) = runtime_gemini_oauth_affinity_attempts(&profiles, &profile_name)
        {
            return Ok(attempts);
        }
        if runtime_gemini_sticky_fresh_oauth_enabled()
            && let Some(scope) = model_scope.filter(|scope| scope.starts_with("session:"))
            && let Some(profile_name) = state.session_profile_bindings.get(scope)
            && let Some(attempts) = runtime_gemini_oauth_affinity_attempts(&profiles, profile_name)
        {
            return Ok(attempts);
        }
        let requested_model = runtime_provider_model_from_body(body)
            .unwrap_or_else(|| GEMINI_DEFAULT_MODEL.to_string());
        let model_chain = runtime_provider_model_fallback_chain(
            RuntimeProviderBridgeKind::Gemini,
            &requested_model,
        );
        let endpoint = gemini_code_assist_endpoint();
        let now_ms = runtime_gemini_now_ms();
        let start = state.next_index.min(profiles.len().saturating_sub(1));
        state.next_index = (start + 1) % profiles.len();
        let mut attempts = (0..profiles.len())
            .map(|offset| {
                runtime_gemini_oauth_attempt_from_profile(
                    &profiles[(start + offset) % profiles.len()],
                )
            })
            .filter(|selected| {
                state.profile_has_available_model(
                    &selected.profile_name,
                    &endpoint,
                    &model_chain,
                    now_ms,
                )
            })
            .collect::<Vec<_>>();
        if attempts.is_empty() {
            attempts = (0..profiles.len())
                .map(|offset| {
                    runtime_gemini_oauth_attempt_from_profile(
                        &profiles[(start + offset) % profiles.len()],
                    )
                })
                .collect();
        }
        Ok(attempts)
    }
}

fn runtime_gemini_sticky_fresh_oauth_enabled() -> bool {
    match std::env::var("PRODEX_GEMINI_STICKY_FRESH_OAUTH") {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => true,
    }
}

pub(super) fn runtime_gemini_initial_oauth_pool_index(profile_count: usize) -> usize {
    if profile_count <= 1 {
        return 0;
    }
    let now = runtime_gemini_now_ms() as usize;
    let pid = std::process::id() as usize;
    now.wrapping_add(pid).wrapping_rem(profile_count)
}

fn runtime_gemini_explicit_session_scope(
    request: &RuntimeProxyRequest,
    body: &[u8],
) -> Option<String> {
    runtime_gemini_request_session_id(request, body)
        .map(|session_id| format!("session:{session_id}"))
}

fn runtime_gemini_request_session_id(request: &RuntimeProxyRequest, body: &[u8]) -> Option<String> {
    runtime_proxy_crate::runtime_request_session_id(request).or_else(|| {
        serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .and_then(|value| runtime_proxy_crate::runtime_request_session_id_from_value(&value))
    })
}

pub(super) fn runtime_gemini_binding_recorder(
    pool: &RuntimeGeminiOAuthPool,
    profile_name: String,
    model_scope: Option<String>,
) -> RuntimeGeminiBindingRecorder {
    let pool = pool.clone();
    Arc::new(move |response_id, tool_call_ids| {
        if let Ok(mut state) = pool.state.lock() {
            state.remember_bindings(
                &profile_name,
                model_scope.as_deref(),
                &response_id,
                &tool_call_ids,
            );
        }
    })
}

pub(super) fn runtime_gemini_model_cache_endpoint(
    auth: &RuntimeGeminiAuth,
    upstream_base_url: &str,
) -> String {
    match auth {
        RuntimeGeminiAuth::ApiKey { .. } => upstream_base_url.trim_end_matches('/').to_string(),
        RuntimeGeminiAuth::OAuth { .. } => gemini_code_assist_endpoint(),
    }
}

pub(super) fn runtime_gemini_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

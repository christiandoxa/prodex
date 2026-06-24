use super::super::gemini_rewrite::{
    RuntimeGeminiAuth, RuntimeGeminiOAuthProfileAuth, RuntimeGeminiProviderAuth,
};
use super::super::gemini_sse::RuntimeGeminiBindingRecorder;
use super::super::local_rewrite::{
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared,
};
use super::super::local_rewrite_gemini_bindings::runtime_gemini_tool_output_call_ids_from_request;
use super::super::local_rewrite_rate_limits::runtime_gemini_quota_codex_headers;
use super::super::local_rewrite_transport::runtime_local_rewrite_api_key_attempts;
use super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_model_fallback_chain,
    runtime_provider_model_from_body,
};
use super::local_rewrite_gemini_auth::{
    runtime_gemini_oauth_affinity_attempts, runtime_gemini_oauth_attempt_from_profile,
};
use crate::{
    RuntimeProxyRequest, fetch_gemini_quota_with_code_assist_endpoint, gemini_code_assist_endpoint,
    runtime_proxy_log_to_path, spawn_runtime_background_worker_or_log,
};
use anyhow::{Context, Result, bail};
use prodex_runtime_gemini::GEMINI_DEFAULT_MODEL;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT: usize = 4096;
const RUNTIME_GEMINI_MODEL_UNAVAILABLE_TTL_MS: u64 = 60 * 60 * 1_000;
pub(super) const RUNTIME_GEMINI_MODEL_PREFERENCE_TTL_MS: u64 = 10 * 60 * 1_000;

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
    pub(in super::super) fn spawn_quota_refresh(&self, log_path: PathBuf) {
        let profiles = match self.state.lock() {
            Ok(state) => state.profiles.clone(),
            Err(_) => return,
        };
        if profiles.is_empty() {
            return;
        }
        let pool = self.clone();
        let code_assist_endpoint = gemini_code_assist_endpoint();
        spawn_runtime_background_worker_or_log(
            "prodex-gemini-quota-refresh",
            Some(log_path.clone()),
            move || {
                for profile in profiles {
                    let result = fetch_gemini_quota_with_code_assist_endpoint(
                        &profile.codex_home,
                        profile.project_id.as_deref(),
                        &code_assist_endpoint,
                    );
                    match result {
                        Ok(info) => {
                            let headers = runtime_gemini_quota_codex_headers(
                                &profile.profile_name,
                                profile.email.as_deref(),
                                &info,
                            );
                            pool.remember_quota_headers(&profile.profile_name, headers);
                            runtime_proxy_log_to_path(
                                &log_path,
                                &runtime_proxy_structured_log_message(
                                    "local_rewrite_gemini_quota_status_ready",
                                    [
                                        runtime_proxy_log_field(
                                            "profile",
                                            profile.profile_name.as_str(),
                                        ),
                                        runtime_proxy_log_field(
                                            "buckets",
                                            info.buckets.len().to_string(),
                                        ),
                                    ],
                                ),
                            );
                        }
                        Err(err) => {
                            runtime_proxy_log_to_path(
                                &log_path,
                                &runtime_proxy_structured_log_message(
                                    "local_rewrite_gemini_quota_status_unavailable",
                                    [
                                        runtime_proxy_log_field(
                                            "profile",
                                            profile.profile_name.as_str(),
                                        ),
                                        runtime_proxy_log_field("error", err.to_string()),
                                    ],
                                ),
                            );
                        }
                    }
                }
            },
        );
    }

    pub(in super::super) fn quota_headers_for_profile(
        &self,
        profile_name: &str,
    ) -> Vec<(String, String)> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.quota_headers.get(profile_name).cloned())
            .unwrap_or_default()
    }

    fn remember_quota_headers(&self, profile_name: &str, headers: Vec<(String, String)>) {
        if headers.is_empty() {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            state
                .quota_headers
                .insert(profile_name.to_string(), headers);
        }
    }

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

impl RuntimeGeminiOAuthPoolState {
    pub(in super::super) fn profile_by_name(
        &self,
        profile_name: &str,
    ) -> Option<RuntimeGeminiOAuthProfileAuth> {
        self.profiles
            .iter()
            .find(|profile| profile.profile_name == profile_name)
            .cloned()
    }

    fn affinity_profile_for_body(&self, body: &[u8]) -> Option<String> {
        let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
        if let Some(previous_response_id) = value
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str)
            && let Some(profile_name) = self.response_profile_bindings.get(previous_response_id)
        {
            return Some(profile_name.clone());
        }
        runtime_gemini_tool_output_call_ids_from_request(&value)
            .into_iter()
            .find_map(|call_id| self.tool_call_profile_bindings.get(&call_id).cloned())
    }

    fn model_scope_for_body(&self, body: &[u8]) -> Option<String> {
        let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
        if let Some(previous_response_id) = value
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str)
            && let Some(scope) = self.response_model_scope_bindings.get(previous_response_id)
        {
            return Some(scope.clone());
        }
        runtime_gemini_tool_output_call_ids_from_request(&value)
            .into_iter()
            .find_map(|call_id| self.tool_call_model_scope_bindings.get(&call_id).cloned())
    }

    pub(in super::super) fn remember_bindings(
        &mut self,
        profile_name: &str,
        model_scope: Option<&str>,
        response_id: &str,
        tool_call_ids: &[String],
    ) {
        if !response_id.trim().is_empty() {
            self.response_profile_bindings
                .insert(response_id.to_string(), profile_name.to_string());
            if let Some(model_scope) = model_scope.filter(|scope| !scope.trim().is_empty()) {
                self.response_model_scope_bindings
                    .insert(response_id.to_string(), model_scope.to_string());
                if model_scope.starts_with("session:") {
                    self.session_profile_bindings
                        .insert(model_scope.to_string(), profile_name.to_string());
                }
            }
        }
        for call_id in tool_call_ids {
            if !call_id.trim().is_empty() {
                self.tool_call_profile_bindings
                    .insert(call_id.clone(), profile_name.to_string());
                if let Some(model_scope) = model_scope.filter(|scope| !scope.trim().is_empty()) {
                    self.tool_call_model_scope_bindings
                        .insert(call_id.clone(), model_scope.to_string());
                }
            }
        }
        runtime_gemini_prune_binding_map(
            &mut self.response_profile_bindings,
            RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT,
        );
        runtime_gemini_prune_binding_map(
            &mut self.tool_call_profile_bindings,
            RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT,
        );
        runtime_gemini_prune_binding_map(
            &mut self.session_profile_bindings,
            RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT,
        );
        runtime_gemini_prune_binding_map(
            &mut self.response_model_scope_bindings,
            RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT,
        );
        runtime_gemini_prune_binding_map(
            &mut self.tool_call_model_scope_bindings,
            RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT,
        );
    }

    pub(in super::super) fn remember_model_cooldown_until(
        &mut self,
        profile_name: &str,
        model: &str,
        until_ms: u64,
    ) {
        let key = runtime_gemini_model_cooldown_key(profile_name, model);
        self.model_cooldowns_until.insert(key, until_ms);
        self.model_cooldowns_until
            .retain(|_, cooldown_until| *cooldown_until > runtime_gemini_now_ms());
    }

    pub(in super::super) fn remember_model_unavailable_until(
        &mut self,
        profile_name: &str,
        endpoint: &str,
        model: &str,
        until_ms: u64,
    ) {
        let key = runtime_gemini_model_unavailable_key(profile_name, endpoint, model);
        self.model_unavailable_until.insert(key, until_ms);
        self.model_unavailable_until
            .retain(|_, unavailable_until| *unavailable_until > runtime_gemini_now_ms());
    }

    pub(in super::super) fn remember_model_preference_until(
        &mut self,
        model_scope: &str,
        profile_name: &str,
        requested_model: &str,
        model: &str,
        until_ms: u64,
    ) {
        let key = runtime_gemini_model_preference_key(model_scope, profile_name, requested_model);
        self.model_preferences.insert(
            key,
            RuntimeGeminiModelPreference {
                model: model.to_string(),
                until_ms,
            },
        );
        self.model_preferences
            .retain(|_, preference| preference.until_ms > runtime_gemini_now_ms());
    }

    fn remember_selected_model_until(&mut self, model_scope: &str, model: &str, until_ms: u64) {
        self.selected_model_preferences.insert(
            model_scope.to_string(),
            RuntimeGeminiModelPreference {
                model: model.to_string(),
                until_ms,
            },
        );
        self.selected_model_preferences
            .retain(|_, preference| preference.until_ms > runtime_gemini_now_ms());
    }

    fn selected_model_for_scope(&mut self, model_scope: &str, now_ms: u64) -> Option<String> {
        self.selected_model_preferences
            .retain(|_, preference| preference.until_ms > now_ms);
        self.selected_model_preferences
            .get(model_scope)
            .map(|preference| preference.model.clone())
    }

    fn preferred_model_chain_for_profile(
        &mut self,
        model_scope: &str,
        profile_name: &str,
        requested_model: &str,
        models: &[String],
        now_ms: u64,
    ) -> Vec<String> {
        self.model_preferences
            .retain(|_, preference| preference.until_ms > now_ms);
        let Some(preference) = self
            .model_preferences
            .get(&runtime_gemini_model_preference_key(
                model_scope,
                profile_name,
                requested_model,
            ))
        else {
            return models.to_vec();
        };
        if !models.iter().any(|model| model == &preference.model) {
            return models.to_vec();
        }
        let mut preferred = Vec::with_capacity(models.len());
        preferred.push(preference.model.clone());
        preferred.extend(
            models
                .iter()
                .filter(|model| *model != &preference.model)
                .cloned(),
        );
        preferred
    }

    fn available_model_chain_for_profile(
        &self,
        profile_name: &str,
        endpoint: &str,
        models: &[String],
        now_ms: u64,
    ) -> Vec<String> {
        models
            .iter()
            .filter(|model| self.model_is_available(profile_name, endpoint, model, now_ms))
            .cloned()
            .collect()
    }

    fn profile_has_available_model(
        &self,
        profile_name: &str,
        endpoint: &str,
        models: &[String],
        now_ms: u64,
    ) -> bool {
        models
            .iter()
            .any(|model| self.model_is_available(profile_name, endpoint, model, now_ms))
    }

    fn model_is_available(
        &self,
        profile_name: &str,
        endpoint: &str,
        model: &str,
        now_ms: u64,
    ) -> bool {
        let cooldown_available = self
            .model_cooldowns_until
            .get(&runtime_gemini_model_cooldown_key(profile_name, model))
            .is_none_or(|cooldown_until| *cooldown_until <= now_ms);
        let endpoint_available = self
            .model_unavailable_until
            .get(&runtime_gemini_model_unavailable_key(
                profile_name,
                endpoint,
                model,
            ))
            .is_none_or(|unavailable_until| *unavailable_until <= now_ms);
        cooldown_available && endpoint_available
    }
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

fn runtime_gemini_prune_binding_map(map: &mut BTreeMap<String, String>, limit: usize) {
    while map.len() > limit {
        let Some(key) = map.keys().next().cloned() else {
            break;
        };
        map.remove(&key);
    }
}

fn runtime_gemini_model_cooldown_key(profile_name: &str, model: &str) -> String {
    format!("{profile_name}\0{model}")
}

fn runtime_gemini_model_unavailable_key(profile_name: &str, endpoint: &str, model: &str) -> String {
    format!("{profile_name}\0{endpoint}\0{model}")
}

fn runtime_gemini_model_preference_key(
    model_scope: &str,
    profile_name: &str,
    requested_model: &str,
) -> String {
    format!("{model_scope}\0{profile_name}\0{requested_model}")
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

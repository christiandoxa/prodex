//! Copilot auth attempt ordering and OAuth response affinity bookkeeping.

use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::local_rewrite_copilot_bindings::RuntimeCopilotBindingRecorder;
use super::super::local_rewrite_transport::runtime_local_rewrite_api_key_attempts;
use super::{
    RuntimeCopilotOAuthPool, RuntimeCopilotOAuthPoolState, RuntimeCopilotProfileAuth,
    RuntimeCopilotProviderAuth, RuntimeCopilotSelectedAuth,
};
use anyhow::{Context, Result, bail};
use std::sync::Arc;

const RUNTIME_COPILOT_PROVIDER_BINDING_LIMIT: usize = 4096;

pub(super) fn runtime_copilot_auth_attempts(
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
                    projected: false,
                })
                .collect::<Vec<_>>();
            if attempts.is_empty() {
                bail!("Copilot API-key pool is empty");
            }
            Ok(attempts)
        }
        RuntimeCopilotProviderAuth::Projected => Ok(vec![RuntimeCopilotSelectedAuth {
            profile_name: "projected".to_string(),
            api_key: String::new(),
            api_url: None,
            hard_affinity: true,
            projected: true,
        }]),
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
    pub(super) fn select_attempts(
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
                projected: false,
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
                    projected: false,
                }
            })
            .collect())
    }
}

pub(super) fn runtime_copilot_upstream_base_url<'a>(
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

    pub(super) fn remember_response_binding(&mut self, profile_name: &str, response_id: &str) {
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

pub(super) fn runtime_copilot_binding_recorder(
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

//! Copilot auth attempt ordering and OAuth response affinity bookkeeping.

use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::local_rewrite_copilot_bindings::{
    RuntimeCopilotBindingAcceptance, RuntimeCopilotBindingRecorder,
};
use super::super::local_rewrite_transport::{
    runtime_local_rewrite_api_key_attempts, runtime_local_rewrite_log_url,
};
use super::super::local_rewrite_upstream::runtime_local_rewrite_bound_binding;
use super::state::runtime_copilot_api_key_profiles;
use super::{
    RuntimeCopilotOAuthPool, RuntimeCopilotProfileAuth, RuntimeCopilotProviderAuth,
    RuntimeCopilotSelectedAuth,
};
use crate::RuntimeProxyRequest;
use anyhow::{Context, Result, bail};
use prodex_provider_core::ProviderId;
use prodex_provider_spi::RuntimeProviderBindingIdentity;
use std::sync::Arc;

pub(super) fn runtime_copilot_auth_attempts_for_request(
    auth: &RuntimeCopilotProviderAuth,
    shared: &RuntimeLocalRewriteProxyShared,
    body: &[u8],
    request: &RuntimeProxyRequest,
) -> Result<Vec<RuntimeCopilotSelectedAuth>> {
    let turn_state = runtime_proxy_crate::runtime_request_turn_state(request);
    let session_id = runtime_proxy_crate::runtime_request_session_id(request);
    let fallback_endpoint = runtime_local_rewrite_log_url(&shared.upstream_base_url);
    runtime_copilot_auth_attempts_with_identity(
        auth,
        shared,
        body,
        turn_state.as_deref(),
        session_id.as_deref(),
        &fallback_endpoint,
    )
}

fn runtime_copilot_auth_attempts_with_identity(
    auth: &RuntimeCopilotProviderAuth,
    shared: &RuntimeLocalRewriteProxyShared,
    body: &[u8],
    turn_state: Option<&str>,
    session_id: Option<&str>,
    fallback_endpoint: &str,
) -> Result<Vec<RuntimeCopilotSelectedAuth>> {
    match auth {
        RuntimeCopilotProviderAuth::ApiKeys { api_keys } => {
            if let Some(pool) = shared.copilot_oauth_pool.as_ref() {
                return pool.select_attempts_with_identity(
                    body,
                    &runtime_copilot_api_key_profiles(api_keys),
                    turn_state,
                    session_id,
                    fallback_endpoint,
                );
            }
            if turn_state.is_some()
                || session_id.is_some()
                || runtime_copilot_previous_response_id(body).is_some()
            {
                bail!("Copilot continuation binding is unavailable");
            }
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
        RuntimeCopilotProviderAuth::Projected => {
            if runtime_local_rewrite_bound_binding(
                &shared.runtime_shared.runtime,
                runtime_copilot_previous_response_id(body).as_deref(),
                turn_state,
                session_id,
            )?
            .is_some()
            {
                bail!("Copilot projected continuation binding is unavailable");
            }
            Ok(vec![RuntimeCopilotSelectedAuth {
                profile_name: "projected".to_string(),
                api_key: String::new(),
                api_url: None,
                hard_affinity: true,
                projected: true,
            }])
        }
        RuntimeCopilotProviderAuth::Profiles { profiles } => {
            let pool = shared
                .copilot_oauth_pool
                .as_ref()
                .context("Copilot OAuth pool was not initialized")?;
            pool.select_attempts_with_identity(
                body,
                profiles,
                turn_state,
                session_id,
                fallback_endpoint,
            )
        }
    }
}

impl RuntimeCopilotOAuthPool {
    #[cfg(test)]
    pub(super) fn select_attempts(
        &self,
        body: &[u8],
        fallback_profiles: &[RuntimeCopilotProfileAuth],
    ) -> Result<Vec<RuntimeCopilotSelectedAuth>> {
        self.select_attempts_with_identity(body, fallback_profiles, None, None, "")
    }

    pub(super) fn select_attempts_with_identity(
        &self,
        body: &[u8],
        fallback_profiles: &[RuntimeCopilotProfileAuth],
        turn_state: Option<&str>,
        session_id: Option<&str>,
        fallback_endpoint: &str,
    ) -> Result<Vec<RuntimeCopilotSelectedAuth>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Copilot OAuth pool lock poisoned"))?;
        let profiles = if state.profiles.is_empty() {
            fallback_profiles.to_vec()
        } else {
            state.profiles.clone()
        };
        let response_id = runtime_copilot_previous_response_id(body);
        if let Some(binding) = runtime_local_rewrite_bound_binding(
            &self.runtime,
            response_id.as_deref(),
            turn_state,
            session_id,
        )? {
            let mut matching_profiles = profiles.iter().filter(|profile| {
                if let Some(binding_identity) = binding.binding_identity.as_ref() {
                    let endpoint =
                        runtime_copilot_profile_public_endpoint(profile, fallback_endpoint);
                    runtime_copilot_binding_identity(profile, &endpoint)
                        .is_some_and(|identity| identity == *binding_identity)
                } else {
                    profile.profile_name == binding.profile_name
                }
            });
            let Some(profile) = matching_profiles.next() else {
                bail!("Copilot continuation binding is unavailable");
            };
            if matching_profiles.next().is_some() {
                bail!("Copilot continuation binding is conflicting");
            }
            let current_endpoint =
                runtime_copilot_profile_public_endpoint(profile, fallback_endpoint);
            let current_identity = runtime_copilot_binding_identity(profile, &current_endpoint)
                .ok_or_else(|| anyhow::anyhow!("Copilot continuation binding is unavailable"))?;
            if binding
                .binding_identity
                .as_ref()
                .is_some_and(|identity| identity != &current_identity)
            {
                bail!("Copilot continuation binding is conflicting");
            }
            return Ok(vec![RuntimeCopilotSelectedAuth {
                profile_name: profile.profile_name.clone(),
                api_key: profile.api_key.clone(),
                api_url: (!profile.api_url.trim().is_empty()).then(|| profile.api_url.clone()),
                hard_affinity: true,
                projected: false,
            }]);
        }
        if profiles.is_empty() {
            bail!("Copilot OAuth pool is empty");
        }
        let start = state.next_index.min(profiles.len().saturating_sub(1));
        state.next_index = (start + 1) % profiles.len();
        Ok((0..profiles.len())
            .map(|offset| {
                let profile = profiles[(start + offset) % profiles.len()].clone();
                let is_raw_api_key = profile.api_url.trim().is_empty();
                RuntimeCopilotSelectedAuth {
                    profile_name: profile.profile_name,
                    api_key: profile.api_key,
                    api_url: (!is_raw_api_key).then_some(profile.api_url),
                    hard_affinity: profiles.len() == 1 && is_raw_api_key,
                    projected: false,
                }
            })
            .collect())
    }

    pub(super) fn remember_accepted_identity(
        &self,
        shared: &crate::RuntimeRotationProxyShared,
        selected: &RuntimeCopilotSelectedAuth,
        endpoint: &str,
        response_id: Option<&str>,
        turn_state: Option<&str>,
        session_id: Option<&str>,
    ) {
        let endpoint = runtime_local_rewrite_log_url(endpoint);
        let Some(binding_identity) = runtime_copilot_binding_identity(
            &RuntimeCopilotProfileAuth {
                profile_name: selected.profile_name.clone(),
                api_key: selected.api_key.clone(),
                api_url: selected.api_url.clone().unwrap_or_default(),
                model_catalog: Vec::new(),
            },
            &endpoint,
        ) else {
            return;
        };
        let response_ids = response_id
            .map(str::to_string)
            .into_iter()
            .collect::<Vec<_>>();
        let _ = crate::runtime_proxy::remember_runtime_external_binding_identity(
            shared,
            &selected.profile_name,
            &binding_identity,
            &response_ids,
            turn_state,
            session_id,
        );
    }
}

fn runtime_copilot_profile_public_endpoint(
    profile: &RuntimeCopilotProfileAuth,
    fallback_endpoint: &str,
) -> String {
    let endpoint = if profile.api_url.trim().is_empty() {
        fallback_endpoint
    } else {
        profile.api_url.as_str()
    };
    runtime_local_rewrite_log_url(endpoint)
        .trim_end_matches('/')
        .to_string()
}

fn runtime_copilot_binding_identity(
    profile: &RuntimeCopilotProfileAuth,
    endpoint: &str,
) -> Option<RuntimeProviderBindingIdentity> {
    if profile.api_key.trim().is_empty() {
        RuntimeProviderBindingIdentity::from_profile(
            ProviderId::Copilot,
            &profile.profile_name,
            endpoint,
        )
    } else {
        RuntimeProviderBindingIdentity::from_raw_key(
            ProviderId::Copilot,
            &profile.api_key,
            endpoint,
            Some(&profile.profile_name),
        )
    }
}

pub(super) fn runtime_copilot_public_endpoint(
    shared: &RuntimeLocalRewriteProxyShared,
    selected: &RuntimeCopilotSelectedAuth,
) -> String {
    let endpoint = selected
        .api_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or(shared.upstream_base_url.as_str());
    runtime_local_rewrite_log_url(endpoint)
        .trim_end_matches('/')
        .to_string()
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

pub(super) fn runtime_copilot_previous_response_id(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("previous_response_id")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

pub(super) fn runtime_copilot_binding_acceptance(
    pool: &RuntimeCopilotOAuthPool,
    shared: &crate::RuntimeRotationProxyShared,
    selected: RuntimeCopilotSelectedAuth,
    endpoint: String,
    response_id: Option<String>,
    turn_state: Option<String>,
    session_id: Option<String>,
) -> RuntimeCopilotBindingAcceptance {
    let pool = pool.clone();
    let shared = shared.clone();
    Arc::new(move || {
        pool.remember_accepted_identity(
            &shared,
            &selected,
            &endpoint,
            response_id.as_deref(),
            turn_state.as_deref(),
            session_id.as_deref(),
        );
    })
}

pub(super) fn runtime_copilot_binding_recorder(
    pool: &RuntimeCopilotOAuthPool,
    shared: &crate::RuntimeRotationProxyShared,
    selected: RuntimeCopilotSelectedAuth,
    endpoint: String,
    turn_state: Option<String>,
    session_id: Option<String>,
) -> RuntimeCopilotBindingRecorder {
    let pool = pool.clone();
    let shared = shared.clone();
    Arc::new(move |response_id| {
        pool.remember_accepted_identity(
            &shared,
            &selected,
            &endpoint,
            Some(&response_id),
            turn_state.as_deref(),
            session_id.as_deref(),
        );
    })
}

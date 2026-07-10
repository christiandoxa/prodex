//! Gemini OAuth pool state bookkeeping for affinity bindings and model availability.

use super::super::super::gemini_rewrite::RuntimeGeminiOAuthProfileAuth;
use super::{RuntimeGeminiModelPreference, RuntimeGeminiOAuthPoolState, runtime_gemini_now_ms};
use prodex_provider_core::gemini_provider_core_tool_output_call_ids_from_request;
use std::collections::BTreeMap;

const RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT: usize = 4096;

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

    pub(super) fn affinity_profile_for_body(&self, body: &[u8]) -> Option<String> {
        let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
        if let Some(previous_response_id) = value
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str)
            && let Some(profile_name) = self.response_profile_bindings.get(previous_response_id)
        {
            return Some(profile_name.clone());
        }
        gemini_provider_core_tool_output_call_ids_from_request(&value)
            .into_iter()
            .find_map(|call_id| self.tool_call_profile_bindings.get(&call_id).cloned())
    }

    pub(super) fn model_scope_for_body(&self, body: &[u8]) -> Option<String> {
        let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
        if let Some(previous_response_id) = value
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str)
            && let Some(scope) = self.response_model_scope_bindings.get(previous_response_id)
        {
            return Some(scope.clone());
        }
        gemini_provider_core_tool_output_call_ids_from_request(&value)
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

    pub(super) fn remember_selected_model_until(
        &mut self,
        model_scope: &str,
        model: &str,
        until_ms: u64,
    ) {
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

    pub(super) fn selected_model_for_scope(
        &mut self,
        model_scope: &str,
        now_ms: u64,
    ) -> Option<String> {
        self.selected_model_preferences
            .retain(|_, preference| preference.until_ms > now_ms);
        self.selected_model_preferences
            .get(model_scope)
            .map(|preference| preference.model.clone())
    }

    pub(super) fn preferred_model_chain_for_profile(
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

    pub(super) fn available_model_chain_for_profile(
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

    pub(super) fn profile_has_available_model(
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

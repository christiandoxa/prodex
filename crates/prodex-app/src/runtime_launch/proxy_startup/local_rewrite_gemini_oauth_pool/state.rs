//! Gemini OAuth pool state bookkeeping for affinity bindings and model availability.

use super::super::super::gemini_rewrite::RuntimeGeminiOAuthProfileAuth;
use super::{RuntimeGeminiModelPreference, RuntimeGeminiOAuthPoolState, runtime_gemini_now_ms};
use prodex_mojo_core::gemini_code_assist_policy::{
    GeminiOAuthBindingChoice, GeminiOAuthModelAvailability, GeminiOAuthModelDeadline,
    gemini_oauth_binding_priority, gemini_oauth_model_availability,
    gemini_oauth_preferred_model_chain,
};
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

    pub(in super::super) fn affinity_profile_for_body(&self, body: &[u8]) -> Option<String> {
        self.bound_value_for_body(
            body,
            &self.response_profile_bindings,
            &self.tool_call_profile_bindings,
        )
    }

    pub(in super::super) fn model_scope_for_body(&self, body: &[u8]) -> Option<String> {
        self.bound_value_for_body(
            body,
            &self.response_model_scope_bindings,
            &self.tool_call_model_scope_bindings,
        )
    }

    fn bound_value_for_body(
        &self,
        body: &[u8],
        response_bindings: &BTreeMap<String, String>,
        tool_bindings: &BTreeMap<String, String>,
    ) -> Option<String> {
        let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
        let previous = value
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|id| response_bindings.get(id));
        let tool_ids = gemini_provider_core_tool_output_call_ids_from_request(&value);
        let tool_values = tool_ids
            .iter()
            .map(|call_id| tool_bindings.get(call_id))
            .collect::<Vec<_>>();
        let bound_flags = tool_values.iter().map(Option::is_some).collect::<Vec<_>>();
        let choice = gemini_oauth_binding_priority(previous.is_some(), &bound_flags)
            .expect("compiled Mojo Gemini OAuth binding priority returned invalid output");
        match choice {
            Some(GeminiOAuthBindingChoice::PreviousResponse) => previous.cloned(),
            Some(GeminiOAuthBindingChoice::ToolOutput(index)) => {
                tool_values.get(index).and_then(|value| *value).cloned()
            }
            None => None,
        }
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
        let preference = self
            .model_preferences
            .get(&runtime_gemini_model_preference_key(
                model_scope,
                profile_name,
                requested_model,
            ))
            .map(|entry| entry.model.as_str());
        gemini_oauth_preferred_model_chain(models, preference)
            .expect("compiled Mojo Gemini OAuth model preference plan returned invalid output")
    }

    pub(super) fn available_model_chain_for_profile(
        &self,
        profile_name: &str,
        endpoint: &str,
        models: &[String],
        now_ms: u64,
    ) -> Vec<String> {
        let plan = self.model_availability_plan(profile_name, endpoint, models, now_ms);
        models
            .iter()
            .zip(plan.eligible)
            .filter_map(|(model, available)| available.then_some(model.clone()))
            .collect()
    }

    pub(super) fn profile_has_available_model(
        &self,
        profile_name: &str,
        endpoint: &str,
        models: &[String],
        now_ms: u64,
    ) -> bool {
        self.model_availability_plan(profile_name, endpoint, models, now_ms)
            .any_available
    }

    fn model_availability_plan(
        &self,
        profile_name: &str,
        endpoint: &str,
        models: &[String],
        now_ms: u64,
    ) -> GeminiOAuthModelAvailability {
        let deadlines = models
            .iter()
            .map(|model| GeminiOAuthModelDeadline {
                cooldown_until_ms: self
                    .model_cooldowns_until
                    .get(&runtime_gemini_model_cooldown_key(profile_name, model))
                    .copied()
                    .unwrap_or(0),
                unavailable_until_ms: self
                    .model_unavailable_until
                    .get(&runtime_gemini_model_unavailable_key(
                        profile_name,
                        endpoint,
                        model,
                    ))
                    .copied()
                    .unwrap_or(0),
            })
            .collect::<Vec<_>>();
        gemini_oauth_model_availability(&deadlines, now_ms)
            .expect("compiled Mojo Gemini OAuth availability decision returned invalid output")
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

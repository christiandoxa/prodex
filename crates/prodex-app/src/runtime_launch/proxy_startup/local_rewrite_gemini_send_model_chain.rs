use super::super::super::gemini_rewrite::RuntimeGeminiAuth;
use super::super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::super::super::local_rewrite_gemini_models::runtime_gemini_model_fallback_chain;
use super::super::local_rewrite_gemini_oauth_pool::{
    RUNTIME_GEMINI_MODEL_PREFERENCE_TTL_MS, RuntimeGeminiSelectedAuth,
    runtime_gemini_model_cache_endpoint,
};
use crate::runtime_proxy_log;
use prodex_provider_core::PRODEX_GEMINI_DEFAULT_MODEL as GEMINI_DEFAULT_MODEL;
use prodex_provider_core::provider_gemini_retain_code_assist_models;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};

pub(super) fn runtime_gemini_model_chain_for_selected_auth(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    selected: &RuntimeGeminiSelectedAuth,
    model_scope: Option<&str>,
    requested_model: &str,
    responses_route: bool,
) -> (Vec<String>, String) {
    let mut model_chain = if responses_route {
        runtime_gemini_model_fallback_chain(&shared.provider, requested_model)
    } else {
        vec![GEMINI_DEFAULT_MODEL.to_string()]
    };
    if matches!(selected.auth, RuntimeGeminiAuth::OAuth { .. }) {
        provider_gemini_retain_code_assist_models(&mut model_chain);
        if model_chain.is_empty() {
            model_chain.push(GEMINI_DEFAULT_MODEL.to_string());
        }
    }
    if let Some(pool) = shared.gemini_oauth_pool.as_ref() {
        model_chain = pool.preferred_model_chain_for_profile(
            model_scope,
            &selected.profile_name,
            requested_model,
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
                        runtime_proxy_log_field("models_after", available_chain.len().to_string()),
                    ],
                ),
            );
            model_chain = available_chain;
        }
    }
    (model_chain, model_cache_endpoint)
}

pub(super) fn runtime_gemini_remember_sticky_model_preference(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    selected: &RuntimeGeminiSelectedAuth,
    model_scope: Option<&str>,
    requested_model: &str,
    translated_model: &str,
    model_index: usize,
) {
    if model_index == 0 {
        return;
    }
    let Some(pool) = shared.gemini_oauth_pool.as_ref() else {
        return;
    };
    pool.remember_model_preference(
        model_scope,
        &selected.profile_name,
        requested_model,
        translated_model,
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_sticky_model",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", selected.profile_name.as_str()),
                runtime_proxy_log_field("scope", model_scope.unwrap_or("-")),
                runtime_proxy_log_field("requested_model", requested_model),
                runtime_proxy_log_field("model", translated_model),
                runtime_proxy_log_field(
                    "ttl_ms",
                    RUNTIME_GEMINI_MODEL_PREFERENCE_TTL_MS.to_string(),
                ),
            ],
        ),
    );
}

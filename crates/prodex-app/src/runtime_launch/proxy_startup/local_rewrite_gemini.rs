use super::gemini_rewrite::RuntimeGeminiOAuthProfileAuth;
#[cfg(test)]
use super::gemini_sse::RuntimeGeminiBindingRecorder;
pub(super) use super::local_rewrite_gemini_bindings::runtime_gemini_remember_bindings_from_responses_body;

#[path = "local_rewrite_gemini_auth.rs"]
mod local_rewrite_gemini_auth;
#[path = "local_rewrite_gemini_oauth_pool.rs"]
mod local_rewrite_gemini_oauth_pool;
use local_rewrite_gemini_oauth_pool::RuntimeGeminiSelectedAuth;
pub(super) use local_rewrite_gemini_oauth_pool::{
    RuntimeGeminiOAuthPool, RuntimeGeminiRequestContext, runtime_gemini_oauth_pool_from_provider,
};
#[cfg(test)]
use local_rewrite_gemini_oauth_pool::{
    RuntimeGeminiOAuthPoolState, runtime_gemini_initial_oauth_pool_index, runtime_gemini_now_ms,
};
#[path = "local_rewrite_gemini_openai.rs"]
mod local_rewrite_gemini_openai;
#[path = "local_rewrite_gemini_precommit.rs"]
mod local_rewrite_gemini_precommit;
#[cfg(test)]
use local_rewrite_gemini_precommit::{
    RuntimeGeminiPrecommitDecision, RuntimeGeminiPrecommitProbe,
    runtime_gemini_precommit_decision_for_data_lines,
};
#[path = "local_rewrite_gemini_send.rs"]
mod local_rewrite_gemini_send;
pub(super) use local_rewrite_gemini_send::send_runtime_gemini_upstream_request;

pub(super) fn runtime_gemini_request_validation_error(body: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    prodex_provider_core::gemini_provider_core_validate_candidate_count(&value)
        .err()
        .or_else(|| {
            prodex_provider_core::gemini_provider_core_validate_request_tools_with_nested_function_paths(
                &value,
            )
            .err()
        })
}
#[cfg(test)]
#[path = "local_rewrite_gemini_precommit_tests.rs"]
mod local_rewrite_gemini_precommit_tests;

#[cfg(test)]
#[path = "local_rewrite_gemini_tests.rs"]
mod local_rewrite_gemini_tests;

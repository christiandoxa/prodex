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
        .or_else(|| runtime_gemini_validate_request_tools(&value).err())
}

fn runtime_gemini_validate_request_tools(value: &serde_json::Value) -> Result<(), String> {
    let Some(tools) = value.get("tools") else {
        return Ok(());
    };
    let Some(tools) = tools.as_array() else {
        return prodex_provider_core::gemini_provider_core_validate_request_tools(value);
    };
    let tools = tools
        .iter()
        .map(|tool| {
            let Some(object) = tool.as_object() else {
                return tool.clone();
            };
            let function_like = object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|tool_type| tool_type == "function")
                || object.contains_key("function");
            if !function_like || object.contains_key("function") {
                return tool.clone();
            }
            let mut function = serde_json::Map::new();
            for field in ["name", "description", "parameters"] {
                if let Some(value) = object.get(field) {
                    function.insert(field.to_string(), value.clone());
                }
            }
            let mut normalized = object.clone();
            normalized.insert("function".to_string(), serde_json::Value::Object(function));
            serde_json::Value::Object(normalized)
        })
        .collect();
    let mut normalized = value
        .as_object()
        .cloned()
        .expect("tools lookup requires a JSON object");
    normalized.insert("tools".to_string(), serde_json::Value::Array(tools));
    prodex_provider_core::gemini_provider_core_validate_request_tools(&serde_json::Value::Object(
        normalized,
    ))
}
#[cfg(test)]
#[path = "local_rewrite_gemini_precommit_tests.rs"]
mod local_rewrite_gemini_precommit_tests;

#[cfg(test)]
#[path = "local_rewrite_gemini_tests.rs"]
mod local_rewrite_gemini_tests;

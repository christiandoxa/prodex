use super::gemini_rewrite::RuntimeGeminiOAuthProfileAuth;
pub(super) use super::gemini_rewrite::runtime_gemini_generate_request_body;
#[cfg(test)]
use super::gemini_sse::RuntimeGeminiBindingRecorder;
use super::local_rewrite::RuntimeLocalRewriteProxyShared;
pub(super) use super::local_rewrite_gemini_bindings::runtime_gemini_remember_bindings_from_responses_body;
use anyhow::Result;
use prodex_provider_core::gemini_provider_core_should_rotate_after_quota_response as runtime_gemini_should_rotate_after_quota_response;

#[path = "local_rewrite_gemini_auth.rs"]
mod local_rewrite_gemini_auth;
#[path = "local_rewrite_gemini_oauth_pool.rs"]
mod local_rewrite_gemini_oauth_pool;
pub(super) use local_rewrite_gemini_oauth_pool::runtime_gemini_live_auth_attempts;
use local_rewrite_gemini_oauth_pool::{
    RUNTIME_GEMINI_MODEL_PREFERENCE_TTL_MS, RuntimeGeminiSelectedAuth,
    runtime_gemini_auth_attempts, runtime_gemini_binding_recorder,
    runtime_gemini_model_cache_endpoint,
};
pub(super) use local_rewrite_gemini_oauth_pool::{
    RuntimeGeminiOAuthPool, RuntimeGeminiRequestContext, runtime_gemini_oauth_pool_from_provider,
};
#[cfg(test)]
use local_rewrite_gemini_oauth_pool::{
    RuntimeGeminiOAuthPoolState, runtime_gemini_initial_oauth_pool_index, runtime_gemini_now_ms,
};
#[path = "local_rewrite_gemini_openai.rs"]
mod local_rewrite_gemini_openai;
use local_rewrite_gemini_openai::send_runtime_gemini_openai_compatible_request;
#[path = "local_rewrite_gemini_precommit.rs"]
mod local_rewrite_gemini_precommit;
#[cfg(test)]
use local_rewrite_gemini_precommit::{
    RuntimeGeminiPrecommitDecision, RuntimeGeminiPrecommitProbe,
    runtime_gemini_precommit_decision_for_data_lines,
};
use local_rewrite_gemini_precommit::{
    RuntimeGeminiPrecommitPeek, runtime_gemini_peek_stream_for_retry,
    runtime_gemini_response_is_sse,
};
#[path = "local_rewrite_gemini_send.rs"]
mod local_rewrite_gemini_send;
pub(super) use local_rewrite_gemini_send::send_runtime_gemini_upstream_request;
#[cfg(test)]
mod tests {
    use super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
    use super::runtime_gemini_generate_request_body;
    use prodex_provider_core::{
        ProviderEndpoint, ProviderId, ProviderTransformInput, gemini_provider_core_request_body,
        gemini_provider_core_simple_request, provider_translator,
    };

    fn conversation_store() -> super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore {
        RuntimeDeepSeekConversationStore::default()
    }

    #[test]
    fn gemini_provider_core_body_overrides_legacy_simple_history_translation() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": [
                {"role":"user","content":"find it"},
                {
                    "role":"assistant",
                    "content":"",
                    "tool_calls":[
                        {
                            "id":"call_1",
                            "type":"function",
                            "function":{
                                "name":"grep",
                                "arguments":"{\"pattern\":\"x\"}"
                            }
                        }
                    ]
                },
                {
                    "role":"tool",
                    "tool_call_id":"call_1",
                    "content":"{\"match_count\":1}"
                }
            ]
        }))
        .unwrap();
        assert!(gemini_provider_core_simple_request(&body));

        let translated =
            runtime_gemini_generate_request_body(&body, &conversation_store(), true, None, None)
                .unwrap();
        let result = provider_translator(ProviderId::Gemini).transform_request(
            ProviderTransformInput::new(ProviderEndpoint::Responses, body),
        );
        let merged = gemini_provider_core_request_body(&result, &translated.body).unwrap();

        let provider_core_json: serde_json::Value = serde_json::from_slice(&merged).unwrap();
        let app_json: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        assert_ne!(provider_core_json, app_json);
        assert_eq!(
            provider_core_json["request"]["contents"][0]["parts"][0]["text"],
            "find it"
        );
        assert_eq!(
            provider_core_json["request"]["contents"][1]["parts"][0]["functionCall"]["name"],
            "grep"
        );
        assert_eq!(
            provider_core_json["request"]["contents"][2]["parts"][0]["functionResponse"]["name"],
            "grep"
        );
        assert!(
            provider_core_json["request"]
                .get("systemInstruction")
                .is_none()
        );
        assert!(app_json["request"].get("systemInstruction").is_some());
    }
}

#[cfg(test)]
#[path = "local_rewrite_gemini_precommit_tests.rs"]
mod local_rewrite_gemini_precommit_tests;

#[cfg(test)]
#[path = "local_rewrite_gemini_tests.rs"]
mod local_rewrite_gemini_tests;

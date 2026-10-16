use super::*;

#[test]
fn provider_scalar_policy_preserves_reasoning_and_copilot_limits() {
    assert_eq!(
        provider_reasoning_effort_class("\u{2003}ULTRA\u{2003}").unwrap(),
        ProviderReasoningEffortClass::Ultra
    );
    assert_eq!(
        provider_reasoning_effort_class("unknown-effort").unwrap(),
        ProviderReasoningEffortClass::Unknown
    );
    assert_eq!(provider_reasoning_effort_label(0).unwrap(), Some("none"));
    assert_eq!(provider_reasoning_effort_label(5).unwrap(), Some("xhigh"));
    assert_eq!(provider_reasoning_effort_label(7).unwrap(), Some("ultra"));
    assert_eq!(provider_reasoning_effort_label(8).unwrap(), None);
    assert_eq!(provider_id_label(0).unwrap(), "openai");
    assert_eq!(provider_id_label(6).unwrap(), "local");
    assert_eq!(
        provider_wire_format_label(1).unwrap(),
        "openai-chat-completions"
    );
    assert_eq!(provider_endpoint_label(1).unwrap(), "responses/compact");
    assert_eq!(provider_endpoint_label(10).unwrap(), "a2a");
    assert_eq!(provider_capability_status_label(4).unwrap(), "partial");
    assert_eq!(
        provider_copilot_prompt_token_limit("GPT-5.3-CODEX").unwrap(),
        Some(272_000)
    );
    assert_eq!(
        provider_copilot_prompt_token_limit(" claude-opus-4.8 ").unwrap(),
        Some(936_000)
    );
    assert_eq!(
        provider_copilot_prompt_token_limit("gpt-5.4-nano").unwrap(),
        Some(128_000)
    );
    assert_eq!(
        provider_copilot_prompt_token_limit("unknown-model").unwrap(),
        None
    );
    assert_eq!(provider_boolean_token(" YES ").unwrap(), Some(true));
    assert_eq!(
        provider_boolean_token("\u{2003}off\u{2003}").unwrap(),
        Some(false)
    );
    assert_eq!(provider_boolean_token("maybe").unwrap(), None);
    assert_eq!(
        provider_retry_plan(1, 1, 0, 4, 0).unwrap(),
        ProviderRetryScalarPlan {
            decision: 0,
            remaining_precommit_retries: 1,
        }
    );
    assert_eq!(provider_retry_plan(1, 2, 0, 4, 0).unwrap().decision, 1);
    assert_eq!(provider_retry_plan(1, 1, 0, 0, 0).unwrap().decision, 3);
    assert_eq!(provider_retry_plan(1, 1, 0, 4, 1).unwrap().decision, 2);
    assert_eq!(provider_retry_transition(4, 0, 2, 0, 2, true).unwrap(), 1);
    assert_eq!(provider_retry_transition(0, 1, 2, 0, 2, true).unwrap(), 2);
    assert_eq!(provider_retry_transition(3, 1, 2, 1, 2, true).unwrap(), 0);
    assert_eq!(provider_retry_transition(4, 0, 2, 0, 2, false).unwrap(), 0);
    assert_eq!(
        provider_precommit_buffered_fallback_class(429, 2, false).unwrap(),
        None
    );
    assert_eq!(
        provider_precommit_buffered_fallback_class(429, 2, true).unwrap(),
        Some(2)
    );
    assert_eq!(
        provider_precommit_live_fallback_class(ProviderPrecommitLiveProgress::QuotaBlocked, 5,)
            .unwrap(),
        Some(1)
    );
    assert!(provider_precommit_should_prefetch(4, false, true, 200, true, true).unwrap());
    assert!(!provider_precommit_should_prefetch(1, false, true, 200, true, true).unwrap());
    assert!(provider_precommit_native_first_should_prefetch(true, 200, true, true).unwrap());
    assert!(!provider_precommit_native_first_should_prefetch(false, 200, true, true).unwrap());
    assert!(!provider_precommit_native_first_should_prefetch(true, 400, true, true).unwrap());
    assert!(provider_precommit_openai_credential_retry(401, false, false, true).unwrap());
    assert!(provider_precommit_openai_credential_retry(429, true, false, true).unwrap());
    assert!(!provider_precommit_openai_credential_retry(429, false, false, true).unwrap());
    assert!(!provider_precommit_openai_credential_retry(401, false, true, true).unwrap());
    assert_eq!(
        provider_precommit_sse_action(false, false, false, false, true).unwrap(),
        ProviderPrecommitSseAction::None
    );
    assert_eq!(
        provider_precommit_sse_action(false, false, false, false, false).unwrap(),
        ProviderPrecommitSseAction::Commit
    );
    assert_eq!(
        provider_precommit_health_action(false, 0, None).unwrap(),
        ProviderPrecommitHealthAction::TransportFailure
    );
    assert_eq!(
        provider_precommit_health_action(true, 503, None).unwrap(),
        ProviderPrecommitHealthAction::Overload
    );
    assert_eq!(
        provider_precommit_metric_class(true, 429, Some(2)).unwrap(),
        ProviderPrecommitMetricClass::RateLimited
    );
    assert_eq!(
        provider_bridge_rate_limit_header_prefix(3).unwrap(),
        "deepseek"
    );
    assert_eq!(
        provider_bridge_rate_limit_header_label(4).unwrap(),
        "Google Gemini"
    );
    assert_eq!(
        provider_bridge_chat_compatible_adapter_label(4).unwrap(),
        "Gemini OpenAI-compatible"
    );
    assert_eq!(
        provider_bridge_function_tool_name_max_bytes(2).unwrap(),
        128
    );
    assert_eq!(provider_bridge_function_tool_name_max_bytes(3).unwrap(), 64);
    assert!(provider_bridge_native_passthrough(2, 0, 1).unwrap());
    assert!(!provider_bridge_native_passthrough(2, 1, 0).unwrap());
    assert!(!provider_bridge_native_passthrough(3, 5, 0).unwrap());
    assert!(provider_bridge_native_passthrough(3, 2, 0).unwrap());
    assert!(provider_bridge_native_passthrough(3, 2, 2).unwrap());
    assert!(!provider_bridge_native_passthrough(3, 2, 1).unwrap());
    assert!(provider_bridge_native_passthrough(5, -1, -1).unwrap());
}

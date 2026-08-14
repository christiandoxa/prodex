use super::*;
use prodex_provider_core::{ProviderId, ProviderModelChoice};

#[test]
fn provider_default_efforts_follow_catalog_for_every_provider() {
    for provider in canonical_sub_agent_providers() {
        let choices = canonical_sub_agent_model_choices(*provider, None);
        assert_eq!(choices.first(), Some(&ProviderModelChoice::ProviderDefault));
        assert_eq!(choices.last(), Some(&ProviderModelChoice::Custom));
        for entry in prodex_provider_core::provider_catalog_entries_for(*provider) {
            assert!(
                choices.contains(&ProviderModelChoice::Model(entry.id.clone())),
                "{} picker omitted {}",
                provider.label(),
                entry.id
            );
        }
        assert!(!canonical_sub_agent_efforts(*provider, None).is_empty());
    }
    assert_eq!(
        canonical_sub_agent_efforts(ProviderId::Copilot, None),
        [
            SubAgentReasoningEffort::None,
            SubAgentReasoningEffort::Low,
            SubAgentReasoningEffort::Medium,
            SubAgentReasoningEffort::High,
            SubAgentReasoningEffort::XHigh,
        ]
    );
    assert_eq!(
        canonical_sub_agent_efforts(ProviderId::Kiro, None),
        [
            SubAgentReasoningEffort::Low,
            SubAgentReasoningEffort::Medium,
            SubAgentReasoningEffort::High,
            SubAgentReasoningEffort::XHigh,
            SubAgentReasoningEffort::Max,
        ]
    );
    assert_eq!(
        canonical_sub_agent_efforts(ProviderId::Kiro, Some("gpt-5.6-luna")),
        [
            SubAgentReasoningEffort::None,
            SubAgentReasoningEffort::Low,
            SubAgentReasoningEffort::Medium,
            SubAgentReasoningEffort::High,
            SubAgentReasoningEffort::XHigh,
            SubAgentReasoningEffort::Max,
        ]
    );
}

#[test]
fn test_env_var_guard_restores_previous_value_and_supports_nested_reentry() {
    let key = "PRODEX_TEST_ENV_GUARD_REENTRY";
    let previous = env::var_os(key);

    {
        let _outer = TestEnvVarGuard::set(key, "outer");
        assert_eq!(env::var(key).ok().as_deref(), Some("outer"));

        {
            let _inner = TestEnvVarGuard::set(key, "inner");
            assert_eq!(env::var(key).ok().as_deref(), Some("inner"));
        }

        assert_eq!(env::var(key).ok().as_deref(), Some("outer"));
    }

    assert_eq!(env::var_os(key), previous);
}

use crate::runtime_governance::compile_runtime_governance_artifact;
use prodex_runtime_policy::{
    RuntimeGovernancePolicyAction, RuntimeGovernancePolicyChannel, RuntimeGovernancePolicyEffect,
    RuntimeGovernancePolicyNetworkZone, RuntimeGovernancePolicyObligation,
    RuntimeGovernancePolicyRule, RuntimeGovernancePolicyRuleCondition,
    RuntimePolicyGovernanceSettings,
};

#[test]
fn rejects_non_api_channel_selectors() {
    for channel in [
        RuntimeGovernancePolicyChannel::Cli,
        RuntimeGovernancePolicyChannel::Ide,
        RuntimeGovernancePolicyChannel::Mcp,
        RuntimeGovernancePolicyChannel::InternalService,
    ] {
        let settings = RuntimePolicyGovernanceSettings {
            policy_rules: vec![RuntimeGovernancePolicyRule {
                id: "deny-non-api".to_string(),
                condition: RuntimeGovernancePolicyRuleCondition {
                    channel: Some(channel),
                    ..Default::default()
                },
                effect: RuntimeGovernancePolicyEffect::Deny,
                obligations: Vec::new(),
                reason_code: "policy.non_api".to_string(),
            }],
            ..Default::default()
        };

        assert!(
            compile_runtime_governance_artifact(&serde_json::to_vec(&settings).unwrap()).is_err()
        );
    }
}

#[test]
fn serialized_policy_artifact_rejects_unavailable_gateway_evidence() {
    let rule = || RuntimeGovernancePolicyRule {
        id: "unavailable-evidence".to_string(),
        condition: Default::default(),
        effect: RuntimeGovernancePolicyEffect::Allow,
        obligations: Vec::new(),
        reason_code: "policy.unavailable_evidence".to_string(),
    };
    let compile = |rule| {
        compile_runtime_governance_artifact(
            &serde_json::to_vec(&RuntimePolicyGovernanceSettings {
                policy_rules: vec![rule],
                ..Default::default()
            })
            .unwrap(),
        )
    };

    let mut unsupported = rule();
    unsupported.condition.minimum_authentication_strength = Some(2);
    assert!(compile(unsupported).is_err());

    let mut unsupported = rule();
    unsupported.condition.action = Some(RuntimeGovernancePolicyAction::UseTool);
    assert!(compile(unsupported).is_err());

    let mut unsupported = rule();
    unsupported.condition.network_zone = Some(RuntimeGovernancePolicyNetworkZone::Partner);
    assert!(compile(unsupported).is_err());

    let mut unsupported = rule();
    unsupported.obligations = vec![RuntimeGovernancePolicyObligation::RequireMfa];
    assert!(compile(unsupported).is_err());
}

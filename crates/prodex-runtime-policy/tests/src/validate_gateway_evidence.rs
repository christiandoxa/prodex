use super::validate_runtime_policy_file;
use crate::{
    MAX_GATEWAY_GUARDRAIL_KEYWORD_BYTES, MAX_GATEWAY_GUARDRAIL_KEYWORDS,
    RuntimeGovernancePolicyObligation, RuntimePolicyFile,
};
use std::path::Path;

fn parse_policy(input: &str) -> RuntimePolicyFile {
    toml::from_str(input).expect("policy TOML should parse")
}

#[test]
fn governance_policy_accepts_verified_authentication_evidence_only() {
    let base = parse_policy(
        r#"
version = 1
[[governance.policy_rules]]
id = "allow.api"
effect = "allow"
obligations = []
reason_code = "policy.allow"
[governance.policy_rules.condition]
"#,
    );

    let mut policy = base.clone();
    policy.governance.policy_rules[0]
        .condition
        .minimum_authentication_strength = Some(2);
    policy.governance.policy_rules[0].obligations = vec![
        RuntimeGovernancePolicyObligation::MinimumAuthenticationStrength { value: 2 },
        RuntimeGovernancePolicyObligation::RequireMfa,
        RuntimeGovernancePolicyObligation::RequireReauthentication,
    ];
    validate_runtime_policy_file(&policy, Path::new("policy.toml")).unwrap();

    let mut policy = base.clone();
    policy.governance.policy_rules[0]
        .condition
        .minimum_authentication_strength = Some(4);
    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("out-of-range authentication strength must be rejected");
    assert!(error.to_string().contains("between 1 and 3"));

    let mut policy = base;
    policy.governance.policy_rules[0].obligations =
        vec![RuntimeGovernancePolicyObligation::MinimumAuthenticationStrength { value: 4 }];
    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("out-of-range authentication obligation must be rejected");
    assert!(error.to_string().contains("between 1 and 3"));
}

#[test]
fn governance_policy_schema_rejects_unavailable_evidence_selectors() {
    for selector in [
        "action = \"use_tool\"",
        "action = \"mutate_control_plane\"",
        "network_zone = \"partner\"",
    ] {
        let input = format!(
            r#"
version = 1
[[governance.policy_rules]]
id = "allow.api"
effect = "allow"
obligations = []
reason_code = "policy.allow"
[governance.policy_rules.condition]
{selector}
"#
        );
        assert!(
            toml::from_str::<RuntimePolicyFile>(&input).is_err(),
            "unavailable selector should not be part of the public schema: {selector}"
        );
    }
}

#[test]
fn guardrail_keywords_are_bounded() {
    let mut policy = parse_policy("version = 1");
    policy.gateway.guardrails.blocked_output_keywords =
        vec!["x".repeat(MAX_GATEWAY_GUARDRAIL_KEYWORD_BYTES + 1)];
    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("oversized output keyword should be rejected");
    assert!(error.to_string().contains("must be at most"), "{error:#}");

    let mut policy = parse_policy("version = 1");
    policy.gateway.guardrails.blocked_keywords =
        vec!["x".to_string(); MAX_GATEWAY_GUARDRAIL_KEYWORDS + 1];
    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("unbounded keyword list should be rejected");
    assert!(
        error.to_string().contains("must contain at most"),
        "{error:#}"
    );
}

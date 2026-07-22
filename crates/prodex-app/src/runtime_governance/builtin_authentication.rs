use anyhow::Result;
use prodex_domain::{
    DataClassification, GovernanceObligation, GovernancePolicyRule, PolicyEffect,
    PolicyRuleCondition, PrincipalKind,
};

use super::{runtime_governance_condition, runtime_governance_rule};

pub(super) fn runtime_builtin_authentication_rules() -> Result<Vec<GovernancePolicyRule>> {
    let mut rules = vec![runtime_governance_rule(
        "builtin.restricted-user-authentication",
        PolicyRuleCondition {
            principal_kind: Some(PrincipalKind::User),
            minimum_classification: Some(DataClassification::Restricted),
            ..runtime_governance_condition()
        },
        PolicyEffect::Allow,
        vec![
            GovernanceObligation::MinimumAuthenticationStrength(2),
            GovernanceObligation::RequireMfa,
            GovernanceObligation::RequireReauthentication,
        ],
        "policy.restricted_user_authentication",
    )?];
    for (id, principal_kind) in [
        (
            "builtin.restricted-service-authentication",
            PrincipalKind::ServiceAccount,
        ),
        (
            "builtin.restricted-virtual-key-authentication",
            PrincipalKind::VirtualKey,
        ),
        (
            "builtin.restricted-break-glass-authentication",
            PrincipalKind::BreakGlass,
        ),
    ] {
        rules.push(runtime_governance_rule(
            id,
            PolicyRuleCondition {
                principal_kind: Some(principal_kind),
                minimum_classification: Some(DataClassification::Restricted),
                ..runtime_governance_condition()
            },
            PolicyEffect::Allow,
            vec![GovernanceObligation::MinimumAuthenticationStrength(3)],
            "policy.restricted_strong_authentication",
        )?);
    }
    Ok(rules)
}

#![no_main]

use libfuzzer_sys::fuzz_target;
use prodex_domain::{
    CanonicalRoute, CapabilitySet, Channel, CredentialScope, DataClassification,
    DataPolicyContext, EnvironmentContext, GovernanceObligation, GovernancePolicyArtifact,
    GovernancePolicyRule, GovernancePolicyRuleId, GovernedAction, InspectionCoverage, NetworkZone,
    PolicyEffect, PolicyInput, PolicyReasonCode, PolicyRevisionId, PolicyRuleCondition, Principal,
    PrincipalId, PrincipalKind, PrincipalPolicyAttributes, QuotaContext, RequestPolicyAttributes,
    RequestRisk, Role, SessionPolicyContext, TenantContext, TenantId, compile_governance_policy,
    evaluate_governance_policy,
};

const MAX_FUZZ_BYTES: usize = 4096;

fuzz_target!(|input: &[u8]| {
    if input.len() > MAX_FUZZ_BYTES {
        return;
    }
    let rule_count = input
        .first()
        .copied()
        .map_or(0, |value| usize::from(value) + 1)
        .min(prodex_domain::MAX_GOVERNANCE_POLICY_RULES);
    let rules = (0..rule_count)
        .map(|index| {
            let selector = input.get(index + 1).copied().unwrap_or_default();
            let matching = index < 31 && selector & 1 == 0;
            let effect = match selector % 3 {
                0 => PolicyEffect::Allow,
                1 => PolicyEffect::Deny,
                _ => PolicyEffect::RequireApproval,
            };
            GovernancePolicyRule {
                id: GovernancePolicyRuleId::new(format!("fuzz.rule.{index}")).unwrap(),
                condition: PolicyRuleCondition {
                    route: (!matching).then(|| CanonicalRoute::new("nonmatching").unwrap()),
                    ..Default::default()
                },
                effect,
                obligations: (effect == PolicyEffect::Allow)
                    .then_some(vec![GovernanceObligation::RequireResponseInspection])
                    .unwrap_or_default(),
                reason_code: PolicyReasonCode::new(format!("fuzz.reason.{index}")).unwrap(),
            }
        })
        .collect();
    let Ok(policy) = compile_governance_policy(GovernancePolicyArtifact {
        revision: PolicyRevisionId::new(),
        valid_until_unix_ms: u64::MAX,
        default_effect: PolicyEffect::Deny,
        rules,
    }) else {
        return;
    };
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant.tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::DataPlane,
    );
    let route = CanonicalRoute::new("responses").unwrap();
    let capabilities = CapabilitySet::new(Vec::new());
    let principal_attributes = PrincipalPolicyAttributes::new(None, None, None).unwrap();
    let request_attributes = RequestPolicyAttributes::new(None, &[], Vec::new(), None, 0).unwrap();
    let policy_input = PolicyInput {
        tenant,
        principal: &principal,
        principal_attributes: &principal_attributes,
        channel: Channel::Api,
        credential_scope: CredentialScope::DataPlane,
        session: SessionPolicyContext {
            age_seconds: 0,
            idle_seconds: 0,
            revoked: false,
            mfa_satisfied: false,
            retained_classification: DataClassification::Internal,
        },
        action: GovernedAction::InvokeModel,
        route: &route,
        data: DataPolicyContext {
            classification: DataClassification::Internal,
            inspection_coverage: InspectionCoverage::Full,
        },
        request_risk: RequestRisk::Low,
        requested_capabilities: &capabilities,
        request_attributes: &request_attributes,
        quota: QuotaContext {
            has_headroom: true,
            reservation_required: true,
        },
        environment: EnvironmentContext {
            network_zone: NetworkZone::Unknown,
            authentication_strength: 1,
            mfa_satisfied: false,
            reauthentication_satisfied: false,
        },
    };
    let first = evaluate_governance_policy(&policy, &policy_input);
    let second = evaluate_governance_policy(&policy, &policy_input);
    assert_eq!(first, second);
    if let Ok(decision) = first
        && decision.effect == PolicyEffect::Deny
    {
        assert!(decision.obligations.is_empty());
    }
});

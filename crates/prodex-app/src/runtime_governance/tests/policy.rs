use crate::runtime_governance::{
    RuntimeGovernanceAuthoritySnapshot, compile_runtime_governance_artifact,
};
use prodex_application::{
    ApplicationGovernanceRequest, ApplicationInspectionPlan, plan_application_governance,
};
use prodex_domain::{
    CanonicalRoute, CapabilitySet, Channel, CredentialScope, DataClassification,
    DetectorRevisionId, EnvironmentContext, GovernedAction, InspectionCoverage, InspectionLimits,
    InspectionResult, NetworkZone, PolicyEffect, Principal, PrincipalId, PrincipalKind,
    PrincipalPolicyAttributes, QuotaContext, RequestPolicyAttributes, RequestRisk, Role,
    SessionPolicyContext, TenantContext, TenantId,
};
use prodex_runtime_policy::{
    RuntimeGovernancePolicyAction, RuntimeGovernancePolicyChannel, RuntimeGovernancePolicyEffect,
    RuntimeGovernancePolicyNetworkZone, RuntimeGovernancePolicyObligation,
    RuntimeGovernancePolicyRule, RuntimeGovernancePolicyRuleCondition,
    RuntimePolicyGovernanceSettings,
};

pub(super) fn policy_effect(
    snapshot: &RuntimeGovernanceAuthoritySnapshot,
    tenant_id: TenantId,
) -> PolicyEffect {
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::DataPlane,
    );
    let inspection = ApplicationInspectionPlan {
        result: InspectionResult::new(
            InspectionCoverage::Unsupported,
            DataClassification::Internal,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            DetectorRevisionId::new("detector-v1").unwrap(),
            InspectionLimits::default(),
        )
        .unwrap(),
        masked_findings: Vec::new(),
    };
    let route = CanonicalRoute::new("responses").unwrap();
    let capabilities = CapabilitySet::new(Vec::new());
    let principal_attributes = PrincipalPolicyAttributes::default();
    let request_attributes = RequestPolicyAttributes::default();
    plan_application_governance(
        &snapshot.application,
        ApplicationGovernanceRequest {
            inspection: &inspection,
            trusted_label: None,
            untrusted_label: None,
            prior_classification: None,
            session_floor: DataClassification::Public,
            route_floor: DataClassification::Public,
            request_risk_floor: DataClassification::Public,
            tenant: TenantContext { tenant_id },
            principal: &principal,
            principal_attributes: &principal_attributes,
            channel: Channel::Api,
            credential_scope: CredentialScope::DataPlane,
            session: SessionPolicyContext {
                age_seconds: 0,
                idle_seconds: 0,
                revoked: false,
                mfa_satisfied: false,
                retained_classification: DataClassification::Public,
            },
            action: GovernedAction::InvokeModel,
            route: &route,
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
            },
        },
    )
    .unwrap()
    .policy
    .effect
}

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

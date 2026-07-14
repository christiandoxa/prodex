use prodex_application::{
    ApplicationGovernancePlan, ApplicationGovernanceRequest, ApplicationGovernanceSnapshot,
    ApplicationInspectionPlan, plan_application_governance,
};
use prodex_domain::{
    CanonicalRoute, CapabilitySet, Channel, ClassificationRuleSet, ClassificationRuleSetChecksum,
    ClassificationRuleSetRevisionId, CredentialScope, DataClassification, EnvironmentContext,
    GovernancePolicyArtifact, GovernedAction, NetworkZone, PolicyEffect, PolicyRevisionId,
    Principal, PrincipalPolicyAttributes, QuotaContext, RequestPolicyAttributes, RequestRisk,
    SessionPolicyContext, TenantContext, TenantId, compile_classification_rule_set,
    compile_governance_policy,
};

pub(super) fn test_governance_plan(
    tenant_id: TenantId,
    principal: &Principal,
    inspection: &ApplicationInspectionPlan,
) -> ApplicationGovernancePlan {
    let snapshot = ApplicationGovernanceSnapshot {
        classification_rules: compile_classification_rule_set(ClassificationRuleSet {
            revision: ClassificationRuleSetRevisionId::new("test-v1").unwrap(),
            checksum: ClassificationRuleSetChecksum::new("test-v1").unwrap(),
            unsupported_coverage_floor: DataClassification::Internal,
            rules: Vec::new(),
        })
        .unwrap(),
        policy: compile_governance_policy(GovernancePolicyArtifact {
            revision: PolicyRevisionId::new(),
            valid_until_unix_ms: u64::MAX,
            default_effect: PolicyEffect::Allow,
            rules: Vec::new(),
        })
        .unwrap(),
    };
    let route = CanonicalRoute::new("responses").unwrap();
    let capabilities = CapabilitySet::new(Vec::new());
    let principal_attributes = PrincipalPolicyAttributes::default();
    let request_attributes = RequestPolicyAttributes::default();
    plan_application_governance(
        &snapshot,
        ApplicationGovernanceRequest {
            inspection,
            trusted_label: None,
            untrusted_label: None,
            prior_classification: None,
            session_floor: DataClassification::Public,
            route_floor: DataClassification::Public,
            request_risk_floor: DataClassification::Public,
            tenant: TenantContext { tenant_id },
            principal,
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
}

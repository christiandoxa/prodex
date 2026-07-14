use prodex_application::{
    ApplicationGovernanceRequest, ApplicationGovernanceSnapshot, ApplicationInspectionPlan,
    plan_application_governance,
};
use prodex_domain::{
    CanonicalRoute, CapabilitySet, Channel, ClassificationRuleSet, ClassificationRuleSetChecksum,
    ClassificationRuleSetRevisionId, CredentialScope, DataClassification, DetectorRevisionId,
    EnvironmentContext, GovernancePolicyArtifact, GovernedAction, InspectionCoverage,
    InspectionLimits, InspectionResult, NetworkZone, PolicyEffect, PolicyRevisionId, Principal,
    PrincipalId, PrincipalKind, PrincipalPolicyAttributes, QuotaContext, RequestPolicyAttributes,
    RequestRisk, Role, SessionPolicyContext, TenantContext, TenantId,
    compile_classification_rule_set, compile_governance_policy,
};

#[test]
fn application_pipeline_classifies_before_policy_evaluation() {
    let tenant_id = TenantId::new();
    let tenant = TenantContext { tenant_id };
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
    let snapshot = ApplicationGovernanceSnapshot {
        classification_rules: compile_classification_rule_set(ClassificationRuleSet {
            revision: ClassificationRuleSetRevisionId::new("classification-v1").unwrap(),
            checksum: ClassificationRuleSetChecksum::new("checksum-v1").unwrap(),
            unsupported_coverage_floor: DataClassification::Restricted,
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

    let plan = plan_application_governance(
        &snapshot,
        ApplicationGovernanceRequest {
            inspection: &inspection,
            trusted_label: None,
            untrusted_label: Some(DataClassification::Public),
            prior_classification: None,
            session_floor: DataClassification::Public,
            route_floor: DataClassification::Public,
            request_risk_floor: DataClassification::Public,
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
                retained_classification: DataClassification::Public,
            },
            action: GovernedAction::InvokeModel,
            route: &route,
            request_risk: RequestRisk::Elevated,
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
    .unwrap();

    assert_eq!(
        plan.classification.classification(),
        DataClassification::Restricted
    );
    assert_eq!(plan.policy.effect, PolicyEffect::Allow);
}

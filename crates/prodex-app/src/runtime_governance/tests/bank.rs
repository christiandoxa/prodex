use super::policy_effect;
use crate::runtime_governance::compile_runtime_governance_settings;
use prodex_domain::{
    CanonicalRoute, CapabilitySet, Channel, CredentialScope, DataClassification,
    EnvironmentContext, GovernanceObligation, GovernedAction, InspectionCoverage, NetworkZone,
    PolicyEffect, PolicyInput, Principal, PrincipalId, PrincipalKind, PrincipalPolicyAttributes,
    QuotaContext, RequestPolicyAttributes, RequestRisk, Role, SessionPolicyContext, TenantContext,
    TenantId, evaluate_governance_policy,
};
use prodex_runtime_policy::{
    RuntimeGovernanceDataClassification, RuntimeGovernanceMode, RuntimeGovernancePolicyFailureMode,
    RuntimeGovernanceProviderTrustTier, RuntimeGovernanceRolloutMode,
    RuntimeGovernanceUnknownClassificationBehavior, RuntimePolicyGovernanceProviderSettings,
    RuntimePolicyGovernanceSessionSettings, RuntimePolicyGovernanceSettings,
};

fn valid_bank_settings() -> RuntimePolicyGovernanceSettings {
    let revision = prodex_domain::PolicyRevisionId::new();
    RuntimePolicyGovernanceSettings {
        mode: RuntimeGovernanceMode::BankEnforce,
        inspection: RuntimeGovernanceRolloutMode::Enforce,
        classification: RuntimeGovernanceRolloutMode::Enforce,
        policy: RuntimeGovernanceRolloutMode::Enforce,
        routing: RuntimeGovernanceRolloutMode::Enforce,
        mandatory_audit: true,
        anonymous_data_plane: false,
        raw_secret_sources: false,
        policy_revision: Some(revision),
        policy_valid_until_unix_ms: Some(4_102_444_800_000),
        classification_revision: Some("classification-v1".to_string()),
        classification_checksum: Some("sha256-test-v1".to_string()),
        provider_registry_revision: Some(1),
        routing_score_revision: Some(1),
        provider: Some(RuntimePolicyGovernanceProviderSettings {
            descriptor_revision: 1,
            enabled: true,
            revoked: false,
            trust_tier: RuntimeGovernanceProviderTrustTier::RestrictedApproved,
            local_execution: true,
            maximum_classification: RuntimeGovernanceDataClassification::Restricted,
            regions: vec!["test-region".to_string()],
            retention_seconds: 0,
            training_use: false,
        }),
        classification_default: RuntimeGovernanceDataClassification::Restricted,
        classification_unknown: RuntimeGovernanceUnknownClassificationBehavior::Deny,
        policy_failure_mode: RuntimeGovernancePolicyFailureMode::Closed,
        active_policy_revision: Some(revision),
        session: RuntimePolicyGovernanceSessionSettings {
            absolute_timeout_seconds: Some(3_600),
            idle_timeout_seconds: Some(900),
            max_concurrent: Some(10),
        },
        ..RuntimePolicyGovernanceSettings::default()
    }
}

#[test]
fn bank_snapshot_denies_unsupported_inspection() {
    let tenant_id = TenantId::new();
    let mut settings = valid_bank_settings();
    let default_snapshot = compile_runtime_governance_settings(&settings).unwrap();
    assert_eq!(
        policy_effect(&default_snapshot, TenantId::new()),
        PolicyEffect::Deny
    );

    settings.policy_rules = vec![prodex_runtime_policy::RuntimeGovernancePolicyRule {
        id: "custom.allow-api".to_string(),
        condition: prodex_runtime_policy::RuntimeGovernancePolicyRuleCondition {
            channel: Some(prodex_runtime_policy::RuntimeGovernancePolicyChannel::Api),
            ..Default::default()
        },
        effect: prodex_runtime_policy::RuntimeGovernancePolicyEffect::Allow,
        obligations: Vec::new(),
        reason_code: "policy.custom_allow".to_string(),
    }];
    let snapshot = compile_runtime_governance_settings(&settings).unwrap();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::DataPlane,
    );
    let route = CanonicalRoute::new("responses").unwrap();
    let capabilities = CapabilitySet::new(Vec::new());
    let principal_attributes = PrincipalPolicyAttributes::default();
    let request_attributes = RequestPolicyAttributes::default();
    let decision = evaluate_governance_policy(
        &snapshot.application.policy,
        &PolicyInput {
            tenant: TenantContext { tenant_id },
            principal: &principal,
            principal_attributes: &principal_attributes,
            channel: Channel::Api,
            credential_scope: CredentialScope::DataPlane,
            session: SessionPolicyContext {
                age_seconds: 0,
                idle_seconds: 0,
                revoked: false,
                mfa_satisfied: true,
                retained_classification: DataClassification::Internal,
            },
            action: GovernedAction::InvokeModel,
            route: &route,
            data: prodex_domain::DataPolicyContext {
                classification: DataClassification::Restricted,
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
                network_zone: NetworkZone::TrustedInternal,
                authentication_strength: 3,
                mfa_satisfied: true,
                reauthentication_satisfied: true,
            },
        },
    )
    .unwrap();

    assert_eq!(decision.effect, PolicyEffect::Allow);
    assert!(
        decision
            .obligations
            .contains(&GovernanceObligation::ProhibitRetention)
    );
    assert!(
        decision
            .obligations
            .contains(&GovernanceObligation::RequireResponseInspection)
    );
    assert!(
        decision
            .obligations
            .contains(&GovernanceObligation::DenyFallbackOutsideEligibility)
    );
    assert!(
        decision
            .obligations
            .contains(&GovernanceObligation::MinimumAuthenticationStrength(3))
    );
}

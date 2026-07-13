use prodex_domain::{
    CanonicalRoute, Channel, CredentialScope, DataClassification, DataPolicyContext,
    EnvironmentContext, GovernanceObligation, GovernancePolicyArtifact, GovernancePolicyRule,
    GovernancePolicyRuleId, GovernedAction, InspectionCoverage, NetworkZone, PolicyEffect,
    PolicyInput, PolicyReasonCode, PolicyRevisionId, PolicyRuleCondition, Principal, PrincipalId,
    PrincipalKind, ProviderTrustTier, QuotaContext, RequestRisk, Role, SessionPolicyContext,
    TenantContext, TenantId, compile_governance_policy, evaluate_governance_policy,
};

fn condition() -> PolicyRuleCondition {
    PolicyRuleCondition::default()
}

fn input<'a>(
    tenant: TenantContext,
    principal: &'a Principal,
    route: &'a CanonicalRoute,
    capabilities: &'a prodex_domain::CapabilitySet,
) -> PolicyInput<'a> {
    PolicyInput {
        tenant,
        principal,
        channel: Channel::Cli,
        credential_scope: CredentialScope::DataPlane,
        session: SessionPolicyContext {
            age_seconds: 10,
            idle_seconds: 1,
            revoked: false,
            mfa_satisfied: true,
            retained_classification: DataClassification::Confidential,
        },
        action: GovernedAction::InvokeModel,
        route,
        data: DataPolicyContext {
            classification: DataClassification::Restricted,
            inspection_coverage: InspectionCoverage::Full,
        },
        request_risk: RequestRisk::High,
        requested_capabilities: capabilities,
        quota: QuotaContext {
            has_headroom: true,
            reservation_required: true,
        },
        environment: EnvironmentContext {
            network_zone: NetworkZone::TrustedInternal,
            authentication_strength: 3,
            mfa_satisfied: true,
        },
    }
}

#[test]
fn explicit_deny_wins_and_drops_obligations() {
    let tenant_id = TenantId::new();
    let tenant = TenantContext { tenant_id };
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Operator,
        CredentialScope::DataPlane,
    );
    let policy = compile_governance_policy(GovernancePolicyArtifact {
        revision: PolicyRevisionId::new(),
        valid_until_unix_ms: 10_000,
        default_effect: PolicyEffect::Allow,
        rules: vec![
            GovernancePolicyRule {
                id: GovernancePolicyRuleId::new("allow.restricted.local").unwrap(),
                condition: PolicyRuleCondition {
                    minimum_classification: Some(DataClassification::Restricted),
                    ..condition()
                },
                effect: PolicyEffect::Allow,
                obligations: vec![
                    GovernanceObligation::MinimumProviderTrust(
                        ProviderTrustTier::RestrictedApproved,
                    ),
                    GovernanceObligation::RequireLocalExecution,
                ],
                reason_code: PolicyReasonCode::new("policy.restricted.local").unwrap(),
            },
            GovernancePolicyRule {
                id: GovernancePolicyRuleId::new("deny.high.risk").unwrap(),
                condition: PolicyRuleCondition {
                    minimum_request_risk: Some(RequestRisk::High),
                    ..condition()
                },
                effect: PolicyEffect::Deny,
                obligations: Vec::new(),
                reason_code: PolicyReasonCode::new("policy.risk.denied").unwrap(),
            },
        ],
    })
    .unwrap();
    let route = CanonicalRoute::new("/v1/responses").unwrap();
    let capabilities = prodex_domain::CapabilitySet::new(Vec::new());

    let decision =
        evaluate_governance_policy(&policy, &input(tenant, &principal, &route, &capabilities))
            .unwrap();

    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert!(decision.obligations.is_empty());
    assert_eq!(decision.reason_codes.len(), 2);
}

#[test]
fn extended_abac_conditions_are_bounded_and_exact() {
    let tenant_id = TenantId::new();
    let tenant = TenantContext { tenant_id };
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Operator,
        CredentialScope::DataPlane,
    );
    let policy = compile_governance_policy(GovernancePolicyArtifact {
        revision: PolicyRevisionId::new(),
        valid_until_unix_ms: 10_000,
        default_effect: PolicyEffect::Allow,
        rules: vec![GovernancePolicyRule {
            id: GovernancePolicyRuleId::new("deny.exact.abac").unwrap(),
            condition: PolicyRuleCondition {
                credential_scope: Some(CredentialScope::DataPlane),
                maximum_session_age_seconds: Some(10),
                maximum_session_idle_seconds: Some(1),
                session_revoked: Some(false),
                session_mfa_satisfied: Some(true),
                minimum_session_retained_classification: Some(DataClassification::Confidential),
                minimum_authentication_strength: Some(3),
                environment_mfa_satisfied: Some(true),
                requested_capability: Some(prodex_domain::ModelCapability::Tools),
                quota_has_headroom: Some(true),
                quota_reservation_required: Some(true),
                ..condition()
            },
            effect: PolicyEffect::Deny,
            obligations: Vec::new(),
            reason_code: PolicyReasonCode::new("policy.exact_abac").unwrap(),
        }],
    })
    .unwrap();
    let route = CanonicalRoute::new("/v1/responses").unwrap();
    let tools = prodex_domain::CapabilitySet::new(vec![prodex_domain::ModelCapability::Tools]);
    let none = prodex_domain::CapabilitySet::new(Vec::new());

    let denied =
        evaluate_governance_policy(&policy, &input(tenant, &principal, &route, &tools)).unwrap();
    let allowed =
        evaluate_governance_policy(&policy, &input(tenant, &principal, &route, &none)).unwrap();

    assert_eq!(denied.effect, PolicyEffect::Deny);
    assert_eq!(allowed.effect, PolicyEffect::Allow);
}

#[test]
fn missing_or_cross_tenant_attributes_fail_closed() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let principal = Principal::new(
        PrincipalId::new(),
        Some(TenantId::new()),
        PrincipalKind::ServiceAccount,
        Role::Admin,
        CredentialScope::DataPlane,
    );
    let policy = compile_governance_policy(GovernancePolicyArtifact {
        revision: PolicyRevisionId::new(),
        valid_until_unix_ms: 10_000,
        default_effect: PolicyEffect::Allow,
        rules: Vec::new(),
    })
    .unwrap();
    let route = CanonicalRoute::new("/v1/responses").unwrap();
    let capabilities = prodex_domain::CapabilitySet::new(Vec::new());

    let decision =
        evaluate_governance_policy(&policy, &input(tenant, &principal, &route, &capabilities))
            .unwrap();

    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert_eq!(
        decision.reason_codes[0].as_str(),
        "policy.missing_or_invalid_attribute"
    );
}

#[test]
fn policy_compilation_is_bounded_and_rejects_duplicate_ids() {
    let duplicate = GovernancePolicyRuleId::new("duplicate").unwrap();
    let rule = GovernancePolicyRule {
        id: duplicate.clone(),
        condition: condition(),
        effect: PolicyEffect::Allow,
        obligations: Vec::new(),
        reason_code: PolicyReasonCode::new("policy.allow").unwrap(),
    };
    let error = compile_governance_policy(GovernancePolicyArtifact {
        revision: PolicyRevisionId::new(),
        valid_until_unix_ms: 10_000,
        default_effect: PolicyEffect::Deny,
        rules: vec![
            rule,
            GovernancePolicyRule {
                id: duplicate,
                condition: condition(),
                effect: PolicyEffect::Allow,
                obligations: Vec::new(),
                reason_code: PolicyReasonCode::new("policy.allow.second").unwrap(),
            },
        ],
    })
    .unwrap_err();

    assert_eq!(error, prodex_domain::GovernancePolicyError::DuplicateRule);
}

use prodex_domain::{
    CanonicalRoute, Channel, CredentialScope, DataClassification, DataModality, DataPolicyContext,
    EnvironmentContext, GovernanceObligation, GovernancePolicyArtifact, GovernancePolicyError,
    GovernancePolicyRule, GovernancePolicyRuleId, GovernedAction, InspectionCoverage, NetworkZone,
    PolicyEffect, PolicyInput, PolicyReasonCode, PolicyRevisionId, PolicyRuleCondition,
    PolicySelector, Principal, PrincipalId, PrincipalKind, PrincipalPolicyAttributes,
    ProviderTrustTier, QuotaContext, RequestPolicyAttributes, RequestRisk, Role,
    SessionPolicyContext, TenantContext, TenantId, compile_governance_policy,
    evaluate_governance_policy,
};
use std::sync::LazyLock;

static PRINCIPAL_ATTRIBUTES: LazyLock<PrincipalPolicyAttributes> =
    LazyLock::new(PrincipalPolicyAttributes::default);
static REQUEST_ATTRIBUTES: LazyLock<RequestPolicyAttributes> =
    LazyLock::new(RequestPolicyAttributes::default);

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
        principal_attributes: &PRINCIPAL_ATTRIBUTES,
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
        request_attributes: &REQUEST_ATTRIBUTES,
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
fn typed_attribute_selectors_match_and_missing_values_fail_closed() {
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
            id: GovernancePolicyRuleId::new("deny.scoped.audio.tool").unwrap(),
            condition: PolicyRuleCondition {
                team_id: Some(PolicySelector::new("team-a").unwrap()),
                project_id: Some(PolicySelector::new("project-a").unwrap()),
                user_id: Some(PolicySelector::new("user-a").unwrap()),
                group_id: Some(PolicySelector::new("engineering").unwrap()),
                department_id: Some(PolicySelector::new("research").unwrap()),
                requested_model: Some(PolicySelector::new("model-a").unwrap()),
                requested_tool: Some(PolicySelector::new("shell").unwrap()),
                requested_modality: Some(DataModality::Audio),
                break_glass_required: Some(true),
                break_glass_scope: Some(PolicySelector::new("incident-a").unwrap()),
                ..condition()
            },
            effect: PolicyEffect::Deny,
            obligations: Vec::new(),
            reason_code: PolicyReasonCode::new("policy.scoped.denied").unwrap(),
        }],
    })
    .unwrap();
    let route = CanonicalRoute::new("responses").unwrap();
    let capabilities = prodex_domain::CapabilitySet::new(Vec::new());
    let groups = vec!["platform".to_string(), "engineering".to_string()];
    let principal_attributes = PrincipalPolicyAttributes::new_with_organization(
        Some("team-a"),
        Some("project-a"),
        Some("user-a"),
        &groups,
        Some("research"),
    )
    .unwrap();
    let tools = vec!["shell".to_string()];
    let request_attributes = RequestPolicyAttributes::new(
        Some("model-a"),
        &tools,
        vec![DataModality::Audio],
        Some(prodex_domain::BreakGlassPolicyContext::new("incident-a", 2_000).unwrap()),
        1_000,
    )
    .unwrap();
    let mut complete = input(tenant, &principal, &route, &capabilities);
    complete.principal_attributes = &principal_attributes;
    complete.request_attributes = &request_attributes;
    assert_eq!(
        evaluate_governance_policy(&policy, &complete)
            .unwrap()
            .effect,
        PolicyEffect::Deny
    );

    let missing = input(tenant, &principal, &route, &capabilities);
    let missing = evaluate_governance_policy(&policy, &missing).unwrap();
    assert_eq!(missing.effect, PolicyEffect::Deny);
    assert_eq!(
        missing.reason_codes[0].as_str(),
        "policy.missing_or_invalid_attribute"
    );
    let expired_attributes = RequestPolicyAttributes::new(
        Some("model-a"),
        &tools,
        vec![DataModality::Audio],
        Some(prodex_domain::BreakGlassPolicyContext::new("incident-a", 999).unwrap()),
        1_000,
    )
    .unwrap();
    let mut expired = input(tenant, &principal, &route, &capabilities);
    expired.principal_attributes = &principal_attributes;
    expired.request_attributes = &expired_attributes;
    assert_eq!(
        evaluate_governance_policy(&policy, &expired)
            .unwrap()
            .reason_codes[0]
            .as_str(),
        "policy.missing_or_invalid_attribute"
    );
    let rendered = format!("{principal_attributes:?} {request_attributes:?}");
    for secret in [
        "team-a",
        "project-a",
        "user-a",
        "engineering",
        "research",
        "model-a",
        "shell",
        "incident-a",
    ] {
        assert!(!rendered.contains(secret));
    }
}

#[test]
fn typed_request_attributes_are_bounded_and_condition_debug_is_redacted() {
    let too_many_tools = vec!["tool".to_string(); 129];
    assert_eq!(
        RequestPolicyAttributes::new(None, &too_many_tools, Vec::new(), None, 1),
        Err(GovernancePolicyError::AttributeLimitExceeded)
    );
    let overlong_model = "x".repeat(129);
    assert_eq!(
        RequestPolicyAttributes::new(Some(&overlong_model), &[], Vec::new(), None, 1),
        Err(GovernancePolicyError::InvalidToken)
    );
    let too_many_groups = vec!["group".to_string(); 129];
    assert_eq!(
        PrincipalPolicyAttributes::new_with_organization(None, None, None, &too_many_groups, None,),
        Err(GovernancePolicyError::AttributeLimitExceeded)
    );

    let condition = PolicyRuleCondition {
        team_id: Some(PolicySelector::new("team-secret").unwrap()),
        group_id: Some(PolicySelector::new("group-secret").unwrap()),
        department_id: Some(PolicySelector::new("department-secret").unwrap()),
        requested_model: Some(PolicySelector::new("model-secret").unwrap()),
        requested_tool: Some(PolicySelector::new("tool-secret").unwrap()),
        break_glass_scope: Some(PolicySelector::new("scope-secret").unwrap()),
        ..condition()
    };
    let rendered = format!("{condition:?}");
    for secret in [
        "team-secret",
        "group-secret",
        "department-secret",
        "model-secret",
        "tool-secret",
        "scope-secret",
    ] {
        assert!(!rendered.contains(secret));
    }
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

fn policy_rule(
    id: &str,
    condition: PolicyRuleCondition,
    obligations: Vec<GovernanceObligation>,
) -> GovernancePolicyRule {
    GovernancePolicyRule {
        id: GovernancePolicyRuleId::new(id).unwrap(),
        condition,
        effect: PolicyEffect::Allow,
        obligations,
        reason_code: PolicyReasonCode::new(format!("policy.{id}")).unwrap(),
    }
}

fn policy_artifact(rules: Vec<GovernancePolicyRule>) -> GovernancePolicyArtifact {
    GovernancePolicyArtifact {
        revision: PolicyRevisionId::new(),
        valid_until_unix_ms: 10_000,
        default_effect: PolicyEffect::Deny,
        rules,
    }
}

#[test]
fn policy_compilation_rejects_conflicting_obligations() {
    let selector = |value| PolicySelector::new(value).unwrap();
    let conflicts = vec![
        vec![
            GovernanceObligation::AllowProvider(selector("openai")),
            GovernanceObligation::DenyProvider(selector("openai")),
        ],
        vec![
            GovernanceObligation::RequireRegion(selector("eu-central")),
            GovernanceObligation::RequireRegion(selector("us-east")),
        ],
        vec![
            GovernanceObligation::ProhibitRetention,
            GovernanceObligation::RetentionSeconds(1),
        ],
        vec![
            GovernanceObligation::DisableTools,
            GovernanceObligation::AllowTool(selector("lookup")),
        ],
        vec![
            GovernanceObligation::MaxInputTokens(101),
            GovernanceObligation::MaxContextTokens(100),
        ],
        vec![
            GovernanceObligation::MaxOutputTokens(101),
            GovernanceObligation::MaxContextTokens(100),
        ],
        vec![GovernanceObligation::MaxInputTokens(0)],
    ];

    for (index, obligations) in conflicts.into_iter().enumerate() {
        let error = compile_governance_policy(policy_artifact(vec![policy_rule(
            &format!("conflict-{index}"),
            condition(),
            obligations,
        )]))
        .unwrap_err();
        assert_eq!(error, GovernancePolicyError::ConflictingObligations);
    }

    let error = compile_governance_policy(policy_artifact(vec![
        policy_rule(
            "allow-provider",
            PolicyRuleCondition {
                channel: Some(Channel::Cli),
                ..condition()
            },
            vec![GovernanceObligation::AllowProvider(selector("openai"))],
        ),
        policy_rule(
            "deny-provider",
            PolicyRuleCondition {
                channel: Some(Channel::Cli),
                ..condition()
            },
            vec![GovernanceObligation::DenyProvider(selector("openai"))],
        ),
    ]))
    .unwrap_err();
    assert_eq!(error, GovernancePolicyError::ConflictingObligations);
}

#[test]
fn compatible_obligations_compile_deterministically() {
    let selector = |value| PolicySelector::new(value).unwrap();
    let obligations = vec![
        GovernanceObligation::AllowProvider(selector("*")),
        GovernanceObligation::DenyProvider(selector("anthropic")),
        GovernanceObligation::RequireLocalExecution,
        GovernanceObligation::RequireRegion(selector("eu-central")),
        GovernanceObligation::ProhibitRetention,
        GovernanceObligation::RetentionSeconds(0),
        GovernanceObligation::ProhibitTrainingUse,
        GovernanceObligation::AllowTool(selector("lookup")),
        GovernanceObligation::AllowModel(selector("model-a")),
        GovernanceObligation::AllowModel(selector("model-b")),
        GovernanceObligation::AllowModality(DataModality::Text),
        GovernanceObligation::AllowModality(DataModality::Image),
        GovernanceObligation::MaxInputTokens(80),
        GovernanceObligation::MaxOutputTokens(20),
        GovernanceObligation::MaxContextTokens(100),
    ];
    let revision = PolicyRevisionId::new();
    let artifact = |obligations: Vec<GovernanceObligation>| GovernancePolicyArtifact {
        revision,
        valid_until_unix_ms: 10_000,
        default_effect: PolicyEffect::Deny,
        rules: vec![policy_rule("compatible", condition(), obligations)],
    };
    let mut reversed = obligations.clone();
    reversed.reverse();
    let first = compile_governance_policy(artifact(obligations.clone())).unwrap();
    let second = compile_governance_policy(artifact(reversed)).unwrap();
    let tenant_id = TenantId::new();
    let tenant = TenantContext { tenant_id };
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Operator,
        CredentialScope::DataPlane,
    );
    let route = CanonicalRoute::new("/v1/responses").unwrap();
    let capabilities = prodex_domain::CapabilitySet::new(Vec::new());

    assert_eq!(
        evaluate_governance_policy(&first, &input(tenant, &principal, &route, &capabilities),)
            .unwrap(),
        evaluate_governance_policy(&second, &input(tenant, &principal, &route, &capabilities),)
            .unwrap(),
    );
}

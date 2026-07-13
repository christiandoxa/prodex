use prodex_domain::{
    CapabilitySet, DataClassification, GovernanceObligation, ModelCapability, PolicyDecision,
    PolicyEffect, PolicyReasonCode, PolicyRevisionId, PolicySelector, ProviderTrustTier, SecretRef,
    TenantContext, TenantId,
};
use prodex_provider_core::ProviderId;
use prodex_provider_spi::{
    GovernedProviderDescriptor, GovernedProviderRegistry, GovernedRoutingError,
    GovernedRoutingRequest, GovernedRoutingSignals, GovernedRoutingWeights,
    MAX_GOVERNED_PROVIDER_REGIONS, MAX_GOVERNED_ROUTING_CANDIDATES, MAX_GOVERNED_ROUTING_FALLBACKS,
    plan_governed_provider_route,
};

fn selector(value: &str) -> PolicySelector {
    PolicySelector::new(value).unwrap()
}

fn decision(effect: PolicyEffect, obligations: Vec<GovernanceObligation>) -> PolicyDecision {
    PolicyDecision {
        effect,
        obligations,
        reason_codes: vec![PolicyReasonCode::new("routing.test").unwrap()],
        policy_revision: PolicyRevisionId::new(),
        valid_until_unix_ms: u64::MAX,
    }
}

fn descriptor(tenant: TenantContext, provider: ProviderId) -> GovernedProviderDescriptor {
    GovernedProviderDescriptor {
        revision: 7,
        tenant,
        provider,
        credential_ref: SecretRef::new(
            "vault",
            format!("providers/{}", provider.label()),
            Some("v3"),
        ),
        credential_available: true,
        enabled: true,
        revoked: false,
        circuit_open: false,
        quota_available: true,
        local_execution: true,
        trust_tier: ProviderTrustTier::RestrictedApproved,
        maximum_classification: DataClassification::Restricted,
        capabilities: CapabilitySet::new(vec![
            ModelCapability::ResponsesApi,
            ModelCapability::Tools,
        ]),
        regions: vec![selector("eu-central")],
        retention_seconds: 0,
        training_use: false,
        signals: GovernedRoutingSignals {
            health: 9_000,
            load: 1_000,
            cost: 2_000,
            latency: 3_000,
            risk: 4_000,
            priority: 5_000,
        },
    }
}

fn plan_one(
    tenant: TenantContext,
    provider: GovernedProviderDescriptor,
    policy: &PolicyDecision,
) -> Result<prodex_provider_spi::GovernedRoutingPlan, GovernedRoutingError> {
    let capabilities =
        CapabilitySet::new(vec![ModelCapability::ResponsesApi, ModelCapability::Tools]);
    let registry = GovernedProviderRegistry {
        revision: 11,
        providers: vec![provider],
    };
    plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: DataClassification::Restricted,
        required_capabilities: &capabilities,
        policy,
        registry: &registry,
        score_revision: 1,
        weights: GovernedRoutingWeights::default(),
        affinity_provider: None,
        max_fallbacks: 0,
    })
}

fn strict_policy() -> PolicyDecision {
    decision(
        PolicyEffect::Allow,
        vec![
            GovernanceObligation::AllowProvider(selector("openai")),
            GovernanceObligation::MinimumProviderTrust(ProviderTrustTier::RestrictedApproved),
            GovernanceObligation::RequireRegion(selector("eu-central")),
            GovernanceObligation::RequireLocalExecution,
            GovernanceObligation::ProhibitRetention,
            GovernanceObligation::RetentionSeconds(0),
            GovernanceObligation::ProhibitTrainingUse,
            GovernanceObligation::DenyFallbackOutsideEligibility,
        ],
    )
}

#[test]
fn governed_routing_enforces_every_hard_eligibility_gate() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let policy = strict_policy();
    let base = descriptor(tenant, ProviderId::OpenAi);
    assert_eq!(
        plan_one(tenant, base.clone(), &policy)
            .unwrap()
            .primary
            .provider,
        ProviderId::OpenAi
    );

    let mut failures = Vec::new();
    let mut candidate = base.clone();
    candidate.enabled = false;
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.revoked = true;
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.circuit_open = true;
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.quota_available = false;
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.credential_available = false;
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.local_execution = false;
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.trust_tier = ProviderTrustTier::Enterprise;
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.maximum_classification = DataClassification::Confidential;
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.capabilities = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.regions = vec![selector("us-east")];
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.retention_seconds = 1;
    failures.push(candidate);
    let mut candidate = base.clone();
    candidate.training_use = true;
    failures.push(candidate);

    for candidate in failures {
        assert_eq!(
            plan_one(tenant, candidate, &policy),
            Err(GovernedRoutingError::NoEligibleProvider)
        );
    }

    let denied = decision(
        PolicyEffect::Allow,
        vec![GovernanceObligation::DenyProvider(selector("openai"))],
    );
    assert_eq!(
        plan_one(tenant, base.clone(), &denied),
        Err(GovernedRoutingError::NoEligibleProvider)
    );
    let wrong_allow_list = decision(
        PolicyEffect::Allow,
        vec![GovernanceObligation::AllowProvider(selector("anthropic"))],
    );
    assert_eq!(
        plan_one(tenant, base, &wrong_allow_list),
        Err(GovernedRoutingError::NoEligibleProvider)
    );
}

#[test]
fn governed_routing_scores_in_fixed_point_and_breaks_ties_by_provider() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let policy = decision(PolicyEffect::Allow, Vec::new());
    let capabilities = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    let openai = descriptor(tenant, ProviderId::OpenAi);
    let mut anthropic = descriptor(tenant, ProviderId::Anthropic);
    anthropic.signals = openai.signals;
    let registry = GovernedProviderRegistry {
        revision: 21,
        providers: vec![anthropic, openai],
    };
    let plan = plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: DataClassification::Internal,
        required_capabilities: &capabilities,
        policy: &policy,
        registry: &registry,
        score_revision: 1,
        weights: GovernedRoutingWeights::default(),
        affinity_provider: None,
        max_fallbacks: 1,
    })
    .unwrap();

    assert_eq!(plan.primary.provider, ProviderId::OpenAi);
    assert_eq!(plan.primary.score, 6_500);
    assert_eq!(plan.fallbacks.len(), 1);
    assert_eq!(plan.fallbacks[0].provider, ProviderId::Anthropic);

    let affinity_only = GovernedRoutingWeights {
        health: 0,
        load: 0,
        cost: 0,
        latency: 0,
        risk: 0,
        priority: 0,
        affinity: 10_000,
    };
    let affinity_plan = plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: DataClassification::Internal,
        required_capabilities: &capabilities,
        policy: &policy,
        registry: &registry,
        score_revision: 1,
        weights: affinity_only,
        affinity_provider: Some(ProviderId::Anthropic),
        max_fallbacks: 1,
    })
    .unwrap();
    assert_eq!(affinity_plan.primary.provider, ProviderId::Anthropic);
    assert_eq!(affinity_plan.primary.score, 10_000);
}

#[test]
fn governed_routing_never_places_ineligible_or_cross_tenant_routes_in_fallbacks() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let other_tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let policy = decision(PolicyEffect::Allow, Vec::new());
    let capabilities = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    let mut unavailable = descriptor(tenant, ProviderId::Anthropic);
    unavailable.quota_available = false;
    let registry = GovernedProviderRegistry {
        revision: 22,
        providers: vec![
            unavailable,
            descriptor(other_tenant, ProviderId::Copilot),
            descriptor(tenant, ProviderId::OpenAi),
            descriptor(tenant, ProviderId::Local),
        ],
    };
    let plan = plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: DataClassification::Internal,
        required_capabilities: &capabilities,
        policy: &policy,
        registry: &registry,
        score_revision: 1,
        weights: GovernedRoutingWeights::default(),
        affinity_provider: None,
        max_fallbacks: 1,
    })
    .unwrap();

    assert_eq!(plan.primary.provider, ProviderId::OpenAi);
    assert_eq!(plan.fallbacks.len(), 1);
    assert_eq!(plan.fallbacks[0].provider, ProviderId::Local);
}

#[test]
fn governed_routing_rejects_policy_and_unbounded_or_invalid_inputs() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let base = descriptor(tenant, ProviderId::OpenAi);
    for policy in [
        decision(PolicyEffect::Deny, Vec::new()),
        decision(PolicyEffect::RequireApproval, Vec::new()),
        decision(
            PolicyEffect::Allow,
            vec![GovernanceObligation::RequireHumanApproval],
        ),
    ] {
        assert!(matches!(
            plan_one(tenant, base.clone(), &policy),
            Err(GovernedRoutingError::PolicyDenied | GovernedRoutingError::ApprovalRequired)
        ));
    }

    let policy = decision(PolicyEffect::Allow, Vec::new());
    let capabilities = CapabilitySet::new(Vec::new());
    let registry = GovernedProviderRegistry {
        revision: 0,
        providers: vec![base.clone()],
    };
    let mut request = GovernedRoutingRequest {
        tenant,
        classification: DataClassification::Public,
        required_capabilities: &capabilities,
        policy: &policy,
        registry: &registry,
        score_revision: 1,
        weights: GovernedRoutingWeights::default(),
        affinity_provider: None,
        max_fallbacks: 0,
    };
    assert_eq!(
        plan_governed_provider_route(&request),
        Err(GovernedRoutingError::InvalidRegistryRevision)
    );

    let valid_registry = GovernedProviderRegistry {
        revision: 1,
        providers: vec![base.clone()],
    };
    request.registry = &valid_registry;
    request.max_fallbacks = MAX_GOVERNED_ROUTING_FALLBACKS + 1;
    assert!(matches!(
        plan_governed_provider_route(&request),
        Err(GovernedRoutingError::FallbackLimitExceeded { .. })
    ));
    request.max_fallbacks = 0;
    request.weights = GovernedRoutingWeights {
        health: 0,
        load: 0,
        cost: 0,
        latency: 0,
        risk: 0,
        priority: 0,
        affinity: 0,
    };
    assert_eq!(
        plan_governed_provider_route(&request),
        Err(GovernedRoutingError::InvalidWeights)
    );

    let mut invalid = base.clone();
    invalid.credential_ref = SecretRef::new("vault", "bad credential", None::<String>);
    assert!(matches!(
        plan_one(tenant, invalid, &policy),
        Err(GovernedRoutingError::InvalidCredentialReference { .. })
    ));
    let mut invalid = base.clone();
    invalid.signals.health = 10_001;
    assert!(matches!(
        plan_one(tenant, invalid, &policy),
        Err(GovernedRoutingError::InvalidSignal { .. })
    ));
    let mut invalid = base.clone();
    invalid.regions = (0..=MAX_GOVERNED_PROVIDER_REGIONS)
        .map(|index| selector(&format!("region-{index}")))
        .collect();
    assert!(matches!(
        plan_one(tenant, invalid, &policy),
        Err(GovernedRoutingError::RegionLimitExceeded { .. })
    ));

    let many = (0..=MAX_GOVERNED_ROUTING_CANDIDATES)
        .map(|_| {
            descriptor(
                TenantContext {
                    tenant_id: TenantId::new(),
                },
                ProviderId::OpenAi,
            )
        })
        .collect();
    let registry = GovernedProviderRegistry {
        revision: 1,
        providers: many,
    };
    assert!(matches!(
        plan_governed_provider_route(&GovernedRoutingRequest {
            tenant,
            classification: DataClassification::Public,
            required_capabilities: &capabilities,
            policy: &policy,
            registry: &registry,
            score_revision: 1,
            weights: GovernedRoutingWeights::default(),
            affinity_provider: None,
            max_fallbacks: 0,
        }),
        Err(GovernedRoutingError::CandidateLimitExceeded { .. })
    ));
}

#[test]
fn governed_routing_debug_output_contains_no_tenant_region_revision_or_secret_reference() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let mut provider = descriptor(tenant, ProviderId::OpenAi);
    provider.revision = 9_876_543;
    provider.credential_ref = SecretRef::new(
        "secret-backend-token",
        "private/provider-credential-token",
        Some("private-version-token"),
    );
    provider.regions = vec![selector("private-region-token")];
    let provider_debug = format!("{provider:?}");
    for secret in [
        tenant.tenant_id.to_string(),
        "9876543".to_string(),
        "secret-backend-token".to_string(),
        "private/provider-credential-token".to_string(),
        "private-version-token".to_string(),
        "private-region-token".to_string(),
    ] {
        assert!(!provider_debug.contains(&secret));
    }

    let policy = decision(PolicyEffect::Allow, Vec::new());
    let plan = plan_one(tenant, provider, &policy).unwrap();
    let plan_debug = format!("{plan:?}");
    assert!(!plan_debug.contains(&tenant.tenant_id.to_string()));
    assert!(!plan_debug.contains("9876543"));
    assert!(!plan_debug.contains("private/provider-credential-token"));
}

use prodex_domain::{
    CapabilitySet, DataClassification, GovernanceObligation, ModelCapability, PolicyDecision,
    PolicyEffect, PolicyReasonCode, PolicyRevisionId, PolicySelector, ProviderTrustTier, SecretRef,
    TenantContext, TenantId,
};
use prodex_provider_core::ProviderId;
use prodex_provider_spi::{
    GOVERNED_SCORE_COMPONENT_COUNT, GovernedCandidateOutcome, GovernedHardFilterReason,
    GovernedProviderDescriptor, GovernedProviderRegistry, GovernedRoutingError,
    GovernedRoutingRequest, GovernedRoutingSignals, GovernedRoutingWeights,
    MAX_GOVERNED_HARD_FILTER_REASONS, MAX_GOVERNED_PROVIDER_REGIONS,
    MAX_GOVERNED_ROUTING_CANDIDATES, MAX_GOVERNED_ROUTING_FALLBACKS, plan_governed_provider_route,
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
        pricing_revision: 3,
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
        inflight_cap_reached: false,
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
            health: Some(9_000),
            load: 1_000,
            quota_headroom: None,
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

fn plan_with_rejected_candidate(
    tenant: TenantContext,
    candidate: GovernedProviderDescriptor,
    policy: &PolicyDecision,
) -> prodex_provider_spi::GovernedRoutingPlan {
    let capabilities =
        CapabilitySet::new(vec![ModelCapability::ResponsesApi, ModelCapability::Tools]);
    let registry = GovernedProviderRegistry {
        revision: 12,
        providers: vec![candidate, descriptor(tenant, ProviderId::OpenAi)],
    };
    plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: DataClassification::Restricted,
        required_capabilities: &capabilities,
        policy,
        registry: &registry,
        score_revision: 3,
        weights: GovernedRoutingWeights::default(),
        affinity_provider: None,
        max_fallbacks: 1,
    })
    .unwrap()
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
    assert_eq!(plan.primary.score_breakdown.score_revision, 1);
    assert_eq!(
        plan.primary.score_breakdown.components.len(),
        GOVERNED_SCORE_COMPONENT_COUNT
    );
    assert_eq!(
        plan.primary
            .score_breakdown
            .components
            .iter()
            .map(|component| component.kind.reason_code())
            .collect::<Vec<_>>(),
        vec![
            "routing.score.health",
            "routing.score.available_capacity",
            "routing.score.cost_efficiency",
            "routing.score.latency_efficiency",
            "routing.score.risk_reduction",
            "routing.score.operator_priority",
            "routing.score.affinity",
        ]
    );
    assert_eq!(
        plan.primary
            .score_breakdown
            .components
            .map(|component| component.normalized_value),
        [9_000, 9_000, 8_000, 7_000, 6_000, 5_000, 0]
    );
    assert_eq!(
        plan.primary.score_breakdown.weighted_total,
        plan.primary
            .score_breakdown
            .components
            .iter()
            .map(|component| component.weighted_value)
            .sum::<u64>()
    );
    assert_eq!(plan.primary.score_breakdown.weight_total, 10_000);
    assert_eq!(plan.fallbacks.len(), 1);
    assert_eq!(plan.fallbacks[0].provider, ProviderId::Anthropic);
    assert_eq!(
        plan.candidate_evaluations
            .iter()
            .filter(|candidate| candidate.outcome() == GovernedCandidateOutcome::Eligible)
            .count(),
        2
    );

    let repeated = plan_governed_provider_route(&GovernedRoutingRequest {
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
    assert_eq!(plan, repeated);

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
fn lower_cost_is_a_soft_score_after_hard_eligibility() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let policy = decision(PolicyEffect::Allow, Vec::new());
    let capabilities = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    let mut cheap = descriptor(tenant, ProviderId::Anthropic);
    cheap.signals.cost = 0;
    cheap.enabled = false;
    let mut eligible = descriptor(tenant, ProviderId::OpenAi);
    eligible.signals.cost = 9_000;
    let weights = GovernedRoutingWeights {
        health: 0,
        load: 0,
        cost: 10_000,
        latency: 0,
        risk: 0,
        priority: 0,
        affinity: 0,
    };
    let plan = |providers| {
        plan_governed_provider_route(&GovernedRoutingRequest {
            tenant,
            classification: DataClassification::Internal,
            required_capabilities: &capabilities,
            policy: &policy,
            registry: &GovernedProviderRegistry {
                revision: 22,
                providers,
            },
            score_revision: 4,
            weights,
            affinity_provider: None,
            max_fallbacks: 0,
        })
        .unwrap()
    };

    assert_eq!(
        plan(vec![cheap.clone(), eligible.clone()]).primary.provider,
        ProviderId::OpenAi
    );
    cheap.enabled = true;
    assert_eq!(
        plan(vec![cheap, eligible]).primary.provider,
        ProviderId::Anthropic
    );
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
    let rejected = plan
        .candidate_evaluations
        .iter()
        .filter(|candidate| candidate.outcome() == GovernedCandidateOutcome::Rejected)
        .collect::<Vec<_>>();
    assert_eq!(rejected.len(), 2);
    assert!(rejected.iter().any(|candidate| {
        candidate.provider == ProviderId::Anthropic
            && candidate.rejection_reasons() == [GovernedHardFilterReason::QuotaUnavailable]
    }));
    assert!(rejected.iter().any(|candidate| {
        candidate.provider == ProviderId::Copilot
            && candidate.rejection_reasons() == [GovernedHardFilterReason::TenantMismatch]
    }));
    assert!(
        plan.primary
            .score_breakdown
            .components
            .iter()
            .all(|component| component.normalized_value <= 10_000 && component.weight <= 10_000)
    );
}

#[test]
fn governed_routing_keeps_eligible_continuation_affinity_ahead_of_soft_score() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let policy = decision(PolicyEffect::Allow, Vec::new());
    let capabilities = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    let preferred = descriptor(tenant, ProviderId::OpenAi);
    let mut affinity = descriptor(tenant, ProviderId::Anthropic);
    affinity.signals = GovernedRoutingSignals {
        health: Some(0),
        load: 10_000,
        quota_headroom: Some(0),
        cost: 10_000,
        latency: 10_000,
        risk: 10_000,
        priority: 0,
    };
    let registry = GovernedProviderRegistry {
        revision: 23,
        providers: vec![preferred, affinity],
    };

    let plan = plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: DataClassification::Internal,
        required_capabilities: &capabilities,
        policy: &policy,
        registry: &registry,
        score_revision: 1,
        weights: GovernedRoutingWeights::default(),
        affinity_provider: Some(ProviderId::Anthropic),
        max_fallbacks: 1,
    })
    .unwrap();

    assert_eq!(plan.primary.provider, ProviderId::Anthropic);
    assert_eq!(plan.fallbacks[0].provider, ProviderId::OpenAi);
    assert!(plan.primary.score < plan.fallbacks[0].score);
}

#[test]
fn governed_routing_runtime_signals_change_selection_and_keep_only_eligible_fallbacks() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let policy = decision(PolicyEffect::Allow, Vec::new());
    let capabilities = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    let mut openai = descriptor(tenant, ProviderId::OpenAi);
    openai.signals.health = Some(1_000);
    openai.signals.load = 9_000;
    openai.signals.quota_headroom = Some(500);
    let mut anthropic = descriptor(tenant, ProviderId::Anthropic);
    anthropic.signals.health = Some(9_000);
    anthropic.signals.load = 500;
    anthropic.signals.quota_headroom = Some(9_500);
    let mut rejected = descriptor(tenant, ProviderId::Copilot);
    rejected.quota_available = false;
    let registry = GovernedProviderRegistry {
        revision: 26,
        providers: vec![openai, anthropic, rejected],
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
        max_fallbacks: 2,
    })
    .unwrap();

    assert_eq!(plan.primary.provider, ProviderId::Anthropic);
    assert_eq!(plan.fallbacks.len(), 1);
    assert_eq!(plan.fallbacks[0].provider, ProviderId::OpenAi);
    assert!(
        plan.fallbacks
            .iter()
            .all(|route| route.provider != ProviderId::Copilot)
    );
}

#[test]
fn governed_routing_keeps_hard_affinity_across_temporary_runtime_degradation() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let policy = decision(PolicyEffect::Allow, Vec::new());
    let capabilities = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    let mut affinity = descriptor(tenant, ProviderId::Anthropic);
    affinity.circuit_open = true;
    affinity.quota_available = false;
    affinity.inflight_cap_reached = true;
    let registry = GovernedProviderRegistry {
        revision: 25,
        providers: vec![descriptor(tenant, ProviderId::OpenAi), affinity],
    };

    let plan = plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: DataClassification::Internal,
        required_capabilities: &capabilities,
        policy: &policy,
        registry: &registry,
        score_revision: 1,
        weights: GovernedRoutingWeights::default(),
        affinity_provider: Some(ProviderId::Anthropic),
        max_fallbacks: 1,
    })
    .unwrap();

    assert_eq!(plan.primary.provider, ProviderId::Anthropic);
    assert_eq!(plan.fallbacks[0].provider, ProviderId::OpenAi);
}

#[test]
fn governed_routing_does_not_preserve_affinity_after_revocation() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let policy = decision(PolicyEffect::Allow, Vec::new());
    let capabilities = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    let mut revoked = descriptor(tenant, ProviderId::Anthropic);
    revoked.revoked = true;
    let registry = GovernedProviderRegistry {
        revision: 24,
        providers: vec![revoked, descriptor(tenant, ProviderId::OpenAi)],
    };

    let plan = plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: DataClassification::Internal,
        required_capabilities: &capabilities,
        policy: &policy,
        registry: &registry,
        score_revision: 1,
        weights: GovernedRoutingWeights::default(),
        affinity_provider: Some(ProviderId::Anthropic),
        max_fallbacks: 1,
    })
    .unwrap();

    assert_eq!(plan.primary.provider, ProviderId::OpenAi);
    assert!(plan.fallbacks.is_empty());
    assert_eq!(
        plan.candidate_evaluations
            .iter()
            .find(|candidate| candidate.provider == ProviderId::Anthropic)
            .unwrap()
            .rejection_reasons(),
        [GovernedHardFilterReason::ProviderRevoked]
    );
}

#[test]
fn governed_routing_explains_every_hard_filter_with_stable_reason_codes() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let allow = decision(PolicyEffect::Allow, Vec::new());
    let mut cases = Vec::new();

    cases.push((
        descriptor(
            TenantContext {
                tenant_id: TenantId::new(),
            },
            ProviderId::Anthropic,
        ),
        allow.clone(),
        GovernedHardFilterReason::TenantMismatch,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.enabled = false;
    cases.push((
        candidate,
        allow.clone(),
        GovernedHardFilterReason::ProviderDisabled,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.revoked = true;
    cases.push((
        candidate,
        allow.clone(),
        GovernedHardFilterReason::ProviderRevoked,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.circuit_open = true;
    cases.push((
        candidate,
        allow.clone(),
        GovernedHardFilterReason::CircuitOpen,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.quota_available = false;
    cases.push((
        candidate,
        allow.clone(),
        GovernedHardFilterReason::QuotaUnavailable,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.inflight_cap_reached = true;
    cases.push((
        candidate,
        allow.clone(),
        GovernedHardFilterReason::InFlightCapReached,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.credential_available = false;
    cases.push((
        candidate,
        allow.clone(),
        GovernedHardFilterReason::CredentialUnavailable,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.maximum_classification = DataClassification::Confidential;
    cases.push((
        candidate,
        allow.clone(),
        GovernedHardFilterReason::ClassificationUnsupported,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.capabilities = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
    cases.push((
        candidate,
        allow.clone(),
        GovernedHardFilterReason::CapabilityMissing,
    ));
    cases.push((
        descriptor(tenant, ProviderId::Anthropic),
        decision(
            PolicyEffect::Allow,
            vec![GovernanceObligation::AllowProvider(selector("openai"))],
        ),
        GovernedHardFilterReason::ProviderNotAllowed,
    ));
    cases.push((
        descriptor(tenant, ProviderId::Anthropic),
        decision(
            PolicyEffect::Allow,
            vec![GovernanceObligation::DenyProvider(selector("anthropic"))],
        ),
        GovernedHardFilterReason::ProviderDenied,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.trust_tier = ProviderTrustTier::Enterprise;
    cases.push((
        candidate,
        decision(
            PolicyEffect::Allow,
            vec![GovernanceObligation::MinimumProviderTrust(
                ProviderTrustTier::RestrictedApproved,
            )],
        ),
        GovernedHardFilterReason::TrustTierInsufficient,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.regions = vec![selector("us-east")];
    cases.push((
        candidate,
        decision(
            PolicyEffect::Allow,
            vec![GovernanceObligation::RequireRegion(selector("eu-central"))],
        ),
        GovernedHardFilterReason::RegionUnavailable,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.local_execution = false;
    cases.push((
        candidate,
        decision(
            PolicyEffect::Allow,
            vec![GovernanceObligation::RequireLocalExecution],
        ),
        GovernedHardFilterReason::LocalExecutionRequired,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.retention_seconds = 1;
    cases.push((
        candidate,
        decision(
            PolicyEffect::Allow,
            vec![GovernanceObligation::ProhibitRetention],
        ),
        GovernedHardFilterReason::RetentionProhibited,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.retention_seconds = 2;
    cases.push((
        candidate,
        decision(
            PolicyEffect::Allow,
            vec![GovernanceObligation::RetentionSeconds(1)],
        ),
        GovernedHardFilterReason::RetentionLimitExceeded,
    ));
    let mut candidate = descriptor(tenant, ProviderId::Anthropic);
    candidate.training_use = true;
    cases.push((
        candidate,
        decision(
            PolicyEffect::Allow,
            vec![GovernanceObligation::ProhibitTrainingUse],
        ),
        GovernedHardFilterReason::TrainingUseProhibited,
    ));

    assert_eq!(cases.len(), MAX_GOVERNED_HARD_FILTER_REASONS);
    for (candidate, policy, expected) in cases {
        let plan = plan_with_rejected_candidate(tenant, candidate, &policy);
        let rejected = plan
            .candidate_evaluations
            .iter()
            .find(|candidate| candidate.provider == ProviderId::Anthropic)
            .unwrap();
        assert_eq!(rejected.outcome(), GovernedCandidateOutcome::Rejected);
        assert_eq!(rejected.rejection_reasons(), [expected]);
        assert_eq!(rejected.score_breakdown(), None);
        assert_eq!(plan.primary.provider, ProviderId::OpenAi);
        assert!(!expected.reason_code().is_empty());
    }

    assert_eq!(
        [
            GovernedHardFilterReason::TenantMismatch,
            GovernedHardFilterReason::ProviderDisabled,
            GovernedHardFilterReason::ProviderRevoked,
            GovernedHardFilterReason::CircuitOpen,
            GovernedHardFilterReason::QuotaUnavailable,
            GovernedHardFilterReason::InFlightCapReached,
            GovernedHardFilterReason::CredentialUnavailable,
            GovernedHardFilterReason::ClassificationUnsupported,
            GovernedHardFilterReason::CapabilityMissing,
            GovernedHardFilterReason::ProviderNotAllowed,
            GovernedHardFilterReason::ProviderDenied,
            GovernedHardFilterReason::TrustTierInsufficient,
            GovernedHardFilterReason::RegionUnavailable,
            GovernedHardFilterReason::LocalExecutionRequired,
            GovernedHardFilterReason::RetentionProhibited,
            GovernedHardFilterReason::RetentionLimitExceeded,
            GovernedHardFilterReason::TrainingUseProhibited,
        ]
        .map(GovernedHardFilterReason::reason_code),
        [
            "routing.hard.tenant_mismatch",
            "routing.hard.provider_disabled",
            "routing.hard.provider_revoked",
            "routing.hard.circuit_open",
            "routing.hard.quota_unavailable",
            "routing.hard.inflight_cap_reached",
            "routing.hard.credential_unavailable",
            "routing.hard.classification_unsupported",
            "routing.hard.capability_missing",
            "routing.hard.provider_not_allowed",
            "routing.hard.provider_denied",
            "routing.hard.trust_tier_insufficient",
            "routing.hard.region_unavailable",
            "routing.hard.local_execution_required",
            "routing.hard.retention_prohibited",
            "routing.hard.retention_limit_exceeded",
            "routing.hard.training_use_prohibited",
        ]
    );
}

#[test]
fn governed_routing_bounds_accumulated_hard_filter_explanations() {
    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let mut rejected = descriptor(
        TenantContext {
            tenant_id: TenantId::new(),
        },
        ProviderId::Anthropic,
    );
    rejected.enabled = false;
    rejected.revoked = true;
    rejected.circuit_open = true;
    rejected.quota_available = false;
    rejected.inflight_cap_reached = true;
    rejected.credential_available = false;
    rejected.maximum_classification = DataClassification::Public;
    rejected.capabilities = CapabilitySet::new(Vec::new());
    rejected.local_execution = false;
    rejected.trust_tier = ProviderTrustTier::Standard;
    rejected.regions = vec![selector("us-east")];
    rejected.retention_seconds = 1;
    rejected.training_use = true;
    let policy = decision(
        PolicyEffect::Allow,
        vec![
            GovernanceObligation::AllowProvider(selector("openai")),
            GovernanceObligation::DenyProvider(selector("anthropic")),
            GovernanceObligation::MinimumProviderTrust(ProviderTrustTier::RestrictedApproved),
            GovernanceObligation::RequireRegion(selector("eu-central")),
            GovernanceObligation::RequireLocalExecution,
            GovernanceObligation::ProhibitRetention,
            GovernanceObligation::RetentionSeconds(0),
            GovernanceObligation::ProhibitTrainingUse,
        ],
    );
    let plan = plan_with_rejected_candidate(tenant, rejected, &policy);
    let rejected = plan
        .candidate_evaluations
        .iter()
        .find(|candidate| candidate.provider == ProviderId::Anthropic)
        .unwrap();

    assert_eq!(
        rejected.rejection_reasons().len(),
        MAX_GOVERNED_HARD_FILTER_REASONS
    );
    assert_eq!(plan.candidate_evaluations.len(), 2);
    assert_eq!(plan.primary.provider, ProviderId::OpenAi);
    assert!(
        plan.fallbacks
            .iter()
            .all(|route| route.provider != ProviderId::Anthropic)
    );
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
    request.weights = GovernedRoutingWeights {
        health: 10_000,
        load: 1,
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
    request.weights = GovernedRoutingWeights {
        health: 10_001,
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
    invalid.signals.health = Some(10_001);
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
    assert!(!plan_debug.contains("9000"));
    assert!(plan_debug.contains("routing.score.health"));
}

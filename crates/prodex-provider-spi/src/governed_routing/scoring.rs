use super::*;

#[cfg(feature = "mojo")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NormalizedRoutingScoreInput {
    pub(super) health: i64,
    pub(super) load: i64,
    pub(super) quota_headroom: i64,
    pub(super) quota_present: bool,
    pub(super) cost: i64,
    pub(super) latency: i64,
    pub(super) risk: i64,
    pub(super) priority: i64,
    pub(super) affinity: bool,
}

#[cfg(feature = "mojo")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MojoRoutingScore {
    pub(super) components: [u16; GOVERNED_SCORE_COMPONENT_COUNT],
    pub(super) weighted_total: u64,
    pub(super) score: u16,
}

pub(super) fn score_providers(
    providers: &[&GovernedProviderDescriptor],
    request: &GovernedRoutingRequest<'_>,
) -> Result<Vec<GovernedScoreBreakdown>, GovernedRoutingError> {
    #[cfg(feature = "mojo")]
    let inputs = providers
        .iter()
        .map(|provider| normalized_routing_score_input(provider, request))
        .collect::<Vec<_>>();

    #[cfg(feature = "mojo")]
    {
        let scores = mojo::score_batch(&inputs, request.weights)
            .ok_or(GovernedRoutingError::InvalidWeights)?;
        Ok(scores
            .into_iter()
            .map(|score| score_breakdown_from_mojo(score, request.score_revision, request.weights))
            .collect())
    }

    #[cfg(not(feature = "mojo"))]
    {
        Ok(providers
            .iter()
            .map(|provider| score_provider_rust(provider, request))
            .collect())
    }
}

#[cfg(feature = "mojo")]
fn normalized_routing_score_input(
    provider: &GovernedProviderDescriptor,
    request: &GovernedRoutingRequest<'_>,
) -> NormalizedRoutingScoreInput {
    NormalizedRoutingScoreInput {
        health: i64::from(provider.signals.health.unwrap_or(ROUTING_SCORE_SCALE / 2)),
        load: i64::from(provider.signals.load),
        quota_headroom: i64::from(provider.signals.quota_headroom.unwrap_or(0)),
        quota_present: provider.signals.quota_headroom.is_some(),
        cost: i64::from(provider.signals.cost),
        latency: i64::from(provider.signals.latency),
        risk: i64::from(provider.signals.risk),
        priority: i64::from(provider.signals.priority),
        affinity: request.affinity_provider == Some(provider.provider),
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn score_provider_rust(
    provider: &GovernedProviderDescriptor,
    request: &GovernedRoutingRequest<'_>,
) -> GovernedScoreBreakdown {
    let weights = request.weights;
    let inverse = |value: u16| ROUTING_SCORE_SCALE - value;
    let affinity = if request.affinity_provider == Some(provider.provider) {
        ROUTING_SCORE_SCALE
    } else {
        0
    };
    let components = [
        score_component(
            GovernedScoreComponentKind::Health,
            provider.signals.health.unwrap_or(ROUTING_SCORE_SCALE / 2),
            weights.health,
        ),
        score_component(
            GovernedScoreComponentKind::AvailableCapacity,
            provider.signals.quota_headroom.map_or_else(
                || inverse(provider.signals.load),
                |quota| quota.min(inverse(provider.signals.load)),
            ),
            weights.load,
        ),
        score_component(
            GovernedScoreComponentKind::CostEfficiency,
            inverse(provider.signals.cost),
            weights.cost,
        ),
        score_component(
            GovernedScoreComponentKind::LatencyEfficiency,
            inverse(provider.signals.latency),
            weights.latency,
        ),
        score_component(
            GovernedScoreComponentKind::RiskReduction,
            inverse(provider.signals.risk),
            weights.risk,
        ),
        score_component(
            GovernedScoreComponentKind::OperatorPriority,
            provider.signals.priority,
            weights.priority,
        ),
        score_component(
            GovernedScoreComponentKind::Affinity,
            affinity,
            weights.affinity,
        ),
    ];
    let weighted_total = components
        .iter()
        .map(|component| component.weighted_value)
        .sum::<u64>();
    let weight_total = request.weights.total().unwrap_or(1) as u16;
    GovernedScoreBreakdown {
        score_revision: request.score_revision,
        components,
        weighted_total,
        weight_total,
        score: (weighted_total / u64::from(weight_total)) as u16,
    }
}

#[cfg(feature = "mojo")]
fn score_breakdown_from_mojo(
    score: MojoRoutingScore,
    score_revision: u64,
    weights: GovernedRoutingWeights,
) -> GovernedScoreBreakdown {
    let kinds = [
        GovernedScoreComponentKind::Health,
        GovernedScoreComponentKind::AvailableCapacity,
        GovernedScoreComponentKind::CostEfficiency,
        GovernedScoreComponentKind::LatencyEfficiency,
        GovernedScoreComponentKind::RiskReduction,
        GovernedScoreComponentKind::OperatorPriority,
        GovernedScoreComponentKind::Affinity,
    ];
    let weights = [
        weights.health,
        weights.load,
        weights.cost,
        weights.latency,
        weights.risk,
        weights.priority,
        weights.affinity,
    ];
    let components = std::array::from_fn(|index| GovernedScoreComponent {
        kind: kinds[index],
        normalized_value: score.components[index],
        weight: weights[index],
        weighted_value: u64::from(score.components[index]) * u64::from(weights[index]),
    });
    GovernedScoreBreakdown {
        score_revision,
        components,
        weighted_total: score.weighted_total,
        weight_total: weights.into_iter().map(u64::from).sum::<u64>() as u16,
        score: score.score,
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn score_component(
    kind: GovernedScoreComponentKind,
    normalized_value: u16,
    weight: u16,
) -> GovernedScoreComponent {
    GovernedScoreComponent {
        kind,
        normalized_value,
        weight,
        weighted_value: u64::from(normalized_value) * u64::from(weight),
    }
}

#[cfg(all(test, feature = "mojo"))]
#[test]
fn mojo_feature_requires_real_compiled_routing_core() {
    if prodex_mojo_core::MOJO_REQUIRED
        && (!prodex_mojo_core::MOJO_ACTIVE || prodex_mojo_core::MOJO_FALLBACK)
    {
        panic!("strict Mojo mode unexpectedly activated the Rust fallback");
    }
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_tests {
    use super::*;

    fn test_descriptor(signals: GovernedRoutingSignals) -> GovernedProviderDescriptor {
        GovernedProviderDescriptor {
            revision: 1,
            pricing_revision: 1,
            tenant: TenantContext {
                tenant_id: prodex_domain::TenantId::new(),
            },
            provider: ProviderId::OpenAi,
            credential_ref: SecretRef::new("vault", "providers/openai", Some("v1")),
            credential_available: true,
            enabled: true,
            revoked: false,
            circuit_open: false,
            quota_available: true,
            inflight_cap_reached: false,
            local_execution: true,
            trust_tier: ProviderTrustTier::RestrictedApproved,
            maximum_classification: DataClassification::Restricted,
            capabilities: CapabilitySet::new(Vec::new()),
            regions: Vec::new(),
            retention_seconds: 0,
            training_use: false,
            signals,
        }
    }

    #[test]
    fn routing_score_batch_matches_rust_oracle_for_seeded_vectors() {
        let tenant = TenantContext {
            tenant_id: prodex_domain::TenantId::new(),
        };
        let policy = PolicyDecision {
            effect: PolicyEffect::Allow,
            obligations: Vec::new(),
            reason_codes: Vec::new(),
            policy_revision: PolicyRevisionId::new(),
            valid_until_unix_ms: u64::MAX,
        };
        let required_capabilities = CapabilitySet::new(Vec::new());
        let registry = GovernedProviderRegistry {
            revision: 1,
            providers: Vec::new(),
        };
        let request = GovernedRoutingRequest {
            tenant,
            classification: DataClassification::Internal,
            required_capabilities: &required_capabilities,
            policy: &policy,
            registry: &registry,
            score_revision: 9,
            weights: GovernedRoutingWeights::default(),
            affinity_provider: Some(ProviderId::OpenAi),
            max_fallbacks: 0,
        };
        let providers = (0..MAX_GOVERNED_ROUTING_CANDIDATES)
            .map(|index| {
                test_descriptor(GovernedRoutingSignals {
                    health: (index % 10_001).try_into().ok(),
                    load: ((index * 97) % 10_001) as u16,
                    quota_headroom: (index % 3 != 0).then_some(((index * 193) % 10_001) as u16),
                    cost: ((index * 211) % 10_001) as u16,
                    latency: ((index * 307) % 10_001) as u16,
                    risk: ((index * 401) % 10_001) as u16,
                    priority: ((index * 503) % 10_001) as u16,
                })
            })
            .collect::<Vec<_>>();
        let provider_refs = providers.iter().collect::<Vec<_>>();
        let inputs = provider_refs
            .iter()
            .map(|provider| normalized_routing_score_input(provider, &request))
            .collect::<Vec<_>>();
        let mojo_scores = mojo::score_batch(&inputs, request.weights).expect("valid batch");
        let rust_scores = provider_refs
            .iter()
            .map(|provider| score_provider_rust(provider, &request))
            .collect::<Vec<_>>();

        for (index, (mojo_score, rust_score)) in mojo_scores.iter().zip(rust_scores).enumerate() {
            let mojo_breakdown =
                score_breakdown_from_mojo(*mojo_score, request.score_revision, request.weights);
            assert_eq!(mojo_breakdown, rust_score, "candidate={index}");
        }
    }
}

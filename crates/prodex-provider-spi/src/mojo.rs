use super::GovernedRoutingWeights;
use super::scoring::{MojoRoutingScore, NormalizedRoutingScoreInput};

pub(super) struct MojoRoutingPlan {
    pub(super) eligible: Vec<bool>,
    pub(super) reason_tags: Vec<u8>,
    pub(super) scores: Vec<MojoRoutingScore>,
    pub(super) ordered_indices: Vec<usize>,
}

fn mojo_weights(weights: GovernedRoutingWeights) -> prodex_mojo_core::routing::ScoreWeights {
    prodex_mojo_core::routing::ScoreWeights {
        health: i64::from(weights.health),
        load: i64::from(weights.load),
        cost: i64::from(weights.cost),
        latency: i64::from(weights.latency),
        risk: i64::from(weights.risk),
        priority: i64::from(weights.priority),
        affinity: i64::from(weights.affinity),
    }
}

pub(super) fn plan_batch(
    inputs: &[NormalizedRoutingScoreInput],
    required_capability_mask: u8,
    weights: GovernedRoutingWeights,
) -> Option<MojoRoutingPlan> {
    let inputs = inputs
        .iter()
        .map(|input| prodex_mojo_core::routing::RoutingPlanInput {
            hard_eligible: input.hard_eligible,
            capability_mask: input.capability_mask,
            provider_order: input.provider_order,
            score: prodex_mojo_core::routing::ScoreInput {
                health: input.health,
                load: input.load,
                quota_headroom: input.quota_headroom,
                quota_present: input.quota_present,
                cost: input.cost,
                latency: input.latency,
                risk: input.risk,
                priority: input.priority,
                affinity: input.affinity,
            },
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::routing::routing_plan_batch(
        &inputs,
        required_capability_mask,
        mojo_weights(weights),
    )
    .map(|plan| MojoRoutingPlan {
        eligible: plan.eligible,
        reason_tags: plan.reason_tags,
        scores: plan
            .scores
            .into_iter()
            .map(|score| MojoRoutingScore {
                components: score.components,
                weighted_total: score.weighted_total,
                score: score.score,
            })
            .collect(),
        ordered_indices: plan.ordered_indices,
    })
}

#[cfg(all(test, feature = "mojo"))]
mod tests {
    use super::super::{
        GovernedProviderDescriptor, GovernedProviderRegistry, GovernedRoutingRequest,
        GovernedRoutingSignals, GovernedRoutingWeights, plan_governed_provider_route,
        plan_governed_provider_route_rust,
    };
    use prodex_domain::{
        CapabilitySet, DataClassification, GovernanceObligation, ModelCapability, PolicyDecision,
        PolicyEffect, PolicyReasonCode, PolicyRevisionId, PolicySelector, ProviderTrustTier,
        SecretRef, TenantContext, TenantId,
    };
    use prodex_provider_core::ProviderId;

    const PROVIDERS: [ProviderId; 7] = [
        ProviderId::OpenAi,
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
        ProviderId::Local,
    ];
    const CAPABILITIES: [ModelCapability; 7] = [
        ModelCapability::ResponsesApi,
        ModelCapability::Streaming,
        ModelCapability::Tools,
        ModelCapability::Vision,
        ModelCapability::JsonMode,
        ModelCapability::RemoteCompact,
        ModelCapability::WebSocket,
    ];

    #[derive(Clone, Copy)]
    struct FixedSeed(u64);

    impl FixedSeed {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0
        }

        fn below(&mut self, upper: u64) -> u64 {
            self.next() % upper
        }
    }

    fn capability_set(mask: u8) -> CapabilitySet {
        CapabilitySet::new(
            CAPABILITIES
                .into_iter()
                .enumerate()
                .filter_map(|(index, capability)| {
                    (mask & (1_u8 << index) != 0).then_some(capability)
                })
                .collect(),
        )
    }

    fn policy() -> PolicyDecision {
        PolicyDecision {
            effect: PolicyEffect::Allow,
            obligations: vec![
                GovernanceObligation::MinimumProviderTrust(ProviderTrustTier::RestrictedApproved),
                GovernanceObligation::RequireLocalExecution,
                GovernanceObligation::RequireRegion(PolicySelector::new("eu-central").unwrap()),
                GovernanceObligation::ProhibitRetention,
                GovernanceObligation::ProhibitTrainingUse,
            ],
            reason_codes: vec![PolicyReasonCode::new("routing.mojo_test").unwrap()],
            policy_revision: PolicyRevisionId::new(),
            valid_until_unix_ms: u64::MAX,
        }
    }

    fn signals(rng: &mut FixedSeed, boundary: bool) -> GovernedRoutingSignals {
        let value = |rng: &mut FixedSeed, slot: u64| {
            if boundary {
                if slot.is_multiple_of(2) { 0 } else { 10_000 }
            } else {
                rng.below(10_001) as u16
            }
        };
        GovernedRoutingSignals {
            health: (!rng.next().is_multiple_of(4)).then(|| value(rng, 0)),
            load: value(rng, 1),
            quota_headroom: (!rng.next().is_multiple_of(3)).then(|| value(rng, 2)),
            cost: value(rng, 3),
            latency: value(rng, 4),
            risk: value(rng, 5),
            priority: value(rng, 6),
        }
    }

    fn weights(case: usize) -> GovernedRoutingWeights {
        match case % 5 {
            0 => GovernedRoutingWeights::default(),
            1 => GovernedRoutingWeights {
                health: 10_000,
                load: 0,
                cost: 0,
                latency: 0,
                risk: 0,
                priority: 0,
                affinity: 0,
            },
            2 => GovernedRoutingWeights {
                health: 0,
                load: 0,
                cost: 0,
                latency: 0,
                risk: 0,
                priority: 0,
                affinity: 10_000,
            },
            3 => GovernedRoutingWeights {
                health: 0,
                load: 10_000,
                cost: 0,
                latency: 0,
                risk: 0,
                priority: 0,
                affinity: 0,
            },
            _ => GovernedRoutingWeights {
                health: 1_000,
                load: 2_000,
                cost: 1_000,
                latency: 2_000,
                risk: 1_000,
                priority: 2_000,
                affinity: 1_000,
            },
        }
    }

    fn descriptor(
        tenant: TenantContext,
        other_tenant: TenantContext,
        provider: ProviderId,
        capabilities: CapabilitySet,
        signals: GovernedRoutingSignals,
        mode: usize,
    ) -> GovernedProviderDescriptor {
        let mut descriptor = GovernedProviderDescriptor {
            revision: 1,
            pricing_revision: 1,
            tenant,
            provider,
            credential_ref: SecretRef::new(
                "vault",
                format!("providers/{}", provider.label()),
                Some("v1"),
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
            capabilities,
            regions: vec![PolicySelector::new("eu-central").unwrap()],
            retention_seconds: 0,
            training_use: false,
            signals,
        };
        match mode {
            1 => descriptor.enabled = false,
            2 => descriptor.revoked = true,
            3 => descriptor.circuit_open = true,
            4 => descriptor.quota_available = false,
            5 => descriptor.inflight_cap_reached = true,
            6 => descriptor.credential_available = false,
            7 => descriptor.local_execution = false,
            8 => descriptor.retention_seconds = 1,
            9 => descriptor.training_use = true,
            10 => descriptor.trust_tier = ProviderTrustTier::Enterprise,
            11 => descriptor.maximum_classification = DataClassification::Public,
            12 => descriptor.capabilities = CapabilitySet::new(Vec::new()),
            13 => descriptor.tenant = other_tenant,
            14 => descriptor.regions = Vec::new(),
            _ => {}
        }
        descriptor
    }

    #[test]
    fn routing_plan_matches_rust_oracle_for_fixed_seed_batches() {
        for seed in [0x5eed_u64, 0xdec0de_u64, 0x1234_5678_u64, 0x000a_11ce_u64] {
            let mut rng = FixedSeed(seed);
            let tenant = TenantContext {
                tenant_id: TenantId::new(),
            };
            let other_tenant = TenantContext {
                tenant_id: TenantId::new(),
            };
            for case in 0..192 {
                let count = rng.below(8) as usize;
                let required_mask = match case % 11 {
                    0 => 0,
                    1 => 0x7f,
                    _ => rng.below(128) as u8,
                };
                let boundary = case % 23 == 0;
                let mut providers = PROVIDERS;
                for index in (1..providers.len()).rev() {
                    providers.swap(index, rng.below((index + 1) as u64) as usize);
                }
                let affinity_provider = (count > 0 && case % 4 == 0)
                    .then(|| providers[rng.below(count as u64) as usize]);
                let descriptors = providers
                    .into_iter()
                    .take(count)
                    .enumerate()
                    .map(|(index, provider)| {
                        descriptor(
                            tenant,
                            other_tenant,
                            provider,
                            capability_set(if case % 13 == 0 {
                                0
                            } else {
                                rng.below(128) as u8
                            }),
                            signals(&mut rng, boundary),
                            (case + index) % 15,
                        )
                    })
                    .collect::<Vec<_>>();
                let policy = policy();
                let capabilities = capability_set(required_mask);
                let registry = GovernedProviderRegistry {
                    revision: (case + 1) as u64,
                    providers: descriptors,
                };
                let request = GovernedRoutingRequest {
                    tenant,
                    classification: DataClassification::Restricted,
                    required_capabilities: &capabilities,
                    policy: &policy,
                    registry: &registry,
                    score_revision: (case + 1) as u64,
                    weights: weights(case),
                    affinity_provider,
                    max_fallbacks: rng.below(9) as usize,
                };
                let expected = plan_governed_provider_route_rust(&request);
                let actual = plan_governed_provider_route(&request);
                assert_eq!(actual, expected, "seed={seed:#x} case={case}");
            }
        }
    }

    #[test]
    fn routing_plan_matches_rust_for_ties_affinity_and_score_boundaries() {
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let policy = policy();
        let capabilities = capability_set(0b101);
        let tied_signals = GovernedRoutingSignals {
            health: Some(5_000),
            load: 5_000,
            quota_headroom: Some(5_000),
            cost: 5_000,
            latency: 5_000,
            risk: 5_000,
            priority: 5_000,
        };
        let registry = GovernedProviderRegistry {
            revision: 90,
            providers: [
                ProviderId::Local,
                ProviderId::Gemini,
                ProviderId::OpenAi,
                ProviderId::Copilot,
                ProviderId::Anthropic,
                ProviderId::Kiro,
                ProviderId::DeepSeek,
            ]
            .into_iter()
            .map(|provider| {
                descriptor(
                    tenant,
                    TenantContext {
                        tenant_id: TenantId::new(),
                    },
                    provider,
                    capabilities.clone(),
                    tied_signals,
                    0,
                )
            })
            .collect(),
        };
        let request = GovernedRoutingRequest {
            tenant,
            classification: DataClassification::Restricted,
            required_capabilities: &capabilities,
            policy: &policy,
            registry: &registry,
            score_revision: 90,
            weights: GovernedRoutingWeights::default(),
            affinity_provider: None,
            max_fallbacks: 6,
        };
        let expected = plan_governed_provider_route_rust(&request).unwrap();
        let actual = plan_governed_provider_route(&request).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            actual
                .fallbacks
                .iter()
                .map(|route| route.provider)
                .collect::<Vec<_>>(),
            vec![
                ProviderId::Anthropic,
                ProviderId::Copilot,
                ProviderId::DeepSeek,
                ProviderId::Gemini,
                ProviderId::Kiro,
                ProviderId::Local,
            ]
        );

        let affinity_request = GovernedRoutingRequest {
            affinity_provider: Some(ProviderId::Gemini),
            ..request
        };
        let expected = plan_governed_provider_route_rust(&affinity_request).unwrap();
        let actual = plan_governed_provider_route(&affinity_request).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.primary.provider, ProviderId::Gemini);
        assert!(actual.fallbacks.is_empty());

        let low = descriptor(
            tenant,
            TenantContext {
                tenant_id: TenantId::new(),
            },
            ProviderId::Anthropic,
            capabilities.clone(),
            GovernedRoutingSignals {
                health: Some(0),
                load: 0,
                quota_headroom: Some(0),
                cost: 0,
                latency: 0,
                risk: 0,
                priority: 0,
            },
            0,
        );
        let high = descriptor(
            tenant,
            TenantContext {
                tenant_id: TenantId::new(),
            },
            ProviderId::OpenAi,
            capabilities,
            GovernedRoutingSignals {
                health: Some(10_000),
                load: 10_000,
                quota_headroom: Some(10_000),
                cost: 10_000,
                latency: 10_000,
                risk: 10_000,
                priority: 10_000,
            },
            0,
        );
        let registry = GovernedProviderRegistry {
            revision: 91,
            providers: vec![low, high],
        };
        let request = GovernedRoutingRequest {
            tenant,
            classification: DataClassification::Restricted,
            required_capabilities: &capability_set(0b101),
            policy: &policy,
            registry: &registry,
            score_revision: 91,
            weights: GovernedRoutingWeights {
                health: 10_000,
                load: 0,
                cost: 0,
                latency: 0,
                risk: 0,
                priority: 0,
                affinity: 0,
            },
            affinity_provider: None,
            max_fallbacks: 1,
        };
        let expected = plan_governed_provider_route_rust(&request).unwrap();
        let actual = plan_governed_provider_route(&request).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.primary.provider, ProviderId::OpenAi);
        assert_eq!(actual.primary.score, 10_000);
        assert_eq!(actual.fallbacks[0].score, 0);
    }

    #[test]
    fn routing_plan_keeps_hard_and_capability_rejections_out_of_routes() {
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let policy = policy();
        let required = capability_set(0b101);
        let mut hard = descriptor(
            tenant,
            TenantContext {
                tenant_id: TenantId::new(),
            },
            ProviderId::Anthropic,
            required.clone(),
            GovernedRoutingSignals {
                health: Some(10_000),
                load: 0,
                quota_headroom: None,
                cost: 0,
                latency: 0,
                risk: 0,
                priority: 0,
            },
            0,
        );
        hard.enabled = false;
        let capability = descriptor(
            tenant,
            TenantContext {
                tenant_id: TenantId::new(),
            },
            ProviderId::Copilot,
            capability_set(1),
            GovernedRoutingSignals {
                health: Some(9_000),
                load: 1_000,
                quota_headroom: None,
                cost: 1_000,
                latency: 1_000,
                risk: 1_000,
                priority: 1_000,
            },
            0,
        );
        let eligible = descriptor(
            tenant,
            TenantContext {
                tenant_id: TenantId::new(),
            },
            ProviderId::OpenAi,
            required.clone(),
            GovernedRoutingSignals {
                health: Some(8_000),
                load: 2_000,
                quota_headroom: None,
                cost: 2_000,
                latency: 2_000,
                risk: 2_000,
                priority: 2_000,
            },
            0,
        );
        let registry = GovernedProviderRegistry {
            revision: 92,
            providers: vec![hard, capability, eligible],
        };
        let request = GovernedRoutingRequest {
            tenant,
            classification: DataClassification::Restricted,
            required_capabilities: &required,
            policy: &policy,
            registry: &registry,
            score_revision: 92,
            weights: GovernedRoutingWeights::default(),
            affinity_provider: None,
            max_fallbacks: 2,
        };
        let expected = plan_governed_provider_route_rust(&request).unwrap();
        let actual = plan_governed_provider_route(&request).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.primary.provider, ProviderId::OpenAi);
        assert_eq!(actual.fallbacks.len(), 0);
        let hard = actual
            .candidate_evaluations
            .iter()
            .find(|candidate| candidate.provider == ProviderId::Anthropic)
            .unwrap();
        assert_eq!(
            hard.rejection_reasons(),
            &[super::super::GovernedHardFilterReason::ProviderDisabled]
        );
        let capability = actual
            .candidate_evaluations
            .iter()
            .find(|candidate| candidate.provider == ProviderId::Copilot)
            .unwrap();
        assert_eq!(
            capability.rejection_reasons(),
            &[super::super::GovernedHardFilterReason::CapabilityMissing]
        );
    }
}

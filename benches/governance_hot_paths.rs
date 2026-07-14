use criterion::{Criterion, criterion_group, criterion_main};
use prodex_application::{
    ApplicationInspectionRequest, ApplicationInspectionSource, ApplicationObligationContext,
    ApplicationObligationMode, ApplicationResponseTransport, plan_application_obligation_execution,
    plan_application_request_inspection,
};
use prodex_domain::{
    CanonicalRoute, CapabilitySet, Channel, ClassificationRequest, ClassificationRule,
    ClassificationRuleSet, ClassificationRuleSetChecksum, ClassificationRuleSetRevisionId,
    ContentLocation, CredentialScope, DataClassification, DataModality, DataPolicyContext,
    DetectorId, DetectorRevisionId, EnvironmentContext, FindingKind, GovernanceObligation,
    GovernancePolicyArtifact, GovernancePolicyRule, GovernancePolicyRuleId, GovernedAction,
    InspectionCoverage, InspectionFinding, InspectionLimits, InspectionReasonCode, NetworkZone,
    PolicyDecision, PolicyEffect, PolicyInput, PolicyReasonCode, PolicyRevisionId,
    PolicyRuleCondition, Principal, PrincipalId, PrincipalKind, PrincipalPolicyAttributes,
    ProviderTrustTier, QuotaContext, RequestPolicyAttributes, RequestRisk, Role, SecretRef,
    SessionPolicyContext, TenantContext, TenantId, classify_inspection,
    compile_classification_rule_set, compile_governance_policy, evaluate_governance_policy,
};
use prodex_provider_core::ProviderId;
use prodex_provider_spi::{
    GovernedProviderDescriptor, GovernedProviderRegistry, GovernedRoutingRequest,
    GovernedRoutingSignals, GovernedRoutingWeights, MAX_GOVERNED_ROUTING_CANDIDATES,
    plan_governed_provider_route,
};
use std::hint::black_box;
use std::sync::Arc;

use arc_swap::ArcSwap;

fn policy_condition() -> PolicyRuleCondition {
    PolicyRuleCondition::default()
}

fn benchmark_governance_hot_paths(c: &mut Criterion) {
    let limits = InspectionLimits::default();
    let max_findings = (0..limits.max_findings)
        .map(|index| {
            InspectionFinding::new(
                FindingKind::ApiKey,
                ContentLocation::new(format!("$.input[{index}]"), index, index + 1).unwrap(),
                9_900,
                DetectorId::new("local.bounded-v1").unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let max_inspection_source = ApplicationInspectionSource {
        coverage: InspectionCoverage::Full,
        findings: max_findings,
        masked_findings: vec![FindingKind::ApiKey],
        tags: Vec::new(),
        reason_codes: vec![InspectionReasonCode::new("detector.finding").unwrap()],
    };
    let many_inspection_source = ApplicationInspectionSource {
        findings: max_inspection_source.findings[..16].to_vec(),
        ..max_inspection_source.clone()
    };
    let unicode = "账户🙂résumé-東京-δοκιμή";
    let unicode_inspection_source = ApplicationInspectionSource {
        coverage: InspectionCoverage::Full,
        findings: unicode
            .char_indices()
            .enumerate()
            .map(|(index, (start, character))| {
                InspectionFinding::new(
                    FindingKind::ALL[index % FindingKind::ALL.len()],
                    ContentLocation::new(
                        format!("$.unicode[{index}]"),
                        start,
                        start + character.len_utf8(),
                    )
                    .unwrap(),
                    9_900,
                    DetectorId::new("local.unicode-v1").unwrap(),
                )
                .unwrap()
            })
            .collect(),
        masked_findings: Vec::new(),
        tags: Vec::new(),
        reason_codes: vec![InspectionReasonCode::new("detector.unicode").unwrap()],
    };
    for (name, sources) in [
        ("governance_inspection_none", Vec::new()),
        (
            "governance_inspection_many",
            vec![many_inspection_source.clone()],
        ),
        (
            "governance_inspection_max_findings",
            vec![max_inspection_source],
        ),
        (
            "governance_inspection_unicode",
            vec![unicode_inspection_source],
        ),
    ] {
        c.bench_function(name, |b| {
            b.iter(|| {
                black_box(plan_application_request_inspection(
                    ApplicationInspectionRequest {
                        sources: sources.clone(),
                        default_classification: DataClassification::Public,
                        trusted_label: None,
                        prior_classification: None,
                        detector_revision: DetectorRevisionId::new("bounded-v1").unwrap(),
                        limits,
                    },
                ))
            })
        });
    }

    let tenant = TenantContext {
        tenant_id: TenantId::new(),
    };
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant.tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::DataPlane,
    );
    let route = CanonicalRoute::new("responses").unwrap();
    let capabilities = CapabilitySet::new(Vec::new());
    let rules = (0..prodex_domain::MAX_GOVERNANCE_POLICY_RULES)
        .map(|index| GovernancePolicyRule {
            id: GovernancePolicyRuleId::new(format!("bench.rule.{index}")).unwrap(),
            condition: PolicyRuleCondition {
                route: (index >= 31).then(|| CanonicalRoute::new("never-match").unwrap()),
                ..policy_condition()
            },
            effect: PolicyEffect::Allow,
            obligations: Vec::new(),
            reason_code: PolicyReasonCode::new(format!("bench.reason.{index}")).unwrap(),
        })
        .collect::<Vec<_>>();
    let mut representative_rules = rules[..8].to_vec();
    representative_rules[0].obligations = vec![
        GovernanceObligation::MaxInputTokens(4_096),
        GovernanceObligation::RequireResponseInspection,
    ];
    let representative_policy = compile_governance_policy(GovernancePolicyArtifact {
        revision: PolicyRevisionId::new(),
        valid_until_unix_ms: u64::MAX,
        default_effect: PolicyEffect::Deny,
        rules: representative_rules,
    })
    .unwrap();
    let policy = compile_governance_policy(GovernancePolicyArtifact {
        revision: PolicyRevisionId::new(),
        valid_until_unix_ms: u64::MAX,
        default_effect: PolicyEffect::Deny,
        rules,
    })
    .unwrap();
    let principal_attributes =
        PrincipalPolicyAttributes::new(Some("bench-team"), Some("bench-project"), None).unwrap();
    let request_attributes =
        RequestPolicyAttributes::new(Some("bench-model"), &[], vec![DataModality::Text], None, 0)
            .unwrap();
    let policy_input = PolicyInput {
        tenant,
        principal: &principal,
        principal_attributes: &principal_attributes,
        channel: Channel::Api,
        credential_scope: CredentialScope::DataPlane,
        session: SessionPolicyContext {
            age_seconds: 1,
            idle_seconds: 0,
            revoked: false,
            mfa_satisfied: true,
            retained_classification: DataClassification::Internal,
        },
        action: GovernedAction::InvokeModel,
        route: &route,
        data: DataPolicyContext {
            classification: DataClassification::Internal,
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
        },
    };
    c.bench_function("governance_policy_representative", |b| {
        b.iter(|| {
            black_box(evaluate_governance_policy(
                black_box(&representative_policy),
                black_box(&policy_input),
            ))
        })
    });
    c.bench_function("governance_policy_max_rules", |b| {
        b.iter(|| {
            black_box(evaluate_governance_policy(
                black_box(&policy),
                black_box(&policy_input),
            ))
        })
    });

    let obligation_policy = PolicyDecision {
        effect: PolicyEffect::Allow,
        obligations: vec![
            GovernanceObligation::MaskFinding(FindingKind::ApiKey),
            GovernanceObligation::MaxInputTokens(8_192),
            GovernanceObligation::MaxInputTokens(4_096),
            GovernanceObligation::MaxContextTokens(32_768),
            GovernanceObligation::MaxContextTokens(16_384),
            GovernanceObligation::RequireResponseInspection,
        ],
        reason_codes: vec![PolicyReasonCode::new("bench.obligations").unwrap()],
        policy_revision: PolicyRevisionId::new(),
        valid_until_unix_ms: u64::MAX,
    };
    let obligation_findings = [FindingKind::ApiKey];
    c.bench_function("governance_obligation_merge_representative", |b| {
        b.iter(|| {
            black_box(plan_application_obligation_execution(
                black_box(&obligation_policy),
                ApplicationObligationContext {
                    mode: ApplicationObligationMode::Enforce,
                    classification: DataClassification::Confidential,
                    inspection_coverage: InspectionCoverage::Full,
                    detected_findings: &obligation_findings,
                    masked_findings: &obligation_findings,
                    requested_capabilities: &capabilities,
                    requested_model: Some("example-model"),
                    requested_tools: None,
                    requested_modalities: &[],
                    estimated_input_tokens: 2_048,
                    estimated_context_tokens: 8_192,
                    requested_output_tokens: Some(1_024),
                    session: SessionPolicyContext {
                        age_seconds: 1,
                        idle_seconds: 0,
                        revoked: false,
                        mfa_satisfied: true,
                        retained_classification: DataClassification::Internal,
                    },
                    environment: EnvironmentContext {
                        network_zone: NetworkZone::TrustedInternal,
                        authentication_strength: 3,
                        mfa_satisfied: true,
                    },
                    response_transport: ApplicationResponseTransport::ServerSentEvents,
                    response_inspection_coverage: InspectionCoverage::Full,
                },
            ))
        })
    });

    let classification_rules = compile_classification_rule_set(ClassificationRuleSet {
        revision: ClassificationRuleSetRevisionId::new("bench-v1").unwrap(),
        checksum: ClassificationRuleSetChecksum::new("bench-v1").unwrap(),
        unsupported_coverage_floor: DataClassification::Restricted,
        rules: FindingKind::ALL
            .into_iter()
            .map(|finding_kind| ClassificationRule {
                finding_kind,
                classification: finding_kind.minimum_classification(),
            })
            .collect(),
    })
    .unwrap();
    let classification_inspection =
        plan_application_request_inspection(ApplicationInspectionRequest {
            sources: vec![many_inspection_source],
            default_classification: DataClassification::Public,
            trusted_label: None,
            prior_classification: None,
            detector_revision: DetectorRevisionId::new("bounded-v1").unwrap(),
            limits,
        })
        .unwrap();
    c.bench_function("governance_classification_obligation_merge", |b| {
        b.iter(|| {
            let classification = classify_inspection(
                black_box(&classification_rules),
                ClassificationRequest {
                    inspection: &classification_inspection.result,
                    trusted_label: None,
                    untrusted_label: None,
                    prior_classification: None,
                    session_floor: DataClassification::Internal,
                    route_floor: DataClassification::Public,
                    request_risk_floor: DataClassification::Public,
                },
            )
            .unwrap();
            black_box(plan_application_obligation_execution(
                black_box(&obligation_policy),
                ApplicationObligationContext {
                    mode: ApplicationObligationMode::Enforce,
                    classification: classification.classification(),
                    inspection_coverage: classification.coverage(),
                    detected_findings: &obligation_findings,
                    masked_findings: &obligation_findings,
                    requested_capabilities: &capabilities,
                    requested_model: Some("example-model"),
                    requested_tools: None,
                    requested_modalities: &[],
                    estimated_input_tokens: 2_048,
                    estimated_context_tokens: 8_192,
                    requested_output_tokens: Some(1_024),
                    session: SessionPolicyContext {
                        age_seconds: 1,
                        idle_seconds: 0,
                        revoked: false,
                        mfa_satisfied: true,
                        retained_classification: DataClassification::Internal,
                    },
                    environment: EnvironmentContext {
                        network_zone: NetworkZone::TrustedInternal,
                        authentication_strength: 3,
                        mfa_satisfied: true,
                    },
                    response_transport: ApplicationResponseTransport::ServerSentEvents,
                    response_inspection_coverage: InspectionCoverage::Full,
                },
            ))
        })
    });

    let routing_capabilities = CapabilitySet::new(Vec::new());
    let routing_policy = PolicyDecision {
        effect: PolicyEffect::Allow,
        obligations: Vec::new(),
        reason_codes: vec![PolicyReasonCode::new("bench.allow").unwrap()],
        policy_revision: PolicyRevisionId::new(),
        valid_until_unix_ms: u64::MAX,
    };
    let provider_ids = [
        ProviderId::OpenAi,
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
        ProviderId::Local,
    ];
    let providers = (0..MAX_GOVERNED_ROUTING_CANDIDATES)
        .map(|index| GovernedProviderDescriptor {
            revision: index as u64 + 1,
            pricing_revision: 1,
            tenant: if index + 1 == MAX_GOVERNED_ROUTING_CANDIDATES {
                tenant
            } else {
                TenantContext {
                    tenant_id: TenantId::new(),
                }
            },
            provider: provider_ids[index % provider_ids.len()],
            credential_ref: SecretRef::new("vault", format!("provider/{index}"), Some("v1")),
            credential_available: true,
            enabled: true,
            revoked: false,
            circuit_open: false,
            quota_available: true,
            inflight_cap_reached: false,
            local_execution: false,
            trust_tier: ProviderTrustTier::Enterprise,
            maximum_classification: DataClassification::Confidential,
            capabilities: routing_capabilities.clone(),
            regions: Vec::new(),
            retention_seconds: 0,
            training_use: false,
            signals: GovernedRoutingSignals {
                health: Some(9_000),
                load: (index as u16).saturating_mul(100),
                quota_headroom: None,
                cost: 2_000,
                latency: 3_000,
                risk: 2_000,
                priority: 5_000,
            },
        })
        .collect();
    let registry = GovernedProviderRegistry {
        revision: 1,
        providers,
    };
    c.bench_function("governance_routing_max_candidates", |b| {
        b.iter(|| {
            black_box(plan_governed_provider_route(&GovernedRoutingRequest {
                tenant,
                classification: DataClassification::Internal,
                required_capabilities: &routing_capabilities,
                policy: &routing_policy,
                registry: &registry,
                score_revision: 1,
                weights: GovernedRoutingWeights::default(),
                affinity_provider: Some(ProviderId::OpenAi),
                max_fallbacks: 3,
            }))
        })
    });

    let mut openai = registry.providers.last().unwrap().clone();
    openai.provider = ProviderId::OpenAi;
    openai.credential_ref = SecretRef::new("vault", "provider/openai", Some("v1"));
    let mut anthropic = openai.clone();
    anthropic.revision += 1;
    anthropic.provider = ProviderId::Anthropic;
    anthropic.credential_ref = SecretRef::new("vault", "provider/anthropic", Some("v1"));
    anthropic.signals.cost = 1_000;
    let mut gemini = openai.clone();
    gemini.revision += 2;
    gemini.provider = ProviderId::Gemini;
    gemini.credential_ref = SecretRef::new("vault", "provider/gemini", Some("v1"));
    gemini.signals.latency = 1_000;
    let representative_registry = GovernedProviderRegistry {
        revision: 2,
        providers: vec![openai, anthropic, gemini],
    };
    c.bench_function("governance_routing_representative", |b| {
        b.iter(|| {
            black_box(plan_governed_provider_route(&GovernedRoutingRequest {
                tenant,
                classification: DataClassification::Internal,
                required_capabilities: &routing_capabilities,
                policy: &routing_policy,
                registry: &representative_registry,
                score_revision: 2,
                weights: GovernedRoutingWeights::default(),
                affinity_provider: Some(ProviderId::OpenAi),
                max_fallbacks: 2,
            }))
        })
    });

    let mut circuit_registry = representative_registry.clone();
    circuit_registry.revision = 3;
    circuit_registry.providers[0].circuit_open = true;
    c.bench_function("governance_routing_circuit_filter_with_fallback", |b| {
        b.iter(|| {
            black_box(plan_governed_provider_route(&GovernedRoutingRequest {
                tenant,
                classification: DataClassification::Internal,
                required_capabilities: &routing_capabilities,
                policy: &routing_policy,
                registry: &circuit_registry,
                score_revision: 2,
                weights: GovernedRoutingWeights::default(),
                affinity_provider: Some(ProviderId::OpenAi),
                max_fallbacks: 1,
            }))
        })
    });

    let registry_snapshot = Arc::new(representative_registry);
    let registry_swap = ArcSwap::from(Arc::clone(&registry_snapshot));
    c.bench_function("governance_registry_snapshot_read", |b| {
        b.iter(|| black_box(registry_swap.load_full()))
    });
    c.bench_function("governance_registry_snapshot_swap", |b| {
        b.iter(|| registry_swap.store(Arc::clone(&registry_snapshot)))
    });
}

criterion_group!(benches, benchmark_governance_hot_paths);
criterion_main!(benches);

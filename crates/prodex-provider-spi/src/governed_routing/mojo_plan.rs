use super::*;
use prodex_domain::ModelCapability;

pub(super) fn plan_governed_provider_route_mojo(
    request: &GovernedRoutingRequest<'_>,
) -> Result<GovernedRoutingPlan, GovernedRoutingError> {
    let inputs = request
        .registry
        .providers
        .iter()
        .map(|provider| {
            let hard_eligible =
                provider_rejection_reasons_without_capability(provider, request).is_empty();
            let mut input = scoring::normalized_routing_score_input(provider, request);
            input.hard_eligible = hard_eligible;
            input.capability_mask = capability_mask(&provider.capabilities);
            input.provider_order = provider_order(provider.provider);
            input
        })
        .collect::<Vec<_>>();
    let plan = mojo::plan_batch(
        &inputs,
        capability_mask(request.required_capabilities),
        request.weights,
    )
    .map_err(|_| GovernedRoutingError::InvalidWeights)?;
    validate_plan_shape(&plan, request.registry.providers.len())?;

    let score_breakdowns = plan
        .scores
        .iter()
        .copied()
        .map(|score| {
            scoring::score_breakdown_from_mojo(score, request.score_revision, request.weights)
        })
        .collect::<Vec<_>>();
    let routes = routes_from_plan(request, &plan, &score_breakdowns)?;
    let candidate_evaluations = candidate_evaluations_from_plan(request, &plan, &score_breakdowns)?;

    if let Some(affinity_provider) = request.affinity_provider
        && !routes
            .iter()
            .any(|route| route.provider == affinity_provider)
    {
        return Err(GovernedRoutingError::NoEligibleProvider);
    }

    let primary = routes
        .first()
        .cloned()
        .ok_or(GovernedRoutingError::NoEligibleProvider)?;
    let fallbacks = if request.affinity_provider.is_some() {
        Vec::new()
    } else {
        routes
            .into_iter()
            .skip(1)
            .take(request.max_fallbacks)
            .collect()
    };
    Ok(GovernedRoutingPlan {
        tenant: request.tenant,
        registry_revision: request.registry.revision,
        score_revision: request.score_revision,
        policy_revision: request.policy.policy_revision,
        primary,
        fallbacks,
        candidate_evaluations,
    })
}

fn validate_plan_shape(
    plan: &mojo::MojoRoutingPlan,
    provider_count: usize,
) -> Result<(), GovernedRoutingError> {
    (plan.eligible.len() == provider_count
        && plan.reason_tags.len() == provider_count
        && plan.scores.len() == provider_count)
        .then_some(())
        .ok_or(GovernedRoutingError::InvalidWeights)
}

fn routes_from_plan(
    request: &GovernedRoutingRequest<'_>,
    plan: &mojo::MojoRoutingPlan,
    score_breakdowns: &[GovernedScoreBreakdown],
) -> Result<Vec<GovernedRoute>, GovernedRoutingError> {
    let mut routes = Vec::with_capacity(plan.ordered_indices.len());
    for &index in &plan.ordered_indices {
        let Some(provider) = request.registry.providers.get(index) else {
            return Err(GovernedRoutingError::InvalidWeights);
        };
        if !plan.eligible[index]
            || plan.reason_tags[index] != prodex_mojo_core::routing::ROUTING_REASON_ELIGIBLE
        {
            return Err(GovernedRoutingError::InvalidWeights);
        }
        let score_breakdown = score_breakdowns[index].clone();
        routes.push(GovernedRoute {
            provider: provider.provider,
            descriptor_revision: provider.revision,
            pricing_revision: provider.pricing_revision,
            credential_ref: provider.credential_ref.clone(),
            score: score_breakdown.score,
            score_breakdown,
        });
    }
    Ok(routes)
}

fn candidate_evaluations_from_plan(
    request: &GovernedRoutingRequest<'_>,
    plan: &mojo::MojoRoutingPlan,
    score_breakdowns: &[GovernedScoreBreakdown],
) -> Result<Vec<GovernedCandidateEvaluation>, GovernedRoutingError> {
    request
        .registry
        .providers
        .iter()
        .enumerate()
        .map(|(index, provider)| {
            if plan.eligible[index] {
                if plan.reason_tags[index] != prodex_mojo_core::routing::ROUTING_REASON_ELIGIBLE {
                    return Err(GovernedRoutingError::InvalidWeights);
                }
                Ok(GovernedCandidateEvaluation {
                    provider: provider.provider,
                    descriptor_revision: provider.revision,
                    outcome: GovernedCandidateOutcome::Eligible,
                    rejection_reasons: Vec::new(),
                    score_breakdown: Some(score_breakdowns[index].clone()),
                })
            } else {
                if !matches!(
                    plan.reason_tags[index],
                    prodex_mojo_core::routing::ROUTING_REASON_HARD_REJECTED
                        | prodex_mojo_core::routing::ROUTING_REASON_CAPABILITY_MISSING
                ) {
                    return Err(GovernedRoutingError::InvalidWeights);
                }
                let rejection_reasons = provider_rejection_reasons(provider, request);
                if rejection_reasons.is_empty() {
                    return Err(GovernedRoutingError::InvalidWeights);
                }
                Ok(GovernedCandidateEvaluation {
                    provider: provider.provider,
                    descriptor_revision: provider.revision,
                    outcome: GovernedCandidateOutcome::Rejected,
                    rejection_reasons,
                    score_breakdown: None,
                })
            }
        })
        .collect()
}

fn capability_mask(capabilities: &CapabilitySet) -> u8 {
    capabilities.as_slice().iter().fold(0, |mask, capability| {
        mask | match capability {
            ModelCapability::ResponsesApi => 1 << 0,
            ModelCapability::Streaming => 1 << 1,
            ModelCapability::Tools => 1 << 2,
            ModelCapability::Vision => 1 << 3,
            ModelCapability::JsonMode => 1 << 4,
            ModelCapability::RemoteCompact => 1 << 5,
            ModelCapability::WebSocket => 1 << 6,
        }
    })
}

const fn provider_order(provider: ProviderId) -> i64 {
    match provider {
        ProviderId::OpenAi => 0,
        ProviderId::Anthropic => 1,
        ProviderId::Copilot => 2,
        ProviderId::DeepSeek => 3,
        ProviderId::Gemini => 4,
        ProviderId::Kiro => 5,
        ProviderId::Local => 6,
    }
}

fn provider_rejection_reasons_without_capability(
    provider: &GovernedProviderDescriptor,
    request: &GovernedRoutingRequest<'_>,
) -> Vec<GovernedHardFilterReason> {
    let mut reasons = Vec::with_capacity(MAX_GOVERNED_HARD_FILTER_REASONS);
    append_provider_static_rejection_reasons(provider, request, &mut reasons);
    append_provider_obligation_rejection_reasons(
        provider,
        &request.policy.obligations,
        &mut reasons,
    );
    reasons
}

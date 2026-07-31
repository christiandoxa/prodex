use super::*;

pub(super) fn legacy_constraint_route_plan(
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    body: &[u8],
    input: &RuntimeGatewayConstraintPlanInput<'_>,
    hard_affinity: bool,
) -> RuntimeGatewayConstraintRoutePlan {
    let mut plan = legacy_route_plan(provider, endpoint, body, input, hard_affinity);
    if input.hard_affinity_required && input.hard_affinity_model.is_none() {
        let reason = ProviderRequestConstraintDecision::AffinityOwnerUnavailable;
        plan.alias_chain = vec![plan.requested_model.clone()];
        plan.concrete_candidates.clear();
        plan.selected_model = None;
        plan.upstream_attempt_model = None;
        plan.body_rewrite_required = false;
        plan.adjustment = None;
        plan.no_route_reason = Some(reason);
        plan.adaptive_decision = None;
        plan.trace = affinity_owner_unavailable_trace(endpoint, &plan.requested_model);
    }
    plan
}

pub(super) fn affinity_owner_unavailable_plan(
    endpoint: ProviderEndpoint,
    requested_model: String,
    requirements: ProviderRequestRequirements,
) -> RuntimeGatewayConstraintRoutePlan {
    let reason = ProviderRequestConstraintDecision::AffinityOwnerUnavailable;
    let trace = affinity_owner_unavailable_trace(endpoint, &requested_model);
    RuntimeGatewayConstraintRoutePlan {
        requested_model: requested_model.clone(),
        alias_chain: vec![requested_model],
        concrete_candidates: Vec::new(),
        requirements,
        selected_model: None,
        upstream_attempt_model: None,
        body_rewrite_required: false,
        adjustment: None,
        no_route_reason: Some(reason),
        trace,
        truncated: false,
        selection_pool_truncated: false,
        omitted_candidates: 0,
        adaptive_decision: None,
    }
}

pub(super) fn resolve_constraint_models(
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    requested_model: &str,
    alias: Option<&RuntimeGatewayRouteAlias>,
    hard_affinity_model: Option<&str>,
    hard_affinity: bool,
) -> (Vec<String>, usize, Vec<String>, bool) {
    let mut alias_chain = vec![requested_model.to_string()];
    let mut alias_chain_truncated = false;
    let (mut models, omitted) = if let Some(model) = hard_affinity_model {
        concrete_models(provider, endpoint, std::iter::once(model))
    } else if let Some(alias) = alias {
        let visible_models =
            crate::RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES.saturating_sub(alias_chain.len());
        alias_chain_truncated = alias.models.len() > visible_models;
        alias_chain.extend(alias.models.iter().take(visible_models).cloned());
        concrete_models(provider, endpoint, alias.models.iter().map(String::as_str))
    } else {
        concrete_models(provider, endpoint, std::iter::once(requested_model))
    };
    if hard_affinity {
        models.truncate(1);
    }
    (models, omitted, alias_chain, alias_chain_truncated)
}

pub(super) fn evaluate_constraint_candidate(
    provider: ProviderId,
    model: String,
    original_order: usize,
    requirements: &ProviderRequestRequirements,
    configured_reasoning_reserve_tokens: Option<u64>,
    policy: ProviderRequestConstraintPolicy,
) -> RuntimeGatewayConstraintCandidate {
    let mut candidate_requirements = requirements.clone();
    let translated_reasoning_reserve = (provider == ProviderId::Gemini)
        .then(|| {
            if matches!(
                candidate_requirements.reasoning_effort,
                Some(ProviderReasoningEffort::None | ProviderReasoningEffort::Minimal)
            ) {
                Some(0)
            } else if gemini_provider_core_model_uses_thinking_level(&model) {
                None
            } else {
                configured_reasoning_reserve_tokens
            }
        })
        .flatten();
    if let Some(translated_reserve) = translated_reasoning_reserve {
        candidate_requirements.reasoning_reserve_tokens = Some(translated_reserve);
        candidate_requirements.total_required_tokens = candidate_requirements
            .estimated_input_tokens
            .saturating_add(
                candidate_requirements
                    .explicit_output_tokens
                    .unwrap_or_default(),
            )
            .saturating_add(translated_reserve);
    }
    let evaluation =
        evaluate_provider_request_constraints(provider, &model, &candidate_requirements, policy);
    let model = evaluation
        .requirements
        .resolved_upstream_model
        .clone()
        .unwrap_or(model);
    RuntimeGatewayConstraintCandidate {
        model,
        original_order,
        selected: false,
        evaluation,
    }
}

pub(super) fn baseline_constraint_selected_model(
    eligible_models: &[String],
    hard_affinity: bool,
    alias: Option<&RuntimeGatewayRouteAlias>,
    diagnostic_seed: u64,
    model_state: &BTreeMap<String, RuntimeGatewayRouteModelState>,
    required_tokens: u64,
) -> Option<String> {
    if eligible_models.is_empty() {
        None
    } else if hard_affinity || eligible_models.len() == 1 {
        eligible_models.first().cloned()
    } else if let Some(alias) = alias {
        runtime_gateway_route_selected_model_from_models(
            alias,
            eligible_models,
            diagnostic_seed,
            model_state,
            required_tokens,
        )
    } else {
        Some(format!("combo:{}", eligible_models.join(",")))
    }
}

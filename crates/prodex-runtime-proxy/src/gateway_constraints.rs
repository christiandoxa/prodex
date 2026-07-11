//! Side-effect-free gateway alias planning with model-aware request constraints.

use std::collections::{BTreeMap, BTreeSet};

use prodex_provider_core::{
    ProviderEndpoint, ProviderId, ProviderOutputAdjustment, ProviderReasoningEffort,
    ProviderRequestConstraintDecision, ProviderRequestConstraintEvaluation,
    ProviderRequestConstraintPolicy, ProviderRequestFeature, ProviderRequestLimitError,
    ProviderRequestRequirements, estimate_request_input_tokens,
    evaluate_provider_request_constraints, gemini_provider_core_model_uses_thinking_level,
    provider_model_fallback_chain, provider_request_requirements_from_value,
};
use serde::{Deserialize, Serialize};

use crate::gateway_policy::runtime_gateway_route_selected_model_from_models;
use crate::{
    RuntimeGatewayRouteAlias, RuntimeGatewayRouteModelState, RuntimeGatewayRouteStrategy,
    RuntimeRouteAffinityKind, RuntimeRouteAffinityOutcome, RuntimeRouteCandidateClass,
    RuntimeRouteCandidateDecisionInput, RuntimeRouteCandidateEligibility,
    RuntimeRouteDecisionDiagnostics, RuntimeRouteDecisionReason, RuntimeRouteDecisionRoute,
    RuntimeRouteDecisionStage, RuntimeRouteDecisionStageOutcome,
    RuntimeRouteDecisionTerminalOutcome, RuntimeRouteDecisionTrace,
    RuntimeRouteDecisionTraceBuilder, runtime_gateway_request_model,
};

#[path = "gateway_constraints/trace.rs"]
mod trace;
pub use self::trace::runtime_gateway_constraint_error_trace;
use self::trace::{
    ConstraintTraceInput, affinity_owner_unavailable_trace, constraint_trace, legacy_trace,
};

pub const RUNTIME_GATEWAY_CONSTRAINT_PLANNER_MAX_CANDIDATES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeGatewayConstraintCandidate {
    pub model: String,
    pub original_order: usize,
    pub selected: bool,
    pub evaluation: ProviderRequestConstraintEvaluation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeGatewayConstraintRoutePlan {
    pub requested_model: String,
    pub alias_chain: Vec<String>,
    pub concrete_candidates: Vec<RuntimeGatewayConstraintCandidate>,
    pub requirements: ProviderRequestRequirements,
    pub selected_model: Option<String>,
    #[serde(skip_serializing)]
    pub upstream_attempt_model: Option<String>,
    pub body_rewrite_required: bool,
    pub adjustment: Option<ProviderOutputAdjustment>,
    pub no_route_reason: Option<ProviderRequestConstraintDecision>,
    pub trace: RuntimeRouteDecisionTrace,
    pub truncated: bool,
    pub selection_pool_truncated: bool,
    pub omitted_candidates: usize,
}

pub struct RuntimeGatewayConstraintPlanInput<'a> {
    pub aliases: &'a [RuntimeGatewayRouteAlias],
    pub diagnostic_seed: u64,
    pub model_state: &'a BTreeMap<String, RuntimeGatewayRouteModelState>,
    pub policy: ProviderRequestConstraintPolicy,
    pub additional_features: &'a [ProviderRequestFeature],
    pub configured_reasoning_reserve_tokens: Option<u64>,
    pub hard_affinity_model: Option<&'a str>,
    pub hard_affinity_required: bool,
}

pub fn runtime_gateway_plan_route_with_constraints(
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    body: &[u8],
    input: RuntimeGatewayConstraintPlanInput<'_>,
) -> Result<RuntimeGatewayConstraintRoutePlan, ProviderRequestLimitError> {
    let hard_affinity = input.hard_affinity_required || input.hard_affinity_model.is_some();
    if !input.policy.enabled {
        let mut plan = legacy_route_plan(provider, endpoint, body, &input, hard_affinity);
        if input.hard_affinity_required && input.hard_affinity_model.is_none() {
            let reason = ProviderRequestConstraintDecision::AffinityOwnerUnavailable;
            plan.alias_chain = vec![plan.requested_model.clone()];
            plan.concrete_candidates.clear();
            plan.selected_model = None;
            plan.upstream_attempt_model = None;
            plan.body_rewrite_required = false;
            plan.adjustment = None;
            plan.no_route_reason = Some(reason);
            plan.trace = affinity_owner_unavailable_trace(endpoint, &plan.requested_model);
        }
        return Ok(plan);
    }
    let RuntimeGatewayConstraintPlanInput {
        aliases,
        diagnostic_seed,
        model_state,
        policy,
        additional_features,
        configured_reasoning_reserve_tokens,
        hard_affinity_model,
        hard_affinity_required,
    } = input;

    let value = serde_json::from_slice::<serde_json::Value>(body).map_err(|_| {
        ProviderRequestLimitError {
            kind: prodex_provider_core::ProviderRequestLimitErrorKind::InvalidJson,
            field: None,
        }
    })?;
    let requested_model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or_default()
        .to_string();
    let requirements = provider_request_requirements_from_value(
        &value,
        endpoint,
        &requested_model,
        additional_features,
    )?;
    if hard_affinity_required && hard_affinity_model.is_none() {
        let reason = ProviderRequestConstraintDecision::AffinityOwnerUnavailable;
        let trace = affinity_owner_unavailable_trace(endpoint, &requested_model);
        return Ok(RuntimeGatewayConstraintRoutePlan {
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
        });
    }
    let alias = aliases
        .iter()
        .find(|alias| alias.alias == requested_model && !alias.models.is_empty());
    let mut alias_chain = vec![requested_model.clone()];
    let mut alias_chain_truncated = false;
    let (mut concrete_models, selection_pool_omitted) = if let Some(model) = hard_affinity_model {
        concrete_models(provider, endpoint, std::iter::once(model))
    } else if let Some(alias) = alias {
        let visible_models =
            crate::RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES.saturating_sub(alias_chain.len());
        alias_chain_truncated = alias.models.len() > visible_models;
        alias_chain.extend(alias.models.iter().take(visible_models).cloned());
        concrete_models(provider, endpoint, alias.models.iter().map(String::as_str))
    } else {
        concrete_models(
            provider,
            endpoint,
            std::iter::once(requested_model.as_str()),
        )
    };
    if hard_affinity {
        concrete_models.truncate(1);
    }

    let mut candidates = concrete_models
        .into_iter()
        .enumerate()
        .map(|(original_order, model)| {
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
            let evaluation = evaluate_provider_request_constraints(
                provider,
                &model,
                &candidate_requirements,
                policy,
            );
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
        })
        .collect::<Vec<_>>();
    let combo_route = !hard_affinity
        && alias
            .map(|alias| alias.strategy == RuntimeGatewayRouteStrategy::Fallback)
            .unwrap_or(candidates.len() > 1);
    if combo_route
        && policy.oversized_output
            == prodex_provider_core::ProviderOversizedOutputPolicy::ClampWithNotice
    {
        normalize_combo_output_adjustment(&mut candidates);
    }
    let eligible_models = candidates
        .iter()
        .filter(|candidate| candidate.evaluation.eligible)
        .map(|candidate| candidate.model.clone())
        .collect::<Vec<_>>();

    let selected_model = if eligible_models.is_empty() {
        None
    } else if hard_affinity || eligible_models.len() == 1 {
        eligible_models.first().cloned()
    } else if let Some(alias) = alias {
        runtime_gateway_route_selected_model_from_models(
            alias,
            &eligible_models,
            diagnostic_seed,
            model_state,
            requirements.total_required_tokens,
        )
    } else {
        Some(format!("combo:{}", eligible_models.join(",")))
    };
    let selected_candidate_index = selected_model.as_deref().and_then(|selected| {
        let selected = selected
            .strip_prefix("combo:")
            .and_then(|models| models.split(',').next())
            .unwrap_or(selected);
        candidates
            .iter()
            .position(|candidate| candidate.model.eq_ignore_ascii_case(selected))
    });
    if let Some(index) = selected_candidate_index {
        candidates[index].selected = true;
    }
    let adjustment =
        selected_candidate_index.and_then(|index| candidates[index].evaluation.adjustment.clone());
    let upstream_attempt_model = selected_model
        .as_deref()
        .map(|model| exact_attempt_model(provider, model));
    let no_route_reason = selected_model.is_none().then(|| {
        candidates
            .first()
            .map(|candidate| candidate.evaluation.decision)
            .unwrap_or(ProviderRequestConstraintDecision::CatalogEntryUnavailable)
    });
    let mut trace = constraint_trace(ConstraintTraceInput {
        endpoint,
        provider,
        requested_model: &requested_model,
        candidates: &candidates,
        selected_index: selected_candidate_index,
        selected_model: selected_model.as_deref(),
        no_route_reason,
        hard_affinity,
        model_state,
    });
    if selection_pool_omitted > 0 {
        trace.truncation.truncated = true;
        trace.truncation.omitted_candidate_records = trace
            .truncation
            .omitted_candidate_records
            .saturating_add(selection_pool_omitted);
    }
    let (candidates, trace_omitted_candidates) =
        bounded_candidate_records(candidates, selected_candidate_index);
    let omitted_candidates = trace_omitted_candidates.saturating_add(selection_pool_omitted);
    Ok(RuntimeGatewayConstraintRoutePlan {
        requested_model,
        alias_chain,
        concrete_candidates: candidates,
        requirements,
        selected_model,
        upstream_attempt_model,
        body_rewrite_required: true,
        adjustment,
        no_route_reason,
        trace,
        truncated: omitted_candidates > 0 || alias_chain_truncated,
        selection_pool_truncated: selection_pool_omitted > 0,
        omitted_candidates,
    })
}

fn normalize_combo_output_adjustment(candidates: &mut [RuntimeGatewayConstraintCandidate]) {
    let Some(applied_tokens) = candidates
        .iter()
        .filter(|candidate| candidate.evaluation.eligible)
        .filter_map(|candidate| candidate.evaluation.adjustment.as_ref())
        .map(|adjustment| adjustment.applied_tokens)
        .min()
    else {
        return;
    };
    for candidate in candidates
        .iter_mut()
        .filter(|candidate| candidate.evaluation.eligible)
    {
        let Some(field) = candidate.evaluation.requirements.output_limit_field else {
            continue;
        };
        let requested_tokens = candidate
            .evaluation
            .adjustment
            .as_ref()
            .map(|adjustment| adjustment.requested_tokens)
            .or(candidate.evaluation.requirements.explicit_output_tokens)
            .unwrap_or(applied_tokens);
        candidate.evaluation.adjustment = Some(ProviderOutputAdjustment {
            field,
            requested_tokens,
            applied_tokens,
            reason: ProviderRequestConstraintDecision::OutputLimitClamped,
        });
        candidate.evaluation.decision = ProviderRequestConstraintDecision::OutputLimitClamped;
        candidate.evaluation.requirements.explicit_output_tokens = Some(applied_tokens);
        candidate.evaluation.requirements.total_required_tokens = candidate
            .evaluation
            .requirements
            .estimated_input_tokens
            .saturating_add(applied_tokens)
            .saturating_add(
                candidate
                    .evaluation
                    .requirements
                    .reasoning_reserve_tokens
                    .unwrap_or_default(),
            );
    }
}

fn concrete_models<'a>(
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    mut configured: impl ExactSizeIterator<Item = &'a str>,
) -> (Vec<String>, usize) {
    let mut seen = BTreeSet::new();
    let mut models = Vec::new();
    while let Some(configured_model) = configured.next() {
        let chain = if endpoint == ProviderEndpoint::Embeddings {
            vec![configured_model.to_string()]
        } else {
            provider_model_fallback_chain(provider, configured_model)
        };
        for model in if chain.is_empty() {
            vec![configured_model.to_string()]
        } else {
            chain
        } {
            if seen.insert(model.to_ascii_lowercase()) {
                if models.len() < RUNTIME_GATEWAY_CONSTRAINT_PLANNER_MAX_CANDIDATES {
                    models.push(model);
                } else {
                    return (models, 1);
                }
            }
        }
        if endpoint == ProviderEndpoint::Embeddings {
            break;
        }
        if models.len() == RUNTIME_GATEWAY_CONSTRAINT_PLANNER_MAX_CANDIDATES && configured.len() > 0
        {
            return (models, 1);
        }
    }
    (models, 0)
}

fn exact_attempt_model(provider: ProviderId, selected_model: &str) -> String {
    if selected_model.starts_with("combo:") {
        return selected_model.to_string();
    }
    if provider_model_fallback_chain(provider, selected_model).len() > 1 {
        format!("combo:{selected_model}")
    } else {
        selected_model.to_string()
    }
}

pub fn runtime_gateway_apply_constraint_plan_body(
    body: &[u8],
    plan: &RuntimeGatewayConstraintRoutePlan,
) -> Option<Vec<u8>> {
    if !plan.body_rewrite_required {
        return Some(body.to_vec());
    }
    let model = plan.upstream_attempt_model.as_deref()?;
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| rewrite_body_once(value, model, plan.adjustment.as_ref()))
}

fn bounded_candidate_records(
    candidates: Vec<RuntimeGatewayConstraintCandidate>,
    selected_index: Option<usize>,
) -> (Vec<RuntimeGatewayConstraintCandidate>, usize) {
    let limit = crate::RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES;
    let omitted = candidates.len().saturating_sub(limit);
    if omitted == 0 {
        return (candidates, 0);
    }
    let mut retained = candidates.iter().take(limit).cloned().collect::<Vec<_>>();
    if let Some(selected_index) = selected_index.filter(|index| *index >= limit) {
        retained[limit - 1] = candidates[selected_index].clone();
        retained.sort_by_key(|candidate| candidate.original_order);
    }
    (retained, omitted)
}

fn rewrite_body_once(
    mut value: serde_json::Value,
    model: &str,
    adjustment: Option<&ProviderOutputAdjustment>,
) -> Option<Vec<u8>> {
    let object = value.as_object_mut()?;
    if let Some(adjustment) = adjustment {
        object.insert(
            adjustment.field.as_str().to_string(),
            serde_json::Value::from(adjustment.applied_tokens),
        );
    }
    object.insert(
        "model".to_string(),
        serde_json::Value::String(model.to_string()),
    );
    serde_json::to_vec(&value).ok()
}

fn legacy_route_plan(
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    body: &[u8],
    input: &RuntimeGatewayConstraintPlanInput<'_>,
    hard_affinity: bool,
) -> RuntimeGatewayConstraintRoutePlan {
    let aliases = input.aliases;
    let diagnostic_seed = input.diagnostic_seed;
    let model_state = input.model_state;
    let additional_features = input.additional_features;
    let requested_model = runtime_gateway_request_model(body).unwrap_or_default();
    let alias = aliases
        .iter()
        .find(|alias| alias.alias == requested_model && !alias.models.is_empty());
    let alias_model = alias.and_then(|alias| {
        let models = alias
            .models
            .iter()
            .map(|model| model.trim())
            .filter(|model| !model.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        runtime_gateway_route_selected_model_from_models(
            alias,
            &models,
            diagnostic_seed,
            model_state,
            crate::runtime_gateway_estimated_tokens(body),
        )
    });
    let selected_model = alias_model
        .clone()
        .or_else(|| (!requested_model.is_empty()).then(|| requested_model.clone()));
    let requirements = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            provider_request_requirements_from_value(
                &value,
                endpoint,
                &requested_model,
                additional_features,
            )
            .ok()
        })
        .unwrap_or(ProviderRequestRequirements {
            endpoint,
            requested_model: requested_model.clone(),
            resolved_upstream_model: selected_model.clone(),
            estimated_input_tokens: estimate_request_input_tokens(body),
            explicit_output_tokens: None,
            output_limit_field: None,
            default_output_reserve_tokens: None,
            reasoning_effort: None,
            reasoning_reserve_tokens: None,
            total_required_tokens: estimate_request_input_tokens(body),
            required_features: additional_features.to_vec(),
        });
    let selected_candidate = selected_model
        .as_deref()
        .map(|model| {
            model
                .strip_prefix("combo:")
                .and_then(|models| models.split(',').next())
                .unwrap_or(model)
        })
        .unwrap_or_default()
        .to_string();
    let evaluation = evaluate_provider_request_constraints(
        provider,
        &selected_candidate,
        &requirements,
        ProviderRequestConstraintPolicy::default(),
    );
    let candidates = (!selected_candidate.is_empty())
        .then(|| RuntimeGatewayConstraintCandidate {
            model: selected_candidate.clone(),
            original_order: 0,
            selected: true,
            evaluation,
        })
        .into_iter()
        .collect::<Vec<_>>();
    let trace = legacy_trace(
        endpoint,
        provider,
        &requested_model,
        &candidates,
        selected_model.as_deref(),
        hard_affinity,
    );
    let body_rewrite_required = alias_model.is_some();
    RuntimeGatewayConstraintRoutePlan {
        requested_model: requested_model.clone(),
        alias_chain: alias_model
            .as_ref()
            .map(|model| vec![requested_model.clone(), model.clone()])
            .unwrap_or_else(|| vec![requested_model]),
        concrete_candidates: candidates,
        requirements,
        selected_model,
        upstream_attempt_model: alias_model,
        body_rewrite_required,
        adjustment: None,
        no_route_reason: None,
        trace,
        truncated: false,
        selection_pool_truncated: false,
        omitted_candidates: 0,
    }
}

#[cfg(test)]
#[path = "../tests/src/gateway_constraints.rs"]
mod tests;

use super::*;

pub(super) fn affinity_owner_unavailable_trace(
    endpoint: ProviderEndpoint,
    requested_model: &str,
) -> RuntimeRouteDecisionTrace {
    let mut builder = RuntimeRouteDecisionTraceBuilder::new(
        trace_route(endpoint),
        (!requested_model.is_empty()).then_some(requested_model),
    );
    builder.record_affinity(
        RuntimeRouteAffinityKind::Strict,
        None,
        true,
        RuntimeRouteAffinityOutcome::Exhausted,
    );
    for stage in [
        RuntimeRouteDecisionStage::ModelResolution,
        RuntimeRouteDecisionStage::EndpointCapability,
        RuntimeRouteDecisionStage::RequestConstraints,
    ] {
        builder.record_stage(stage, RuntimeRouteDecisionStageOutcome::Skipped);
    }
    builder.finish(
        RuntimeRouteDecisionTerminalOutcome::AffinityExhausted,
        Some(RuntimeRouteDecisionReason::from_label(
            ProviderRequestConstraintDecision::AffinityOwnerUnavailable.as_str(),
        )),
    )
}

pub fn runtime_gateway_constraint_error_trace(
    endpoint: ProviderEndpoint,
    body: &[u8],
    reason: ProviderRequestConstraintDecision,
) -> RuntimeRouteDecisionTrace {
    let requested_model = runtime_gateway_request_model(body);
    let mut builder =
        RuntimeRouteDecisionTraceBuilder::new(trace_route(endpoint), requested_model.as_deref());
    for stage in [
        RuntimeRouteDecisionStage::ModelResolution,
        RuntimeRouteDecisionStage::EndpointCapability,
    ] {
        builder.record_stage(stage, RuntimeRouteDecisionStageOutcome::Skipped);
    }
    builder.record_stage(
        RuntimeRouteDecisionStage::RequestConstraints,
        RuntimeRouteDecisionStageOutcome::Rejected,
    );
    builder.finish(
        RuntimeRouteDecisionTerminalOutcome::Failed,
        Some(RuntimeRouteDecisionReason::from_label(reason.as_str())),
    )
}

pub(super) struct ConstraintTraceInput<'a> {
    pub(super) endpoint: ProviderEndpoint,
    pub(super) provider: ProviderId,
    pub(super) requested_model: &'a str,
    pub(super) candidates: &'a [RuntimeGatewayConstraintCandidate],
    pub(super) selected_index: Option<usize>,
    pub(super) selected_model: Option<&'a str>,
    pub(super) no_route_reason: Option<ProviderRequestConstraintDecision>,
    pub(super) hard_affinity: bool,
    pub(super) model_state: &'a BTreeMap<String, RuntimeGatewayRouteModelState>,
}

pub(super) fn constraint_trace(input: ConstraintTraceInput<'_>) -> RuntimeRouteDecisionTrace {
    let ConstraintTraceInput {
        endpoint,
        provider,
        requested_model,
        candidates,
        selected_index,
        selected_model,
        no_route_reason,
        hard_affinity,
        model_state,
    } = input;
    let mut builder = RuntimeRouteDecisionTraceBuilder::new(
        trace_route(endpoint),
        (!requested_model.is_empty()).then_some(requested_model),
    );
    builder.record_stage(
        RuntimeRouteDecisionStage::ModelResolution,
        RuntimeRouteDecisionStageOutcome::Passed,
    );
    builder.record_affinity(
        if hard_affinity {
            RuntimeRouteAffinityKind::Strict
        } else {
            RuntimeRouteAffinityKind::None
        },
        hard_affinity
            .then(|| candidates.first().map(|candidate| candidate.model.as_str()))
            .flatten(),
        hard_affinity,
        if hard_affinity {
            if selected_index.is_some() {
                RuntimeRouteAffinityOutcome::Retained
            } else {
                RuntimeRouteAffinityOutcome::Exhausted
            }
        } else {
            RuntimeRouteAffinityOutcome::NotApplicable
        },
    );
    let candidate_order = selected_index
        .into_iter()
        .chain((0..candidates.len()).filter(|index| Some(*index) != selected_index));
    for index in candidate_order {
        let candidate = &candidates[index];
        let reason =
            (candidate.evaluation.decision != ProviderRequestConstraintDecision::Compatible).then(
                || RuntimeRouteDecisionReason::from_label(candidate.evaluation.decision.as_str()),
            );
        let mut input = RuntimeRouteCandidateDecisionInput::eligible(
            candidate.original_order,
            if hard_affinity {
                RuntimeRouteCandidateClass::Affinity
            } else {
                RuntimeRouteCandidateClass::Fallback
            },
        );
        input.provider = Some(provider.label().to_string());
        input.model = Some(candidate.model.clone());
        input.hard_affinity = hard_affinity;
        input.eligibility = if candidate.evaluation.eligible {
            RuntimeRouteCandidateEligibility::Eligible
        } else {
            RuntimeRouteCandidateEligibility::Rejected
        };
        input.rejection_stage = (!candidate.evaluation.eligible).then_some(
            if candidate.evaluation.decision
                == ProviderRequestConstraintDecision::EndpointUnsupported
            {
                RuntimeRouteDecisionStage::EndpointCapability
            } else {
                RuntimeRouteDecisionStage::RequestConstraints
            },
        );
        input.reason = reason;
        input.selected = candidate.selected;
        input.inflight_count = model_state
            .get(&candidate.model)
            .map(|state| state.in_flight);
        input.diagnostics = trace_diagnostics(&candidate.evaluation);
        builder.record_candidate(&candidate.model, input);
    }
    let endpoint_supported = candidates.iter().any(|candidate| {
        candidate.evaluation.decision != ProviderRequestConstraintDecision::EndpointUnsupported
    });
    builder.record_stage(
        RuntimeRouteDecisionStage::EndpointCapability,
        if endpoint_supported {
            RuntimeRouteDecisionStageOutcome::Passed
        } else {
            RuntimeRouteDecisionStageOutcome::Rejected
        },
    );
    if endpoint_supported {
        builder.record_stage(
            RuntimeRouteDecisionStage::RequestConstraints,
            if selected_index.is_some() {
                RuntimeRouteDecisionStageOutcome::Passed
            } else {
                RuntimeRouteDecisionStageOutcome::Rejected
            },
        );
    }
    if let Some(index) = selected_index {
        builder.mark_selected(&candidates[index].model);
        builder.set_resolved_model(selected_model);
    }
    builder.finish(
        if selected_index.is_some() {
            RuntimeRouteDecisionTerminalOutcome::Selected
        } else if hard_affinity {
            RuntimeRouteDecisionTerminalOutcome::AffinityExhausted
        } else {
            RuntimeRouteDecisionTerminalOutcome::NoCandidate
        },
        no_route_reason.map(|reason| RuntimeRouteDecisionReason::from_label(reason.as_str())),
    )
}

fn trace_diagnostics(
    evaluation: &ProviderRequestConstraintEvaluation,
) -> RuntimeRouteDecisionDiagnostics {
    let requirements = &evaluation.requirements;
    RuntimeRouteDecisionDiagnostics {
        estimated_input_tokens: Some(requirements.estimated_input_tokens),
        output_tokens: requirements
            .explicit_output_tokens
            .or(requirements.default_output_reserve_tokens),
        reasoning_reserve_tokens: requirements.reasoning_reserve_tokens,
        total_required_tokens: Some(requirements.total_required_tokens),
        available_context_tokens: evaluation.available_context_tokens,
        max_output_tokens: evaluation.max_output_tokens,
        requested_output_tokens: evaluation
            .adjustment
            .as_ref()
            .map(|adjustment| adjustment.requested_tokens),
        applied_output_tokens: evaluation
            .adjustment
            .as_ref()
            .map(|adjustment| adjustment.applied_tokens),
    }
}

fn trace_route(endpoint: ProviderEndpoint) -> RuntimeRouteDecisionRoute {
    match endpoint {
        ProviderEndpoint::Responses => RuntimeRouteDecisionRoute::Responses,
        ProviderEndpoint::ResponsesCompact => RuntimeRouteDecisionRoute::ResponsesCompact,
        ProviderEndpoint::ChatCompletions => RuntimeRouteDecisionRoute::ChatCompletions,
        ProviderEndpoint::Messages => RuntimeRouteDecisionRoute::Messages,
        ProviderEndpoint::Embeddings => RuntimeRouteDecisionRoute::Embeddings,
        _ => RuntimeRouteDecisionRoute::Standard,
    }
}

pub(super) fn legacy_trace(
    endpoint: ProviderEndpoint,
    provider: ProviderId,
    requested_model: &str,
    candidates: &[RuntimeGatewayConstraintCandidate],
    selected_model: Option<&str>,
    hard_affinity: bool,
) -> RuntimeRouteDecisionTrace {
    let mut builder = RuntimeRouteDecisionTraceBuilder::new(
        trace_route(endpoint),
        (!requested_model.is_empty()).then_some(requested_model),
    );
    builder.record_affinity(
        if hard_affinity {
            RuntimeRouteAffinityKind::Strict
        } else {
            RuntimeRouteAffinityKind::None
        },
        candidates.first().map(|candidate| candidate.model.as_str()),
        hard_affinity,
        if hard_affinity {
            RuntimeRouteAffinityOutcome::Retained
        } else {
            RuntimeRouteAffinityOutcome::NotApplicable
        },
    );
    builder.record_stage(
        RuntimeRouteDecisionStage::RequestConstraints,
        RuntimeRouteDecisionStageOutcome::Skipped,
    );
    if let Some(candidate) = candidates.first() {
        let mut input = RuntimeRouteCandidateDecisionInput::eligible(
            0,
            if hard_affinity {
                RuntimeRouteCandidateClass::Affinity
            } else {
                RuntimeRouteCandidateClass::Current
            },
        );
        input.provider = Some(provider.label().to_string());
        input.model = Some(candidate.model.clone());
        input.selected = true;
        input.hard_affinity = hard_affinity;
        builder.record_candidate(&candidate.model, input);
        builder.mark_selected(&candidate.model);
    }
    builder.set_resolved_model(selected_model);
    builder.finish(RuntimeRouteDecisionTerminalOutcome::Selected, None)
}

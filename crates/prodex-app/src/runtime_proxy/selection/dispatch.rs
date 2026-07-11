use anyhow::Result;

use super::{
    RuntimeAffinitySelectionDecision, RuntimeAffinitySelectionKind,
    RuntimeResponseCandidateSelection, RuntimeRotationProxyShared,
    next_runtime_previous_response_candidate_with_trace,
    next_runtime_response_candidate_for_route_with_prompt_cache_key,
    runtime_affinity_selection_decision,
    runtime_proxy_optimistic_current_candidate_for_route_with_selection,
    runtime_selection_trace_builder, runtime_selection_trace_log,
};

#[cfg(any(test, feature = "bench-support"))]
pub(crate) fn select_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    selection: RuntimeResponseCandidateSelection<'_>,
) -> Result<Option<String>> {
    select_runtime_response_candidate_for_route_with_request(shared, selection, None, None)
}

pub(crate) fn select_runtime_response_candidate_for_route_with_request(
    shared: &RuntimeRotationProxyShared,
    selection: RuntimeResponseCandidateSelection<'_>,
    request_id: Option<u64>,
    requested_model: Option<&str>,
) -> Result<Option<String>> {
    let mut trace = runtime_selection_trace_builder(selection.route_kind, requested_model);
    if requested_model.is_some() {
        trace.set_resolved_model(requested_model);
        trace.record_stage(
            runtime_proxy_crate::RuntimeRouteDecisionStage::ModelResolution,
            runtime_proxy_crate::RuntimeRouteDecisionStageOutcome::Passed,
        );
    }
    let mut affinity_exhausted = false;
    let result = select_runtime_response_candidate_for_route_inner(
        shared,
        selection,
        &mut trace,
        &mut affinity_exhausted,
    );
    let (outcome, reason) = match &result {
        Ok(Some(candidate_name)) => {
            trace.mark_selected(candidate_name);
            (
                runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::Selected,
                None,
            )
        }
        Ok(None) if affinity_exhausted => (
            runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::AffinityExhausted,
            None,
        ),
        Ok(None) => (
            runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::NoCandidate,
            None,
        ),
        Err(_) => (
            runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::Failed,
            Some(runtime_proxy_crate::RuntimeRouteDecisionReason::known(
                runtime_proxy_crate::RuntimeRouteDecisionReasonKind::SelectionFailed,
            )),
        ),
    };
    let trace = trace.finish(outcome, reason);
    runtime_selection_trace_log(shared, request_id, &trace);
    result
}

fn select_runtime_response_candidate_for_route_inner(
    shared: &RuntimeRotationProxyShared,
    selection: RuntimeResponseCandidateSelection<'_>,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    affinity_exhausted: &mut bool,
) -> Result<Option<String>> {
    for affinity_kind in [
        RuntimeAffinitySelectionKind::Strict,
        RuntimeAffinitySelectionKind::Pinned,
        RuntimeAffinitySelectionKind::TurnState,
    ] {
        match runtime_affinity_selection_decision(shared, selection, affinity_kind, trace)? {
            RuntimeAffinitySelectionDecision::Selected(profile_name) => {
                return Ok(Some(profile_name));
            }
            RuntimeAffinitySelectionDecision::Continue => {}
            RuntimeAffinitySelectionDecision::Exhausted => {
                *affinity_exhausted = true;
                return Ok(None);
            }
        }
    }

    if selection.discover_previous_response_owner {
        return next_runtime_previous_response_candidate_with_trace(
            shared,
            selection.excluded_profiles,
            selection.previous_response_id,
            selection.route_kind,
            trace,
        );
    }

    match runtime_affinity_selection_decision(
        shared,
        selection,
        RuntimeAffinitySelectionKind::Session,
        trace,
    )? {
        RuntimeAffinitySelectionDecision::Selected(profile_name) => return Ok(Some(profile_name)),
        RuntimeAffinitySelectionDecision::Continue => {}
        RuntimeAffinitySelectionDecision::Exhausted => {
            *affinity_exhausted = true;
            return Ok(None);
        }
    }

    if let Some(profile_name) = runtime_proxy_optimistic_current_candidate_for_route_with_selection(
        shared,
        selection.excluded_profiles,
        selection.route_kind,
        selection.prompt_cache_key,
        trace,
    )? {
        return Ok(Some(profile_name));
    }

    next_runtime_response_candidate_for_route_with_prompt_cache_key(
        shared,
        selection.excluded_profiles,
        selection.route_kind,
        selection.prompt_cache_key,
        trace,
    )
}

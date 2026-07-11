use super::{
    RuntimeAffinitySelectionKind, RuntimeQuotaPressureBand, RuntimeQuotaSummary,
    RuntimeResponsePlannedCandidate, RuntimeRotationProxyShared, RuntimeRouteKind,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};

pub(super) fn runtime_selection_trace_builder(
    route_kind: RuntimeRouteKind,
    requested_model: Option<&str>,
) -> runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder {
    runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder::new(
        prodex_runtime_quota::runtime_route_kind_to_proxy(route_kind).into(),
        requested_model,
    )
}

pub(super) fn runtime_selection_trace_reason(
    reason: &str,
) -> runtime_proxy_crate::RuntimeRouteDecisionReason {
    runtime_proxy_crate::RuntimeRouteDecisionReason::from_label(reason)
}

pub(super) fn runtime_selection_trace_quota_band(
    band: RuntimeQuotaPressureBand,
) -> runtime_proxy_crate::RuntimeRouteQuotaBand {
    match band {
        RuntimeQuotaPressureBand::Healthy => runtime_proxy_crate::RuntimeRouteQuotaBand::Healthy,
        RuntimeQuotaPressureBand::Thin => runtime_proxy_crate::RuntimeRouteQuotaBand::Thin,
        RuntimeQuotaPressureBand::Critical => runtime_proxy_crate::RuntimeRouteQuotaBand::Critical,
        RuntimeQuotaPressureBand::Exhausted => {
            runtime_proxy_crate::RuntimeRouteQuotaBand::Exhausted
        }
        RuntimeQuotaPressureBand::Unknown => runtime_proxy_crate::RuntimeRouteQuotaBand::Unknown,
    }
}

pub(super) fn runtime_selection_trace_candidate(
    original_order: usize,
    class: runtime_proxy_crate::RuntimeRouteCandidateClass,
    quota_summary: Option<RuntimeQuotaSummary>,
    inflight_count: Option<usize>,
    health_score: Option<u32>,
    circuit_state: Option<runtime_proxy_crate::RuntimeRouteCircuitState>,
) -> runtime_proxy_crate::RuntimeRouteCandidateDecisionInput {
    let mut candidate =
        runtime_proxy_crate::RuntimeRouteCandidateDecisionInput::eligible(original_order, class);
    candidate.quota_band =
        quota_summary.map(|summary| runtime_selection_trace_quota_band(summary.route_band));
    candidate.inflight_count = inflight_count;
    candidate.health_band = health_score.map(|score| {
        if score == 0 {
            runtime_proxy_crate::RuntimeRouteHealthBand::Healthy
        } else {
            runtime_proxy_crate::RuntimeRouteHealthBand::Penalized
        }
    });
    candidate.circuit_state = circuit_state;
    candidate
}

pub(super) fn runtime_selection_trace_planned_candidate(
    candidate: &RuntimeResponsePlannedCandidate,
    class: runtime_proxy_crate::RuntimeRouteCandidateClass,
) -> runtime_proxy_crate::RuntimeRouteCandidateDecisionInput {
    runtime_selection_trace_candidate(
        candidate.order_index,
        class,
        Some(candidate.quota_summary),
        Some(candidate.inflight_count),
        Some(candidate.health_sort_key),
        None,
    )
}

pub(super) fn runtime_selection_trace_reject(
    candidate: &mut runtime_proxy_crate::RuntimeRouteCandidateDecisionInput,
    reason: &str,
    stage: Option<runtime_proxy_crate::RuntimeRouteDecisionStage>,
) {
    let reason = runtime_selection_trace_reason(reason);
    candidate.eligibility = runtime_proxy_crate::RuntimeRouteCandidateEligibility::Rejected;
    candidate.rejection_stage = stage.or_else(|| reason.rejection_stage());
    candidate.reason = Some(reason);
    candidate.selected = false;
}

pub(super) fn runtime_selection_trace_log(
    shared: &RuntimeRotationProxyShared,
    request_id: Option<u64>,
    trace: &runtime_proxy_crate::RuntimeRouteDecisionTrace,
) {
    let mut fields = Vec::with_capacity(5);
    if let Some(request_id) = request_id {
        fields.push(runtime_proxy_log_field("request", request_id.to_string()));
    }
    fields.extend([
        runtime_proxy_log_field("schema_version", trace.schema_version.to_string()),
        runtime_proxy_log_field("route", trace.route.as_str()),
        runtime_proxy_log_field("outcome", trace.terminal_outcome.as_str()),
        runtime_proxy_log_field(
            "trace",
            serde_json::to_string(trace).unwrap_or_else(|_| "{}".to_string()),
        ),
    ]);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message("route_decision", fields),
    );
}

pub(crate) struct RuntimeSelectionTraceDirect<'a> {
    pub(crate) requested_model: Option<&'a str>,
    pub(crate) route_kind: RuntimeRouteKind,
    pub(crate) candidate_key: &'a str,
    pub(crate) class: runtime_proxy_crate::RuntimeRouteCandidateClass,
    pub(crate) affinity_kind: Option<runtime_proxy_crate::RuntimeRouteAffinityKind>,
    pub(crate) hard_affinity: bool,
}

pub(crate) fn runtime_selection_trace_log_direct(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    direct: RuntimeSelectionTraceDirect<'_>,
) {
    let mut trace = runtime_selection_trace_builder(direct.route_kind, direct.requested_model);
    if direct.requested_model.is_some() {
        trace.set_resolved_model(direct.requested_model);
        trace.record_stage(
            runtime_proxy_crate::RuntimeRouteDecisionStage::ModelResolution,
            runtime_proxy_crate::RuntimeRouteDecisionStageOutcome::Passed,
        );
    }
    let mut candidate = runtime_selection_trace_candidate(
        0,
        direct.class,
        None,
        None,
        None,
        Some(runtime_proxy_crate::RuntimeRouteCircuitState::Closed),
    );
    candidate.hard_affinity = direct.hard_affinity;
    candidate.selected = true;
    trace.record_candidate(direct.candidate_key, candidate);
    if let Some(kind) = direct.affinity_kind {
        trace.record_affinity(
            kind,
            Some(direct.candidate_key),
            direct.hard_affinity,
            runtime_proxy_crate::RuntimeRouteAffinityOutcome::Retained,
        );
    } else {
        trace.record_stage(
            runtime_proxy_crate::RuntimeRouteDecisionStage::Ranking,
            runtime_proxy_crate::RuntimeRouteDecisionStageOutcome::Passed,
        );
    }
    trace.mark_selected(direct.candidate_key);
    let trace = trace.finish(
        runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::Selected,
        None,
    );
    runtime_selection_trace_log(shared, Some(request_id), &trace);
}

pub(super) fn runtime_selection_trace_affinity_kind(
    kind: RuntimeAffinitySelectionKind,
) -> runtime_proxy_crate::RuntimeRouteAffinityKind {
    match kind {
        RuntimeAffinitySelectionKind::Strict => {
            runtime_proxy_crate::RuntimeRouteAffinityKind::Strict
        }
        RuntimeAffinitySelectionKind::Pinned => {
            runtime_proxy_crate::RuntimeRouteAffinityKind::PreviousResponse
        }
        RuntimeAffinitySelectionKind::TurnState => {
            runtime_proxy_crate::RuntimeRouteAffinityKind::TurnState
        }
        RuntimeAffinitySelectionKind::Session => {
            runtime_proxy_crate::RuntimeRouteAffinityKind::Session
        }
    }
}

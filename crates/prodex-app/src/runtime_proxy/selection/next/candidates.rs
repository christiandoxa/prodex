use super::super::*;

pub(super) fn record_runtime_response_candidates(
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    candidate_plan: &RuntimeResponseCandidateExecutionPlan,
) -> usize {
    let candidate_record_count = candidate_plan
        .ready_candidates
        .len()
        .saturating_add(candidate_plan.fallback_candidates.len());
    let ready_names = candidate_plan
        .ready_candidates
        .iter()
        .map(|candidate| candidate.name.as_str())
        .collect::<BTreeSet<_>>();
    for candidate in &candidate_plan.ready_candidates {
        let mut traced = runtime_selection_trace_planned_candidate(
            candidate,
            runtime_proxy_crate::RuntimeRouteCandidateClass::Ready,
        );
        traced.eligibility = runtime_proxy_crate::RuntimeRouteCandidateEligibility::Deferred;
        if let Some(reason) = candidate.ready_skip_reason() {
            runtime_selection_trace_reject(&mut traced, reason, None);
        }
        trace.record_candidate(&candidate.name, traced);
    }
    for candidate in &candidate_plan.fallback_candidates {
        if ready_names.contains(candidate.name.as_str()) {
            continue;
        }
        let mut traced = runtime_selection_trace_planned_candidate(
            candidate,
            runtime_proxy_crate::RuntimeRouteCandidateClass::Fallback,
        );
        traced.eligibility = runtime_proxy_crate::RuntimeRouteCandidateEligibility::Deferred;
        trace.record_candidate(&candidate.name, traced);
    }
    trace.record_stage(
        runtime_proxy_crate::RuntimeRouteDecisionStage::Ranking,
        runtime_proxy_crate::RuntimeRouteDecisionStageOutcome::Passed,
    );
    candidate_record_count
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeResponseCandidatePass {
    Ready,
    Fallback,
}

impl RuntimeResponseCandidatePass {
    fn class(self) -> runtime_proxy_crate::RuntimeRouteCandidateClass {
        match self {
            Self::Ready => runtime_proxy_crate::RuntimeRouteCandidateClass::Ready,
            Self::Fallback => runtime_proxy_crate::RuntimeRouteCandidateClass::Fallback,
        }
    }

    fn mode(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Fallback => "backoff",
        }
    }

    fn skip_reason(self, candidate: &RuntimeResponsePlannedCandidate) -> Option<&'static str> {
        match self {
            Self::Ready => candidate.ready_skip_reason(),
            Self::Fallback => candidate.fallback_skip_reason(),
        }
    }
}

pub(super) fn select_runtime_response_candidate(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    candidates: Vec<RuntimeResponsePlannedCandidate>,
    pass: RuntimeResponseCandidatePass,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> Result<Option<String>> {
    for candidate in candidates {
        if let Some(reason) = pass.skip_reason(&candidate) {
            let mut traced = runtime_selection_trace_planned_candidate(&candidate, pass.class());
            runtime_selection_trace_reject(&mut traced, reason, None);
            trace.record_candidate(&candidate.name, traced);
            log_runtime_response_candidate_skip(shared, route_kind, &candidate, pass, reason);
            continue;
        }
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            let reason =
                runtime_proxy_crate::RuntimeRouteDecisionReasonKind::RouteCircuitHalfOpenProbeWait
                    .as_str();
            let mut traced = runtime_selection_trace_planned_candidate(&candidate, pass.class());
            traced.circuit_state =
                Some(runtime_proxy_crate::RuntimeRouteCircuitState::HalfOpenWait);
            runtime_selection_trace_reject(&mut traced, reason, None);
            trace.record_candidate(&candidate.name, traced);
            log_runtime_response_candidate_skip(shared, route_kind, &candidate, pass, reason);
            continue;
        }
        let mut traced = runtime_selection_trace_planned_candidate(&candidate, pass.class());
        traced.selected = true;
        trace.record_candidate(&candidate.name, traced);
        log_runtime_response_candidate_pick(shared, route_kind, &candidate, pass);
        return Ok(Some(candidate.name));
    }
    Ok(None)
}

fn log_runtime_response_candidate_skip(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    candidate: &RuntimeResponsePlannedCandidate,
    pass: RuntimeResponseCandidatePass,
    reason: &'static str,
) {
    let mut fields = vec![
        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
        runtime_proxy_log_field("profile", candidate.name.as_str()),
        runtime_proxy_log_field("reason", reason),
        runtime_proxy_log_field("inflight", candidate.inflight_count.to_string()),
    ];
    if pass == RuntimeResponseCandidatePass::Ready && reason == "profile_inflight_soft_limit" {
        fields.push(runtime_proxy_log_field(
            "soft_limit",
            candidate.inflight_soft_limit.to_string(),
        ));
    }
    fields.extend([
        runtime_proxy_log_field("health", candidate.health_sort_key.to_string()),
        runtime_proxy_log_field(
            "quota_source",
            runtime_quota_source_label(candidate.quota_source),
        ),
    ]);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "selection_skip_current",
            runtime_selection_log_fields_with_quota(fields, candidate.quota_summary),
        ),
    );
}

fn log_runtime_response_candidate_pick(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    candidate: &RuntimeResponsePlannedCandidate,
    pass: RuntimeResponseCandidatePass,
) {
    let mut fields = vec![
        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
        runtime_proxy_log_field("profile", candidate.name.as_str()),
        runtime_proxy_log_field("mode", pass.mode()),
        runtime_proxy_log_field("inflight", candidate.inflight_count.to_string()),
        runtime_proxy_log_field("health", candidate.health_sort_key.to_string()),
    ];
    if pass == RuntimeResponseCandidatePass::Fallback {
        fields.push(runtime_proxy_log_field(
            "backoff",
            format!("{:?}", candidate.backoff_sort_key),
        ));
    }
    fields.extend([
        runtime_proxy_log_field("order", candidate.order_index.to_string()),
        runtime_proxy_log_field(
            "quota_source",
            runtime_quota_source_label(candidate.quota_source),
        ),
    ]);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "selection_pick",
            runtime_selection_log_fields_with_quota(fields, candidate.quota_summary),
        ),
    );
}

pub(super) fn select_runtime_auto_redeem_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    candidate_record_count: usize,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> Result<Option<String>> {
    if let Some(profile) =
        runtime_best_auto_redeem_profile_name(shared, route_kind, excluded_profiles)?
    {
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, &profile, route_kind)?;
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "selection_pick",
                runtime_selection_log_fields_with_quota(
                    [
                        runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                        runtime_proxy_log_field("profile", profile.as_str()),
                        runtime_proxy_log_field("mode", "auto_redeem"),
                        runtime_proxy_log_field(
                            "quota_source",
                            runtime_selection_quota_source_label(quota_source),
                        ),
                    ],
                    quota_summary,
                ),
            ),
        );
        let mut traced = runtime_selection_trace_candidate(
            candidate_record_count,
            runtime_proxy_crate::RuntimeRouteCandidateClass::AutoRedeem,
            Some(quota_summary),
            None,
            None,
            None,
        );
        traced.selected = true;
        trace.record_candidate(&profile, traced);
        return Ok(Some(profile));
    }
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "selection_pick",
            [
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("profile", "none"),
                runtime_proxy_log_field("mode", "exhausted"),
                runtime_proxy_log_field("excluded_count", excluded_profiles.len().to_string()),
            ],
        ),
    );
    Ok(None)
}

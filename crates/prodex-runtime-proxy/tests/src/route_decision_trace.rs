use super::*;

#[test]
fn compact_trace_keeps_route_outcome_and_selection() {
    let mut builder = RuntimeRouteDecisionTraceBuilder::new(
        RuntimeRouteDecisionRoute::Responses,
        Some("gpt-5.6"),
    );
    builder.set_resolved_model(Some("gpt-5.6"));
    let input = RuntimeRouteCandidateDecisionInput::eligible(0, RuntimeRouteCandidateClass::Ready);
    assert_eq!(
        builder.record_candidate("profile-main", input).as_deref(),
        Some("profile-main")
    );
    builder.mark_selected("profile-main");
    let trace = builder.finish(RuntimeRouteDecisionTerminalOutcome::Selected, None);

    assert_eq!(
        trace.schema_version,
        RUNTIME_ROUTE_DECISION_TRACE_SCHEMA_VERSION
    );
    assert_eq!(trace.route, RuntimeRouteDecisionRoute::Responses);
    assert_eq!(trace.requested_model.as_deref(), Some("gpt-5.6"));
    assert_eq!(trace.resolved_model.as_deref(), Some("gpt-5.6"));
    assert_eq!(trace.selected_candidate.as_deref(), Some("profile-main"));
    assert_eq!(
        trace.terminal_outcome,
        RuntimeRouteDecisionTerminalOutcome::Selected
    );

    let json = serde_json::to_string(&trace).expect("trace serializes");
    assert!(json.contains("\"selected_candidate\":\"profile-main\""));
    assert!(!json.contains("candidates"));
    assert!(!json.contains("diagnostics"));
}

#[test]
fn without_recording_preserves_selection_contract_without_payload() {
    let mut builder = RuntimeRouteDecisionTraceBuilder::without_recording(
        RuntimeRouteDecisionRoute::ResponsesCompact,
    );
    assert!(
        builder
            .record_candidate(
                "profile-main",
                RuntimeRouteCandidateDecisionInput::eligible(0, RuntimeRouteCandidateClass::Ready),
            )
            .is_none()
    );
    builder.mark_selected("profile-main");
    let trace = builder.finish(RuntimeRouteDecisionTerminalOutcome::NoCandidate, None);
    assert_eq!(trace.selected_candidate, None);
    assert_eq!(
        trace.terminal_outcome,
        RuntimeRouteDecisionTerminalOutcome::NoCandidate
    );
}

#[test]
fn trace_reason_preserves_known_and_sanitizes_unknown_labels() {
    let known = RuntimeRouteDecisionReason::from_label("quota_exhausted");
    assert_eq!(known.as_str(), "quota_exhausted");
    assert_eq!(
        known.rejection_stage(),
        Some(RuntimeRouteDecisionStage::Quota)
    );

    let unknown = RuntimeRouteDecisionReason::from_label("not safe / secret");
    assert_eq!(unknown.as_str(), "unknown");
    assert_eq!(unknown.rejection_stage(), None);
}

#[test]
fn route_reason_known_labels_keep_fixed_stage_contract() {
    use RuntimeRouteDecisionStage::{
        Admission, Affinity, Authentication, CircuitAndBackoff, EndpointCapability, FinalSelection,
        ModelResolution, Quota, Ranking, RequestConstraints,
    };

    let cases = [
        ("auth_failure_backoff", Authentication),
        ("selection_backoff", CircuitAndBackoff),
        ("route_circuit_open", CircuitAndBackoff),
        ("route_circuit_half_open_probe_wait", CircuitAndBackoff),
        ("profile_health", Ranking),
        ("profile_performance", Ranking),
        ("quota_probe_unavailable", Quota),
        ("stale_persisted_quota", Quota),
        ("quota_healthy", Quota),
        ("quota_thin", Quota),
        ("quota_critical", Quota),
        ("quota_exhausted", Quota),
        ("quota_unknown", Quota),
        ("quota_exhausted_before_send", Quota),
        ("quota_windows_unavailable", Quota),
        ("profile_inflight_soft_limit", Admission),
        ("auth_not_quota_compatible", Authentication),
        ("prompt_cache_affinity", Ranking),
        ("negative_cache", Affinity),
        ("excluded", Affinity),
        ("affinity_owner_unavailable", Affinity),
        ("selection_failed", FinalSelection),
        ("compatible", RequestConstraints),
        ("endpoint_unsupported", EndpointCapability),
        ("required_capability_missing", EndpointCapability),
        ("catalog_entry_unavailable", ModelResolution),
        ("context_window_unknown", RequestConstraints),
        ("context_window_exceeded", RequestConstraints),
        ("output_limit_unknown", RequestConstraints),
        ("requested_output_exceeds_model_limit", RequestConstraints),
        ("reasoning_reserve_unsupported", RequestConstraints),
        ("reasoning_reserve_excessive", RequestConstraints),
        ("malformed_request_limits", RequestConstraints),
        ("output_limit_clamped", RequestConstraints),
    ];

    for (label, stage) in cases {
        let reason = RuntimeRouteDecisionReason::from_label(label);
        assert_eq!(reason.as_str(), label, "label={label}");
        assert_eq!(reason.rejection_stage(), Some(stage), "label={label}");
    }

    let exact_miss = RuntimeRouteDecisionReason::from_label(" quota_exhausted ");
    assert_eq!(exact_miss.as_str(), "quota_exhausted");
    assert_eq!(exact_miss.rejection_stage(), None);
    assert_eq!(
        RuntimeRouteDecisionReason::from_label("UPPER").as_str(),
        "unknown"
    );
}

#[test]
fn safe_identifier_is_utf8_boundary_aware_and_bounded() {
    let long = "表".repeat(RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES);
    let (safe, truncated) = runtime_route_decision_safe_identifier(&long);
    assert!(truncated);
    assert!(safe.len() <= RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES);
    assert!(std::str::from_utf8(safe.as_bytes()).is_ok());
}

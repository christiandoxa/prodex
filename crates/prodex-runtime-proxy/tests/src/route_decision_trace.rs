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
fn safe_identifier_is_utf8_boundary_aware_and_bounded() {
    let long = "表".repeat(RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES);
    let (safe, truncated) = runtime_route_decision_safe_identifier(&long);
    assert!(truncated);
    assert!(safe.len() <= RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES);
    assert!(std::str::from_utf8(safe.as_bytes()).is_ok());
}

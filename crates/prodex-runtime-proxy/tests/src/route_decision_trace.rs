use super::*;

fn candidate(
    order: usize,
    class: RuntimeRouteCandidateClass,
) -> RuntimeRouteCandidateDecisionInput {
    RuntimeRouteCandidateDecisionInput::eligible(order, class)
}

#[test]
fn trace_orders_stages_and_candidates_deterministically() {
    let mut builder = RuntimeRouteDecisionTraceBuilder::new(
        RuntimeRouteDecisionRoute::Responses,
        Some("gpt-test"),
    );
    builder.record_stage(
        RuntimeRouteDecisionStage::Ranking,
        RuntimeRouteDecisionStageOutcome::Passed,
    );
    builder.record_stage(
        RuntimeRouteDecisionStage::Authentication,
        RuntimeRouteDecisionStageOutcome::Passed,
    );
    builder.record_candidate("profile-b", candidate(1, RuntimeRouteCandidateClass::Ready));
    builder.record_candidate("profile-a", candidate(0, RuntimeRouteCandidateClass::Ready));
    builder.mark_selected("profile-a");

    let trace = builder.finish(RuntimeRouteDecisionTerminalOutcome::Selected, None);

    assert_eq!(
        trace
            .stages
            .iter()
            .map(|summary| summary.stage)
            .collect::<Vec<_>>(),
        vec![
            RuntimeRouteDecisionStage::Authentication,
            RuntimeRouteDecisionStage::Ranking,
            RuntimeRouteDecisionStage::FinalSelection,
        ]
    );
    assert_eq!(
        trace
            .candidates
            .iter()
            .map(|candidate| candidate.original_order)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(trace.selected_candidate.as_deref(), Some("candidate-0002"));
    assert!(trace.candidates[0].selected);
}

#[test]
fn trace_records_selected_and_no_candidate_outcomes() {
    let mut selected =
        RuntimeRouteDecisionTraceBuilder::new(RuntimeRouteDecisionRoute::Responses, None);
    selected.record_candidate(
        "profile-a",
        candidate(0, RuntimeRouteCandidateClass::Current),
    );
    selected.mark_selected("profile-a");
    let selected = selected.finish(RuntimeRouteDecisionTerminalOutcome::Selected, None);
    assert_eq!(
        selected.terminal_outcome,
        RuntimeRouteDecisionTerminalOutcome::Selected
    );
    assert_eq!(
        selected.selected_candidate.as_deref(),
        Some("candidate-0001")
    );

    let empty = RuntimeRouteDecisionTraceBuilder::new(RuntimeRouteDecisionRoute::Responses, None)
        .finish(RuntimeRouteDecisionTerminalOutcome::NoCandidate, None);
    assert!(empty.candidates.is_empty());
    assert_eq!(
        empty.stages[0],
        RuntimeRouteDecisionStageSummary {
            stage: RuntimeRouteDecisionStage::FinalSelection,
            outcome: RuntimeRouteDecisionStageOutcome::Rejected,
        }
    );
}

#[test]
fn trace_reason_labels_cover_existing_selection_skips() {
    let labels = [
        (
            RuntimeRouteDecisionReasonKind::AuthFailureBackoff,
            "auth_failure_backoff",
        ),
        (
            RuntimeRouteDecisionReasonKind::SelectionBackoff,
            "selection_backoff",
        ),
        (
            RuntimeRouteDecisionReasonKind::RouteCircuitOpen,
            "route_circuit_open",
        ),
        (
            RuntimeRouteDecisionReasonKind::RouteCircuitHalfOpenProbeWait,
            "route_circuit_half_open_probe_wait",
        ),
        (
            RuntimeRouteDecisionReasonKind::ProfileHealth,
            "profile_health",
        ),
        (
            RuntimeRouteDecisionReasonKind::ProfilePerformance,
            "profile_performance",
        ),
        (
            RuntimeRouteDecisionReasonKind::QuotaProbeUnavailable,
            "quota_probe_unavailable",
        ),
        (
            RuntimeRouteDecisionReasonKind::StalePersistedQuota,
            "stale_persisted_quota",
        ),
        (
            RuntimeRouteDecisionReasonKind::QuotaHealthy,
            "quota_healthy",
        ),
        (RuntimeRouteDecisionReasonKind::QuotaThin, "quota_thin"),
        (
            RuntimeRouteDecisionReasonKind::QuotaCritical,
            "quota_critical",
        ),
        (
            RuntimeRouteDecisionReasonKind::QuotaExhausted,
            "quota_exhausted",
        ),
        (
            RuntimeRouteDecisionReasonKind::QuotaUnknown,
            "quota_unknown",
        ),
        (
            RuntimeRouteDecisionReasonKind::QuotaExhaustedBeforeSend,
            "quota_exhausted_before_send",
        ),
        (
            RuntimeRouteDecisionReasonKind::QuotaWindowsUnavailable,
            "quota_windows_unavailable",
        ),
        (
            RuntimeRouteDecisionReasonKind::ProfileInflightSoftLimit,
            "profile_inflight_soft_limit",
        ),
        (
            RuntimeRouteDecisionReasonKind::AuthNotQuotaCompatible,
            "auth_not_quota_compatible",
        ),
        (
            RuntimeRouteDecisionReasonKind::PromptCacheAffinity,
            "prompt_cache_affinity",
        ),
        (
            RuntimeRouteDecisionReasonKind::NegativeCache,
            "negative_cache",
        ),
        (RuntimeRouteDecisionReasonKind::Excluded, "excluded"),
        (
            RuntimeRouteDecisionReasonKind::AffinityOwnerUnavailable,
            "affinity_owner_unavailable",
        ),
        (
            RuntimeRouteDecisionReasonKind::SelectionFailed,
            "selection_failed",
        ),
    ];

    for (reason, label) in labels {
        assert_eq!(reason.as_str(), label);
        assert_eq!(
            RuntimeRouteDecisionReason::from_label(label),
            RuntimeRouteDecisionReason::Known(reason)
        );
    }
}

#[test]
fn hard_affinity_trace_retains_owner_despite_temporary_signals() {
    let mut builder =
        RuntimeRouteDecisionTraceBuilder::new(RuntimeRouteDecisionRoute::Responses, None);
    let mut owner = candidate(0, RuntimeRouteCandidateClass::Affinity);
    owner.hard_affinity = true;
    owner.health_band = Some(RuntimeRouteHealthBand::Penalized);
    owner.circuit_state = Some(RuntimeRouteCircuitState::Open);
    builder.record_candidate("owner", owner);
    builder.record_affinity(
        RuntimeRouteAffinityKind::PreviousResponse,
        Some("owner"),
        true,
        RuntimeRouteAffinityOutcome::Retained,
    );
    builder.mark_selected("owner");

    let trace = builder.finish(RuntimeRouteDecisionTerminalOutcome::Selected, None);
    assert!(trace.affinity.hard);
    assert_eq!(
        trace.affinity.outcome,
        RuntimeRouteAffinityOutcome::Retained
    );
    assert!(trace.candidates[0].selected);
    assert_eq!(trace.candidates[0].reason, None);
}

#[test]
fn fallback_candidate_classification_survives_selection() {
    let mut builder =
        RuntimeRouteDecisionTraceBuilder::new(RuntimeRouteDecisionRoute::Responses, None);
    let mut fallback = candidate(2, RuntimeRouteCandidateClass::Fallback);
    fallback.inflight_count = Some(3);
    fallback.quota_band = Some(RuntimeRouteQuotaBand::Thin);
    builder.record_candidate("fallback", fallback);
    builder.mark_selected("fallback");
    let trace = builder.finish(RuntimeRouteDecisionTerminalOutcome::Selected, None);

    assert_eq!(
        trace.candidates[0].class,
        RuntimeRouteCandidateClass::Fallback
    );
    assert_eq!(trace.candidates[0].inflight_count, Some(3));
}

#[test]
fn selected_candidate_keeps_non_rejection_reason_and_diagnostics() {
    let mut builder =
        RuntimeRouteDecisionTraceBuilder::new(RuntimeRouteDecisionRoute::Responses, None);
    let mut input = candidate(0, RuntimeRouteCandidateClass::Fallback);
    input.reason = Some(RuntimeRouteDecisionReason::from_label(
        "output_limit_clamped",
    ));
    input.diagnostics.requested_output_tokens = Some(100);
    input.diagnostics.applied_output_tokens = Some(80);
    builder.record_candidate("model", input);
    builder.mark_selected("model");

    let trace = builder.finish(RuntimeRouteDecisionTerminalOutcome::Selected, None);
    assert_eq!(
        trace.candidates[0]
            .reason
            .as_ref()
            .map(RuntimeRouteDecisionReason::as_str),
        Some("output_limit_clamped")
    );
    assert_eq!(
        trace.candidates[0].diagnostics.requested_output_tokens,
        Some(100)
    );
    assert_eq!(
        trace.candidates[0].diagnostics.applied_output_tokens,
        Some(80)
    );
}

#[test]
fn deferred_candidate_update_does_not_erase_prior_rejection() {
    let mut builder =
        RuntimeRouteDecisionTraceBuilder::new(RuntimeRouteDecisionRoute::Responses, None);
    let mut rejected = RuntimeRouteCandidateDecisionInput::rejected(
        0,
        RuntimeRouteCandidateClass::Current,
        RuntimeRouteDecisionReason::from_label("profile_health"),
    );
    rejected.rejection_stage = Some(RuntimeRouteDecisionStage::Ranking);
    rejected.circuit_state = Some(RuntimeRouteCircuitState::Open);
    builder.record_candidate("current", rejected);
    let mut deferred = candidate(0, RuntimeRouteCandidateClass::Current);
    deferred.eligibility = RuntimeRouteCandidateEligibility::Deferred;
    builder.record_candidate("current", deferred);
    let mut not_evaluated = candidate(1, RuntimeRouteCandidateClass::Fallback);
    not_evaluated.eligibility = RuntimeRouteCandidateEligibility::NotEvaluated;
    builder.record_candidate("current", not_evaluated);

    let trace = builder.finish(RuntimeRouteDecisionTerminalOutcome::NoCandidate, None);
    assert_eq!(
        trace.candidates[0].class,
        RuntimeRouteCandidateClass::Current
    );
    assert_eq!(
        trace.candidates[0].eligibility,
        RuntimeRouteCandidateEligibility::Rejected
    );
    assert_eq!(
        trace.candidates[0]
            .reason
            .as_ref()
            .map(RuntimeRouteDecisionReason::as_str),
        Some("profile_health")
    );
    assert_eq!(
        trace.candidates[0].circuit_state,
        Some(RuntimeRouteCircuitState::Open)
    );
}

#[test]
fn trace_truncates_candidate_records_and_identifiers() {
    let long_model = "m".repeat(RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES + 10);
    let mut builder = RuntimeRouteDecisionTraceBuilder::new(
        RuntimeRouteDecisionRoute::Responses,
        Some(&long_model),
    );
    for index in 0..=RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES {
        builder.record_candidate(
            &format!("profile-{index}"),
            candidate(index, RuntimeRouteCandidateClass::Ready),
        );
    }
    let trace = builder.finish(RuntimeRouteDecisionTerminalOutcome::NoCandidate, None);

    assert_eq!(
        trace.candidates.len(),
        RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES
    );
    assert!(trace.truncation.truncated);
    assert_eq!(trace.truncation.omitted_candidate_records, 1);
    assert_eq!(trace.truncation.truncated_identifiers, 1);
    assert_eq!(
        trace.requested_model.as_deref().map(str::len),
        Some(RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES)
    );
}

#[test]
fn trace_redacts_untrusted_identifiers_and_never_serializes_candidate_keys() {
    let mut builder = RuntimeRouteDecisionTraceBuilder::new(
        RuntimeRouteDecisionRoute::Responses,
        Some("prompt text with bearer sk-test-secret"),
    );
    let mut input = candidate(0, RuntimeRouteCandidateClass::Ready);
    input.provider = Some("/home/test-user/private".to_string());
    input.model = Some("sk-proj-secret-token".to_string());
    builder.record_candidate("raw-profile-person@example.com", input);
    let json = serde_json::to_string(
        &builder.finish(RuntimeRouteDecisionTerminalOutcome::NoCandidate, None),
    )
    .unwrap();

    for secret in [
        "prompt text",
        "sk-test-secret",
        "/home/test-user/private",
        "person@example.com",
        "sk-proj-secret-token",
        "raw-profile-person@example.com",
    ] {
        assert!(!json.contains(secret), "trace leaked {secret}: {json}");
    }
    assert!(json.contains("redacted"));
    assert!(json.contains("candidate-0001"));
}

#[test]
fn trace_serialization_is_stable_and_unknown_reasons_round_trip() {
    let mut builder = RuntimeRouteDecisionTraceBuilder::new(
        RuntimeRouteDecisionRoute::Embeddings,
        Some("embed-test"),
    );
    builder.record_candidate(
        "candidate",
        RuntimeRouteCandidateDecisionInput::rejected(
            0,
            RuntimeRouteCandidateClass::Ready,
            RuntimeRouteDecisionReason::from_label("future_constraint"),
        ),
    );
    let trace = builder.finish(
        RuntimeRouteDecisionTerminalOutcome::NoCandidate,
        Some(RuntimeRouteDecisionReason::from_label("future_terminal")),
    );
    let json = serde_json::to_string(&trace).unwrap();

    assert_eq!(
        json,
        r#"{"schema_version":1,"route":"embeddings","requested_model":"embed-test","resolved_model":null,"affinity":{"kind":"none","candidate_id":null,"hard":false,"outcome":"not_applicable"},"stages":[{"stage":"final_selection","outcome":"rejected"}],"candidates":[{"candidate_id":"candidate-0001","original_order":0,"provider":null,"model":null,"hard_affinity":false,"class":"ready","eligibility":"rejected","rejection_stage":null,"reason":"future_constraint","selected":false,"quota_band":null,"circuit_state":null,"health_band":null,"inflight_count":null,"diagnostics":{}}],"selected_candidate":null,"terminal_outcome":"no_candidate","terminal_reason":"future_terminal","commit_state":"pre_commit","truncation":{"truncated":false,"omitted_stages":0,"omitted_candidate_records":0,"truncated_identifiers":0}}"#
    );
    assert_eq!(
        serde_json::from_str::<RuntimeRouteDecisionTrace>(&json).unwrap(),
        trace
    );
}

#[test]
fn unknown_reason_labels_cannot_carry_arbitrary_data() {
    for value in [
        "Bearer secret-token",
        "/home/test-user/private",
        "person@example.com",
        "UPPER_CASE",
        &"x".repeat(RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES + 1),
    ] {
        let reason = RuntimeRouteDecisionReason::from_label(value);
        assert_eq!(reason.as_str(), "unknown");
        assert!(!serde_json::to_string(&reason).unwrap().contains(value));
    }
    assert_eq!(
        RuntimeRouteDecisionReason::from_label("future_constraint").as_str(),
        "future_constraint"
    );
}

#[test]
fn trace_distinguishes_precommit_and_committed_state() {
    let precommit =
        RuntimeRouteDecisionTraceBuilder::new(RuntimeRouteDecisionRoute::Responses, None)
            .finish(RuntimeRouteDecisionTerminalOutcome::NoCandidate, None);
    assert_eq!(
        precommit.commit_state,
        RuntimeRouteDecisionCommitState::PreCommit
    );

    let mut committed =
        RuntimeRouteDecisionTraceBuilder::new(RuntimeRouteDecisionRoute::Responses, None);
    committed.set_commit_state(RuntimeRouteDecisionCommitState::Committed);
    let committed = committed.finish(RuntimeRouteDecisionTerminalOutcome::Failed, None);
    assert_eq!(
        committed.commit_state,
        RuntimeRouteDecisionCommitState::Committed
    );
}

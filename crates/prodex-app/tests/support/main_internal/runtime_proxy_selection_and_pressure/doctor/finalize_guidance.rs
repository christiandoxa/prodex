use super::*;

#[test]
fn runtime_doctor_finalize_summary_prefers_session_replayable_blocked_fallback_diagnosis() {
    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    summary
        .marker_counts
        .insert("previous_response_fresh_fallback_blocked", 1);
    summary.marker_last_fields.insert(
        "previous_response_fresh_fallback_blocked",
        BTreeMap::from([
            (
                "reason".to_string(),
                "previous_response_not_found".to_string(),
            ),
            (
                "request_shape".to_string(),
                "session_replayable".to_string(),
            ),
        ]),
    );

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(
        summary.diagnosis,
        "Recent session-scoped previous_response_id continuation failed closed before commit. Fresh replay is disabled for stale continuation handling. Latest reason: previous_response_not_found. Next step: Inspect `previous_response_not_found` and `chain_dead_upstream_confirmed` for the owning context before retrying; fail-closed stale continuation handling blocks fresh replay when continuity is unverified. Start a fresh turn instead of forcing rotation if the owner cannot be recovered. Latest guard: previous_response_not_found."
    );
}

#[test]
fn runtime_doctor_finalize_summary_explains_continuation_only_blocked_fallback() {
    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    summary
        .marker_counts
        .insert("previous_response_fresh_fallback_blocked", 1);
    summary
        .previous_response_fresh_fallback_blocked_by_request_shape
        .insert("continuation_only".to_string(), 1);
    summary.marker_last_fields.insert(
        "previous_response_fresh_fallback_blocked",
        BTreeMap::from([
            (
                "reason".to_string(),
                "previous_response_not_found".to_string(),
            ),
            ("request_shape".to_string(), "continuation_only".to_string()),
        ]),
    );

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(
        summary.diagnosis,
        "Recent context-dependent previous_response_id continuation failed closed before commit. Fresh replay is disabled to preserve continuity. Latest reason: previous_response_not_found. Next step: Inspect `previous_response_not_found`, affinity bindings, and owning-profile chain markers before retrying; Prodex failed closed because this follow-up is context-dependent and cannot be replayed safely. Start a fresh turn only if context continuity can be abandoned. Latest guard: previous_response_not_found."
    );
}

#[test]
fn runtime_doctor_log_summary_surfaces_continuation_only_blocked_fallback() {
    let mut summary = summarize_runtime_log_tail(
        br#"[2026-04-21 10:00:00.000 +07:00] request=41 transport=http route=responses previous_response_not_found profile=beta response_id=resp-missing retry_index=0
[2026-04-21 10:00:00.001 +07:00] request=41 transport=http previous_response_fresh_fallback_blocked reason=previous_response_not_found request_shape=continuation_only outcome=blocked_nonreplayable_without_affinity profile=beta
"#,
    );
    summary.pointer_exists = true;
    summary.log_exists = true;
    runtime_doctor_finalize_summary(&mut summary);

    let fields = runtime_doctor_fields_for_summary(
        &summary,
        std::path::Path::new("/tmp/prodex-runtime-latest.path"),
    )
    .into_iter()
    .collect::<BTreeMap<_, _>>();

    assert_eq!(
        runtime_doctor_marker_count(&summary, "previous_response_fresh_fallback_blocked"),
        1
    );
    assert_eq!(
        summary
            .previous_response_fresh_fallback_blocked_by_request_shape
            .get("continuation_only")
            .copied(),
        Some(1)
    );
    assert!(
        summary
            .diagnosis
            .contains("context-dependent previous_response_id continuation"),
        "doctor diagnosis should explain continuation-only blocking: {}",
        summary.diagnosis
    );
    assert!(
        summary.diagnosis.contains("Fresh replay is disabled"),
        "doctor diagnosis should avoid misclassifying the guard: {}",
        summary.diagnosis
    );
    assert_eq!(
        fields.get("Continuation shape").map(String::as_str),
        Some("continuation_only")
    );
    assert_eq!(
        fields.get("Fail-closed shapes").map(String::as_str),
        Some("continuation_only=1")
    );
    assert!(
        fields
            .get("Continuation next step")
            .is_some_and(|value| value.contains("affinity bindings")),
        "doctor fields should point to affinity inspection: {fields:?}"
    );
}

#[test]
fn runtime_doctor_finalize_summary_surfaces_compact_exit_breakdown() {
    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    summary.marker_counts.insert("compact_final_failure", 1);
    summary.marker_last_fields.insert(
        "compact_final_failure",
        BTreeMap::from([
            ("exit".to_string(), "candidate_exhausted".to_string()),
            ("reason".to_string(), "quota".to_string()),
            ("profile".to_string(), "main".to_string()),
        ]),
    );

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(
        summary.diagnosis,
        "Recent compact final failure exited via candidate_exhausted with reason quota. Next step: Inspect compact budget and candidate-exhausted markers on profile main, then retry after compact quota refreshes or another profile becomes eligible."
    );
}

#[test]
fn runtime_doctor_finalize_summary_adds_lane_pressure_guidance() {
    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    summary
        .marker_counts
        .insert("runtime_proxy_lane_limit_reached", 1);
    summary.marker_last_fields.insert(
        "runtime_proxy_lane_limit_reached",
        BTreeMap::from([
            ("lane".to_string(), "compact".to_string()),
            ("active".to_string(), "4".to_string()),
            ("limit".to_string(), "4".to_string()),
        ]),
    );

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(
        summary.diagnosis,
        "Recent per-lane admission limit was triggered on compact. Next step: Inspect repeated lane=compact markers and trim bursty compact traffic if it is starving responses. Latest load: 4/4."
    );
}

#[test]
fn runtime_doctor_finalize_summary_adds_active_pressure_guidance() {
    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    summary
        .marker_counts
        .insert("runtime_proxy_active_limit_reached", 1);
    summary.marker_last_fields.insert(
        "runtime_proxy_active_limit_reached",
        BTreeMap::from([
            ("active".to_string(), "12".to_string()),
            ("limit".to_string(), "12".to_string()),
        ]),
    );

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(
        summary.diagnosis,
        "Recent global active-request admission limit was triggered. Next step: Reduce concurrent fresh work or wait for in-flight requests to drain before retrying. Latest load: 12/12."
    );
}

#[test]
fn runtime_doctor_finalize_summary_adds_profile_inflight_guidance() {
    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    summary
        .marker_counts
        .insert("profile_inflight_saturated", 1);
    summary.marker_last_fields.insert(
        "profile_inflight_saturated",
        BTreeMap::from([
            ("profile".to_string(), "main".to_string()),
            ("hard_limit".to_string(), "8".to_string()),
        ]),
    );

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(
        summary.diagnosis,
        "Recent per-profile in-flight saturation blocked main at hard limit 8. Next step: Wait for in-flight work on profile main to drop below hard limit 8 before retrying, or let fresh selection land on another eligible profile."
    );
}

#[test]
fn runtime_doctor_finalize_summary_adds_route_health_guidance() {
    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    summary.marker_counts.insert("profile_health", 1);
    summary.marker_last_fields.insert(
        "profile_health",
        BTreeMap::from([
            ("profile".to_string(), "main".to_string()),
            ("route".to_string(), "responses".to_string()),
            ("score".to_string(), "4".to_string()),
            ("reason".to_string(), "stream_read_error".to_string()),
        ]),
    );

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(
        summary.diagnosis,
        "Recent route-specific health penalty is steering fresh selection away from main/responses (score 4, reason stream_read_error). Next step: Inspect recent transport or overload markers for main/responses, especially `stream_read_error`, and wait for that route score to decay before expecting fresh selection to reuse it."
    );
}

#[test]
fn runtime_doctor_finalize_summary_adds_sync_probe_skip_guidance() {
    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    summary.marker_counts.insert("selection_skip_sync_probe", 1);
    summary.marker_last_fields.insert(
        "selection_skip_sync_probe",
        BTreeMap::from([
            ("route".to_string(), "responses".to_string()),
            ("reason".to_string(), "pressure_mode".to_string()),
            ("cold_start_profiles".to_string(), "2".to_string()),
        ]),
    );

    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(summary.selection_pressure, "elevated");
    assert_eq!(summary.quota_freshness_pressure, "stale_risk");
    assert_eq!(
        summary.diagnosis,
        "Recent fresh selection skipped inline quota probing on route responses under pressure mode. Next step: Inspect `selection_skip_sync_probe`, `profile_probe_refresh_backpressure`, and `profile_probe_refresh_queued` markers for route responses; pressure mode (pressure_mode) deferred 2 cold-start profile(s), so cold-start profiles may stay on stale quota data until background probes finish."
    );
}

#[test]
fn runtime_doctor_finalize_summary_adds_background_queue_guidance() {
    let mut summary = summarize_runtime_log_tail(
        br#"[2026-04-21 10:00:00.000 +07:00] state_save_queue_backpressure revision=2 reason=session_id:main backlog=7
[2026-04-21 10:00:00.001 +07:00] continuation_journal_queue_backpressure reason=session_id:main backlog=5
[2026-04-21 10:00:00.002 +07:00] profile_probe_refresh_backpressure profile=second backlog=4
"#,
    );
    summary.pointer_exists = true;
    summary.log_exists = true;
    runtime_doctor_finalize_summary(&mut summary);

    assert_eq!(summary.persistence_pressure, "elevated");
    assert_eq!(summary.quota_freshness_pressure, "stale_risk");
    assert_eq!(summary.state_save_queue_backlog, Some(7));
    assert_eq!(summary.continuation_journal_save_backlog, Some(5));
    assert_eq!(summary.profile_probe_refresh_backlog, Some(4));
    assert_eq!(
        summary.diagnosis,
        "Recent background persistence queue backpressure was detected. Next step: Reduce rapid rotation or continuation churn and wait for background persistence queues to drain. Latest backlog: state=7 journal=5. Latest reason: session_id:main."
    );
}

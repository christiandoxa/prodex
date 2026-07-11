use super::*;

#[test]
fn runtime_doctor_json_value_includes_selection_markers() {
    let mut summary = RuntimeDoctorSummary {
        line_count: 3,
        ..RuntimeDoctorSummary::default()
    };
    summary.marker_counts.insert("selection_pick", 2);
    summary.marker_counts.insert("selection_skip_current", 1);
    summary
        .marker_counts
        .insert("previous_response_not_found", 2);
    summary.marker_counts.insert("chain_retried_owner", 1);
    summary
        .marker_counts
        .insert("chain_dead_upstream_confirmed", 1);
    summary.marker_counts.insert("stale_continuation", 1);
    summary
        .marker_counts
        .insert("previous_response_fresh_fallback", 1);
    summary
        .marker_counts
        .insert("previous_response_fresh_fallback_blocked", 1);
    summary.marker_counts.insert("compact_followup_owner", 1);
    summary.marker_counts.insert("compact_committed", 1);
    summary
        .marker_counts
        .insert("compact_candidate_exhausted", 1);
    summary.marker_counts.insert("compact_final_failure", 1);
    summary
        .marker_counts
        .insert("compact_fresh_fallback_blocked", 1);
    summary.first_timestamp = Some("2026-03-25 00:00:00.000 +07:00".to_string());
    summary.last_timestamp = Some("2026-03-25 00:00:05.000 +07:00".to_string());
    summary.facet_counts.insert(
        "route".to_string(),
        BTreeMap::from([("responses".to_string(), 2)]),
    );
    summary.facet_counts.insert(
        "quota_source".to_string(),
        BTreeMap::from([("persisted_snapshot".to_string(), 1)]),
    );
    summary.facet_counts.insert(
        "request_shape".to_string(),
        BTreeMap::from([("session_replayable".to_string(), 2)]),
    );
    summary.previous_response_not_found_by_route =
        BTreeMap::from([("responses".to_string(), 1), ("websocket".to_string(), 1)]);
    summary.previous_response_not_found_by_transport =
        BTreeMap::from([("http".to_string(), 1), ("websocket".to_string(), 1)]);
    summary.chain_retried_owner_by_reason =
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)]);
    summary.chain_dead_upstream_confirmed_by_reason =
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)]);
    summary.stale_continuation_by_reason =
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)]);
    summary.latest_chain_event = Some(
        "chain_dead_upstream_confirmed reason=previous_response_not_found_locked_affinity profile=second"
            .to_string(),
    );
    summary.latest_stale_continuation_reason =
        Some("previous_response_not_found_locked_affinity".to_string());
    summary.latest_request_id = Some("42".to_string());
    summary.latest_request_timeline = vec![
        RuntimeDoctorRequestTimelineEvent {
            timestamp: Some("2026-03-25 00:00:01.000 +07:00".to_string()),
            phase: "selection".to_string(),
            marker: "selection_pick".to_string(),
            detail: "profile=second route=responses".to_string(),
        },
        RuntimeDoctorRequestTimelineEvent {
            timestamp: Some("2026-03-25 00:00:02.000 +07:00".to_string()),
            phase: "commit".to_string(),
            marker: "first_local_chunk".to_string(),
            detail: "profile=second route=responses".to_string(),
        },
    ];
    summary.marker_last_fields.insert(
        "selection_pick",
        BTreeMap::from([
            ("profile".to_string(), "second".to_string()),
            ("route".to_string(), "responses".to_string()),
            ("quota_source".to_string(), "persisted_snapshot".to_string()),
        ]),
    );
    summary.marker_last_fields.insert(
        "previous_response_fresh_fallback",
        BTreeMap::from([
            ("reason".to_string(), "quota_blocked".to_string()),
            (
                "request_shape".to_string(),
                "session_replayable".to_string(),
            ),
        ]),
    );
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
    summary.marker_last_fields.insert(
        "compact_candidate_exhausted",
        BTreeMap::from([("transport".to_string(), "http".to_string())]),
    );
    summary.marker_last_fields.insert(
        "compact_final_failure",
        BTreeMap::from([
            ("exit".to_string(), "candidate_exhausted".to_string()),
            ("reason".to_string(), "quota".to_string()),
            ("last_failure".to_string(), "quota".to_string()),
        ]),
    );
    summary.diagnosis = "Recent selection decisions were logged.".to_string();
    summary.persisted_verified_continuations = 2;
    summary.persisted_warm_continuations = 1;
    summary.persisted_suspect_continuations = 1;
    summary.persisted_continuation_journal_response_bindings = 3;
    summary.persisted_continuation_journal_session_bindings = 2;
    summary.persisted_continuation_journal_turn_state_bindings = 1;
    summary.persisted_continuation_journal_session_id_bindings = 4;
    summary.binding_state.active_profile = Some("main".to_string());
    summary.binding_state.profile_count = 2;
    summary.binding_state.last_run_selected_profiles = 2;
    summary
        .binding_state
        .runtime_continuations
        .response_bindings = 3;
    summary
        .binding_state
        .runtime_continuations
        .turn_state_bindings = 1;
    summary.binding_state.runtime_continuations.total_bindings = 4;
    summary.binding_state.runtime_continuations.profile_count = 2;
    summary.binding_state.merged_continuations.response_bindings = 4;
    summary.binding_state.merged_continuations.total_bindings = 5;
    summary.state_save_queue_backlog = Some(2);
    summary.state_save_lag_ms = Some(17);
    summary.continuation_journal_save_backlog = Some(1);
    summary.continuation_journal_save_lag_ms = Some(9);
    summary.profile_probe_refresh_backlog = Some(3);
    summary.profile_probe_refresh_lag_ms = Some(5);
    summary.continuation_journal_saved_at = Some(123);
    summary.suspect_continuation_bindings = vec!["turn-second:suspect".to_string()];
    summary.failure_class_counts = BTreeMap::from([
        ("admission".to_string(), 3),
        ("persistence".to_string(), 1),
        ("transport".to_string(), 2),
    ]);
    summary.recovered_continuation_journal_file = true;

    let value = runtime_doctor_json_value(&summary);
    assert_eq!(value["line_count"], 3);
    assert_eq!(value["first_timestamp"], "2026-03-25 00:00:00.000 +07:00");
    assert_eq!(value["last_timestamp"], "2026-03-25 00:00:05.000 +07:00");
    assert_eq!(value["marker_counts"]["selection_pick"], 2);
    assert_eq!(value["marker_counts"]["selection_skip_current"], 1);
    assert_eq!(value["marker_counts"]["previous_response_not_found"], 2);
    assert_eq!(value["marker_counts"]["chain_retried_owner"], 1);
    assert_eq!(value["marker_counts"]["chain_dead_upstream_confirmed"], 1);
    assert_eq!(value["marker_counts"]["stale_continuation"], 1);
    assert_eq!(
        value["marker_counts"]["previous_response_fresh_fallback"],
        1
    );
    assert_eq!(
        value["marker_counts"]["previous_response_fresh_fallback_blocked"],
        1
    );
    assert_eq!(value["marker_counts"]["compact_followup_owner"], 1);
    assert_eq!(value["marker_counts"]["compact_committed"], 1);
    assert_eq!(value["marker_counts"]["compact_candidate_exhausted"], 1);
    assert_eq!(value["marker_counts"]["compact_final_failure"], 1);
    assert_eq!(value["marker_counts"]["compact_fresh_fallback_blocked"], 1);
    assert_eq!(
        value["previous_response_not_found_by_route"]["responses"],
        1
    );
    assert_eq!(
        value["previous_response_not_found_by_route"]["websocket"],
        1
    );
    assert_eq!(value["previous_response_not_found_by_transport"]["http"], 1);
    assert_eq!(
        value["previous_response_not_found_by_transport"]["websocket"],
        1
    );
    assert_eq!(
        value["chain_retried_owner_by_reason"]["previous_response_not_found_locked_affinity"],
        1
    );
    assert_eq!(
        value["chain_dead_upstream_confirmed_by_reason"]["previous_response_not_found_locked_affinity"],
        1
    );
    assert_eq!(
        value["stale_continuation_by_reason"]["previous_response_not_found_locked_affinity"],
        1
    );
    assert_eq!(
        value["latest_chain_event"],
        "chain_dead_upstream_confirmed reason=previous_response_not_found_locked_affinity profile=second"
    );
    assert_eq!(
        value["latest_stale_continuation_reason"],
        "previous_response_not_found_locked_affinity"
    );
    assert_eq!(value["latest_request_id"], "42");
    assert_eq!(value["latest_request_timeline"][0]["phase"], "selection");
    assert_eq!(
        value["latest_request_timeline"][0]["marker"],
        "selection_pick"
    );
    assert_eq!(
        value["latest_request_timeline"][1]["marker"],
        "first_local_chunk"
    );
    assert_eq!(value["facet_counts"]["route"]["responses"], 2);
    assert_eq!(
        value["facet_counts"]["quota_source"]["persisted_snapshot"],
        1
    );
    assert_eq!(
        value["facet_counts"]["request_shape"]["session_replayable"],
        2
    );
    assert_eq!(
        value["marker_last_fields"]["selection_pick"]["profile"],
        "second"
    );
    assert_eq!(
        value["marker_last_fields"]["selection_pick"]["quota_source"],
        "persisted_snapshot"
    );
    assert_eq!(
        value["marker_last_fields"]["previous_response_fresh_fallback"]["request_shape"],
        "session_replayable"
    );
    assert_eq!(
        value["marker_last_fields"]["previous_response_fresh_fallback_blocked"]["reason"],
        "previous_response_not_found"
    );
    assert_eq!(
        value["marker_last_fields"]["compact_final_failure"]["exit"],
        "candidate_exhausted"
    );
    assert_eq!(value["persisted_verified_continuations"], 2);
    assert_eq!(value["persisted_warm_continuations"], 1);
    assert_eq!(value["persisted_suspect_continuations"], 1);
    assert_eq!(value["persisted_continuation_journal_response_bindings"], 3);
    assert_eq!(value["persisted_continuation_journal_session_bindings"], 2);
    assert_eq!(
        value["persisted_continuation_journal_turn_state_bindings"],
        1
    );
    assert_eq!(
        value["persisted_continuation_journal_session_id_bindings"],
        4
    );
    assert_eq!(value["binding_state"]["active_profile"], "main");
    assert_eq!(value["binding_state"]["profile_count"], 2);
    assert_eq!(
        value["binding_state"]["runtime_continuations"]["response_bindings"],
        3
    );
    assert_eq!(
        value["binding_state"]["runtime_continuations"]["turn_state_bindings"],
        1
    );
    assert_eq!(
        value["binding_state"]["merged_continuations"]["total_bindings"],
        5
    );
    assert_eq!(value["state_save_queue_backlog"], 2);
    assert_eq!(value["state_save_lag_ms"], 17);
    assert_eq!(value["continuation_journal_save_backlog"], 1);
    assert_eq!(value["continuation_journal_save_lag_ms"], 9);
    assert_eq!(value["profile_probe_refresh_backlog"], 3);
    assert_eq!(value["profile_probe_refresh_lag_ms"], 5);
    assert_eq!(value["continuation_journal_saved_at"], 123);
    assert_eq!(
        value["suspect_continuation_bindings"][0],
        "turn-second:suspect"
    );
    assert_eq!(value["failure_class_counts"]["admission"], 3);
    assert_eq!(value["failure_class_counts"]["persistence"], 1);
    assert_eq!(value["failure_class_counts"]["transport"], 2);
    summary.startup_audit_pressure = "elevated".to_string();
    summary.persisted_retry_backoffs = 2;
    summary.persisted_transport_backoffs = 1;
    summary.persisted_route_circuits = 3;
    summary.persisted_usage_snapshots = 4;
    summary.stale_persisted_usage_snapshots = 1;
    summary.recovered_state_file = true;
    summary.recovered_scores_file = false;
    summary.recovered_usage_snapshots_file = true;
    summary.recovered_backoffs_file = false;
    summary.last_good_backups_present = 3;
    summary.degraded_routes = vec!["main/responses circuit=open until=123".to_string()];
    summary.orphan_managed_dirs = vec!["ghost_profile".to_string()];
    summary.profiles = vec![RuntimeDoctorProfileSummary {
        profile: "main".to_string(),
        quota_freshness: "stale".to_string(),
        quota_age_seconds: 420,
        retry_backoff_until: Some(100),
        transport_backoff_until: Some(200),
        routes: vec![RuntimeDoctorRouteSummary {
            route: "responses".to_string(),
            circuit_state: "open".to_string(),
            circuit_until: Some(200),
            transport_backoff_until: Some(200),
            health_score: 4,
            bad_pairing_score: 2,
            performance_score: 3,
            quota_band: "quota_critical".to_string(),
            five_hour_status: "quota_ready".to_string(),
            weekly_status: "quota_critical".to_string(),
        }],
    }];

    let value = runtime_doctor_json_value(&summary);
    assert_eq!(
        value["diagnosis"],
        "Recent selection decisions were logged."
    );
    assert_eq!(value["startup_audit_pressure"], "elevated");
    assert_eq!(value["persisted_retry_backoffs"], 2);
    assert_eq!(value["persisted_route_circuits"], 3);
    assert_eq!(value["persisted_usage_snapshots"], 4);
    assert_eq!(value["stale_persisted_usage_snapshots"], 1);
    assert_eq!(value["recovered_state_file"], true);
    assert_eq!(value["recovered_usage_snapshots_file"], true);
    assert_eq!(value["recovered_continuation_journal_file"], true);
    assert_eq!(value["last_good_backups_present"], 3);
    assert_eq!(
        value["degraded_routes"][0],
        "main/responses circuit=open until=123"
    );
    assert_eq!(value["orphan_managed_dirs"][0], "ghost_profile");
    assert_eq!(value["profiles"][0]["profile"], "main");
    assert_eq!(value["profiles"][0]["quota_freshness"], "stale");
    assert_eq!(value["profiles"][0]["routes"][0]["route"], "responses");
    assert_eq!(value["profiles"][0]["routes"][0]["performance_score"], 3);
    assert_eq!(
        value["profiles"][0]["routes"][0]["transport_backoff_until"],
        200
    );
}

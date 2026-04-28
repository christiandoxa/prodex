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

#[test]
fn runtime_doctor_degraded_routes_sort_and_cap_output() {
    let now = Local::now().timestamp();
    let routes = runtime_doctor_degraded_routes(
        &RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([
                ("alpha".to_string(), now + 10),
                ("zeta".to_string(), now + 11),
            ]),
            transport_backoff_until: BTreeMap::from([
                (
                    runtime_profile_transport_backoff_key("beta", RuntimeRouteKind::Responses),
                    now + 20,
                ),
                ("gamma".to_string(), now + 21),
            ]),
            route_circuit_open_until: BTreeMap::from([
                ("__route_circuit__:responses:delta".to_string(), now - 1),
                ("__route_circuit__:websocket:eta".to_string(), now + 30),
                ("__route_circuit__:compact:theta".to_string(), now + 31),
            ]),
        },
        &BTreeMap::from([
            (
                "__route_bad_pairing__:standard:aardvark".to_string(),
                RuntimeProfileHealth {
                    score: 5,
                    updated_at: now,
                },
            ),
            (
                "__route_health__:compact:lambda".to_string(),
                RuntimeProfileHealth {
                    score: 1,
                    updated_at: now,
                },
            ),
            (
                "__route_bad_pairing__:responses:main".to_string(),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: now,
                },
            ),
            (
                "__route_health__:websocket:omega".to_string(),
                RuntimeProfileHealth {
                    score: 4,
                    updated_at: now,
                },
            ),
        ]),
        now,
    );

    assert_eq!(routes.len(), 8);
    assert_eq!(routes[0], "aardvark/standard bad_pairing=5");
    assert_eq!(
        routes[1],
        format!("alpha/retry retry_backoff until={}", now + 10)
    );
    assert_eq!(
        routes[3],
        format!("delta/responses circuit=half-open until={}", now - 1)
    );
    assert_eq!(
        routes[4],
        format!("eta/websocket circuit=open until={}", now + 30)
    );
    assert_eq!(
        routes[7], "main/responses bad_pairing=2",
        "helper should keep the first eight sorted entries"
    );
    assert!(
        !routes.iter().any(|route| route.starts_with("omega/")
            || route.starts_with("theta/")
            || route.starts_with("zeta/")),
        "later sorted entries should be truncated: {routes:?}"
    );
}

#[test]
fn runtime_doctor_fields_surface_queue_lag_and_failure_classes() {
    let summary = RuntimeDoctorSummary {
        log_path: Some(PathBuf::from("/tmp/prodex-runtime.log")),
        pointer_exists: true,
        log_exists: true,
        line_count: 8,
        state_save_queue_backlog: Some(4),
        state_save_lag_ms: Some(21),
        continuation_journal_save_backlog: Some(2),
        continuation_journal_save_lag_ms: Some(11),
        profile_probe_refresh_backlog: Some(6),
        profile_probe_refresh_lag_ms: Some(7),
        persisted_suspect_continuations: 2,
        suspect_continuation_bindings: vec![
            "resp-main:suspect".to_string(),
            "turn-main:suspect".to_string(),
        ],
        failure_class_counts: BTreeMap::from([
            ("admission".to_string(), 3),
            ("continuation".to_string(), 1),
            ("persistence".to_string(), 2),
            ("quota".to_string(), 2),
            ("transport".to_string(), 3),
        ]),
        marker_counts: BTreeMap::from([
            ("runtime_proxy_active_limit_reached", 1),
            ("runtime_proxy_lane_limit_reached", 1),
            ("profile_inflight_saturated", 1),
            ("previous_response_fresh_fallback", 2),
            ("previous_response_fresh_fallback_blocked", 1),
            ("compact_committed", 1),
            ("compact_candidate_exhausted", 2),
            ("compact_retryable_failure", 1),
            ("compact_final_failure", 1),
            ("profile_health", 1),
            ("selection_skip_sync_probe", 1),
            ("state_save_queue_backpressure", 1),
            ("continuation_journal_queue_backpressure", 1),
            ("profile_probe_refresh_backpressure", 1),
        ]),
        marker_last_fields: BTreeMap::from([
            (
                "runtime_proxy_active_limit_reached",
                BTreeMap::from([
                    ("active".to_string(), "12".to_string()),
                    ("limit".to_string(), "12".to_string()),
                ]),
            ),
            (
                "runtime_proxy_lane_limit_reached",
                BTreeMap::from([
                    ("lane".to_string(), "compact".to_string()),
                    ("active".to_string(), "4".to_string()),
                    ("limit".to_string(), "4".to_string()),
                ]),
            ),
            (
                "profile_inflight_saturated",
                BTreeMap::from([
                    ("profile".to_string(), "main".to_string()),
                    ("hard_limit".to_string(), "8".to_string()),
                ]),
            ),
            (
                "previous_response_fresh_fallback",
                BTreeMap::from([
                    ("reason".to_string(), "quota_blocked".to_string()),
                    ("request_shape".to_string(), "session_replayable".to_string()),
                ]),
            ),
            (
                "previous_response_fresh_fallback_blocked",
                BTreeMap::from([
                    (
                        "reason".to_string(),
                        "previous_response_not_found".to_string(),
                    ),
                    ("request_shape".to_string(), "session_replayable".to_string()),
                ]),
            ),
            (
                "compact_final_failure",
                BTreeMap::from([
                    ("exit".to_string(), "candidate_exhausted".to_string()),
                    ("reason".to_string(), "quota".to_string()),
                    ("last_failure".to_string(), "quota".to_string()),
                    ("profile".to_string(), "main".to_string()),
                ]),
            ),
            (
                "profile_health",
                BTreeMap::from([
                    ("profile".to_string(), "main".to_string()),
                    ("route".to_string(), "responses".to_string()),
                    ("score".to_string(), "4".to_string()),
                    ("reason".to_string(), "stream_read_error".to_string()),
                ]),
            ),
            (
                "selection_skip_sync_probe",
                BTreeMap::from([
                    ("route".to_string(), "responses".to_string()),
                    ("reason".to_string(), "pressure_mode".to_string()),
                    ("cold_start_jobs".to_string(), "3".to_string()),
                ]),
            ),
            (
                "state_save_queue_backpressure",
                BTreeMap::from([
                    ("reason".to_string(), "session_id:main".to_string()),
                    ("backlog".to_string(), "4".to_string()),
                ]),
            ),
            (
                "continuation_journal_queue_backpressure",
                BTreeMap::from([
                    ("reason".to_string(), "session_id:main".to_string()),
                    ("backlog".to_string(), "2".to_string()),
                ]),
            ),
            (
                "profile_probe_refresh_backpressure",
                BTreeMap::from([
                    ("profile".to_string(), "second".to_string()),
                    ("backlog".to_string(), "6".to_string()),
                ]),
            ),
        ]),
        chain_retried_owner_by_reason: BTreeMap::from([(
            "previous_response_not_found_locked_affinity".to_string(),
            1,
        )]),
        chain_dead_upstream_confirmed_by_reason: BTreeMap::from([(
            "previous_response_not_found_locked_affinity".to_string(),
            1,
        )]),
        stale_continuation_by_reason: BTreeMap::from([(
            "previous_response_not_found_locked_affinity".to_string(),
            1,
        )]),
        prodex_binary_identities: vec!["/usr/bin/prodex version=0.29.0 sha256=abc".to_string()],
        runtime_broker_identities: vec![
            "broker_key=broker pid=123 listen_addr=- status=binary_mismatch mismatch=version_mismatch version=0.26.0 path=/tmp/prodex sha256=def source=health stale_leases=0".to_string(),
        ],
        prodex_binary_mismatch: false,
        runtime_broker_mismatch: true,
        latest_chain_event: Some(
            "chain_dead_upstream_confirmed reason=previous_response_not_found_locked_affinity profile=second"
                .to_string(),
        ),
        latest_stale_continuation_reason: Some(
            "previous_response_not_found_locked_affinity".to_string(),
        ),
        diagnosis: "test diagnosis".to_string(),
        ..RuntimeDoctorSummary::default()
    };

    let fields = runtime_doctor_fields_for_summary(
        &summary,
        std::path::Path::new("/tmp/prodex-runtime-latest.path"),
    );
    let fields = fields.into_iter().collect::<BTreeMap<_, _>>();

    assert_eq!(
        fields.get("State save backlog").map(String::as_str),
        Some("4")
    );
    assert_eq!(fields.get("State save lag").map(String::as_str), Some("21"));
    assert_eq!(
        fields.get("Cont journal backlog").map(String::as_str),
        Some("2")
    );
    assert_eq!(
        fields.get("Cont journal lag").map(String::as_str),
        Some("11")
    );
    assert_eq!(fields.get("Probe backlog").map(String::as_str), Some("6"));
    assert_eq!(fields.get("Probe lag").map(String::as_str), Some("7"));
    assert_eq!(
        fields.get("Failure classes").map(String::as_str),
        Some("admission=3, continuation=1, persistence=2, quota=2, transport=3")
    );
    assert_eq!(
        fields.get("In-flight saturated").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        fields.get("In-flight profile").map(String::as_str),
        Some("main")
    );
    assert_eq!(
        fields.get("In-flight hard limit").map(String::as_str),
        Some("8")
    );
    assert_eq!(
        fields.get("In-flight next step").map(String::as_str),
        Some(
            "Wait for in-flight work on profile main to drop below hard limit 8 before retrying, or let fresh selection land on another eligible profile."
        )
    );
    assert_eq!(
        fields.get("Suspect continuations").map(String::as_str),
        Some("count=2 bindings=resp-main:suspect, turn-main:suspect")
    );
    assert_eq!(
        fields.get("Chain retry reasons").map(String::as_str),
        Some("previous_response_not_found_locked_affinity=1")
    );
    assert_eq!(
        fields.get("Legacy prev recovery").map(String::as_str),
        Some("2")
    );
    assert_eq!(
        fields.get("Prev fail-closed").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        fields.get("Sync-probe skips").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        fields.get("Sync-probe route").map(String::as_str),
        Some("responses")
    );
    assert_eq!(
        fields.get("Sync-probe deferred").map(String::as_str),
        Some("3 job(s)")
    );
    assert_eq!(
        fields.get("Sync-probe next step").map(String::as_str),
        Some(
            "Inspect `selection_skip_sync_probe`, `profile_probe_refresh_backpressure`, and `profile_probe_refresh_queued` markers for route responses; pressure mode (pressure_mode) deferred 3 cold-start job(s), so cold-start profiles may stay on stale quota data until background probes finish."
        )
    );
    assert_eq!(
        fields.get("Active next step").map(String::as_str),
        Some(
            "Reduce concurrent fresh work or wait for in-flight requests to drain before retrying. Latest load: 12/12."
        )
    );
    assert_eq!(
        fields.get("Lane next step").map(String::as_str),
        Some(
            "Inspect repeated lane=compact markers and trim bursty compact traffic if it is starving responses. Latest load: 4/4."
        )
    );
    assert_eq!(
        fields.get("Continuation next step").map(String::as_str),
        Some(
            "Inspect `previous_response_not_found` and `chain_dead_upstream_confirmed` for the owning context before retrying; fail-closed stale continuation handling blocks fresh replay when continuity is unverified. Start a fresh turn instead of forcing rotation if the owner cannot be recovered. Latest guard: previous_response_not_found."
        )
    );
    assert_eq!(
        fields.get("Compact committed").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        fields.get("Compact exhausted").map(String::as_str),
        Some("2")
    );
    assert_eq!(fields.get("Compact retry").map(String::as_str), Some("1"));
    assert_eq!(fields.get("Compact final").map(String::as_str), Some("1"));
    assert_eq!(
        fields.get("Compact exit").map(String::as_str),
        Some("candidate_exhausted")
    );
    assert_eq!(
        fields.get("Compact reason").map(String::as_str),
        Some("quota")
    );
    assert_eq!(
        fields.get("Compact next step").map(String::as_str),
        Some(
            "Inspect compact budget and candidate-exhausted markers on profile main, then retry after compact quota refreshes or another profile becomes eligible."
        )
    );
    assert_eq!(
        fields.get("Health route").map(String::as_str),
        Some("responses")
    );
    assert_eq!(
        fields.get("Health profile").map(String::as_str),
        Some("main")
    );
    assert_eq!(fields.get("Health score").map(String::as_str), Some("4"));
    assert_eq!(
        fields.get("Health reason").map(String::as_str),
        Some("stream_read_error")
    );
    assert_eq!(
        fields.get("Health next step").map(String::as_str),
        Some(
            "Inspect recent transport or overload markers for main/responses, especially `stream_read_error`, and wait for that route score to decay before expecting fresh selection to reuse it."
        )
    );
    assert_eq!(
        fields.get("Chain dead reasons").map(String::as_str),
        Some("previous_response_not_found_locked_affinity=1")
    );
    assert_eq!(
        fields.get("State save pressure").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        fields.get("State pressure reason").map(String::as_str),
        Some("session_id:main")
    );
    assert_eq!(
        fields.get("State pressure backlog").map(String::as_str),
        Some("4")
    );
    assert_eq!(
        fields.get("Persistence next step").map(String::as_str),
        Some(
            "Reduce rapid rotation or continuation churn and wait for background persistence queues to drain. Latest backlog: state=4 journal=2. Latest reason: session_id:main."
        )
    );
    assert_eq!(
        fields.get("Cont journal pressure").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        fields
            .get("Cont journal pressure backlog")
            .map(String::as_str),
        Some("2")
    );
    assert_eq!(
        fields.get("Probe refresh pressure").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        fields.get("Probe pressure profile").map(String::as_str),
        Some("second")
    );
    assert_eq!(
        fields.get("Probe pressure backlog").map(String::as_str),
        Some("6")
    );
    assert_eq!(
        fields.get("Probe next step").map(String::as_str),
        Some(
            "Let the background quota-refresh queue drain for profile second before expecting cold-start profiles to become selectable again. Latest probe backlog: 6."
        )
    );
    assert_eq!(
        fields.get("Stale reasons").map(String::as_str),
        Some("previous_response_not_found_locked_affinity=1")
    );
    assert_eq!(
        fields.get("Latest stale reason").map(String::as_str),
        Some("previous_response_not_found_locked_affinity")
    );
    assert_eq!(
        fields.get("Latest chain event").map(String::as_str),
        Some(
            "chain_dead_upstream_confirmed reason=previous_response_not_found_locked_affinity profile=second"
        )
    );
    assert_eq!(
        fields.get("Prodex binaries").map(String::as_str),
        Some("/usr/bin/prodex version=0.29.0 sha256=abc")
    );
    assert_eq!(
        fields.get("Runtime brokers").map(String::as_str),
        Some(
            "broker_key=broker pid=123 listen_addr=- status=binary_mismatch mismatch=version_mismatch version=0.26.0 path=/tmp/prodex sha256=def source=health stale_leases=0"
        )
    );
    assert_eq!(
        fields.get("Broker issues").map(String::as_str),
        Some("broker: pid 123 runs different prodex binary; restart active prodex/codex sessions")
    );
    assert_eq!(
        fields.get("Binary mismatch").map(String::as_str),
        Some("installed=false broker=true")
    );
}

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

#[test]
fn runtime_doctor_collect_state_flags_runtime_broker_binary_mismatch() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");

    let server = TinyServer::http("127.0.0.1:0").expect("health server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("health server should expose a TCP address");
    save_runtime_broker_registry(
        &paths,
        "doctor-mismatch",
        &RuntimeBrokerRegistry {
            pid: std::process::id(),
            listen_addr: listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            current_profile: "main".to_string(),
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        },
    )
    .expect("doctor mismatch registry should save");

    let health_thread = thread::spawn(move || {
        let request = server.recv().expect("health request should arrive");
        let body = serde_json::to_string(&RuntimeBrokerHealth {
            pid: std::process::id(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            active_requests: 0,
            instance_token: "instance".to_string(),
            persistence_role: "owner".to_string(),
            prodex_version: Some("0.26.0".to_string()),
            executable_path: Some("/tmp/prodex-0.26.0".to_string()),
            executable_sha256: None,
        })
        .expect("health payload should serialize");
        let response = TinyResponse::from_string(body).with_status_code(200);
        request
            .respond(response)
            .expect("health response should write");
    });

    let mut summary = RuntimeDoctorSummary::default();
    collect_runtime_doctor_state(&paths, &mut summary);

    health_thread
        .join()
        .expect("health server thread should join");

    assert!(
        summary.runtime_broker_mismatch,
        "doctor should flag mismatched live runtime broker identity"
    );
    assert!(
        summary.runtime_broker_identities.iter().any(|line| line
            .contains("broker_key=doctor-mismatch")
            && line.contains("status=binary_mismatch")
            && line.contains("mismatch=version_mismatch")
            && line.contains("version=0.26.0")
            && line.contains("source=health")),
        "doctor should surface the mismatched broker identity: {:?}",
        summary.runtime_broker_identities
    );
    summary.pointer_exists = true;
    summary.log_exists = true;
    summary.line_count = 1;
    runtime_doctor_finalize_summary(&mut summary);
    assert!(
        summary
            .diagnosis
            .contains("Runtime broker doctor-mismatch pid"),
        "doctor should explain how to resolve live broker mismatch: {}",
        summary.diagnosis
    );
}

#[test]
fn runtime_doctor_collect_state_surfaces_dead_broker_registry_and_stale_leases() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");

    save_runtime_broker_registry(
        &paths,
        "doctor-dead",
        &RuntimeBrokerRegistry {
            pid: 999_999,
            listen_addr: "127.0.0.1:9".to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            current_profile: "main".to_string(),
            instance_token: "dead-instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: Some("0.1.0".to_string()),
            executable_path: Some("/tmp/old-prodex".to_string()),
            executable_sha256: Some("deadbeef".to_string()),
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        },
    )
    .expect("dead broker registry should save");
    let lease_dir = runtime_broker_lease_dir(&paths, "doctor-dead");
    fs::create_dir_all(&lease_dir).expect("lease dir should exist");
    fs::write(lease_dir.join("stale.lease"), "pid=999998\n").expect("stale lease should write");

    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    collect_runtime_doctor_state(&paths, &mut summary);
    runtime_doctor_finalize_summary(&mut summary);

    assert!(
        summary
            .runtime_broker_identities
            .iter()
            .any(|line| line.contains("broker_key=doctor-dead")
                && line.contains("status=dead_pid")
                && line.contains("stale_leases=1")),
        "doctor should keep dead registry artifacts visible: {:?}",
        summary.runtime_broker_identities
    );
    assert!(
        summary
            .diagnosis
            .contains("run `prodex cleanup` or restart `prodex run`"),
        "doctor should point to cleanup/restart action for dead broker registry: {}",
        summary.diagnosis
    );
}

#[test]
fn runtime_doctor_collect_state_surfaces_unreachable_live_broker_health() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let _connect_timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS", "20");
    let _read_timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS", "20");
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");

    let server = TinyServer::http("127.0.0.1:0").expect("health timeout server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("health timeout server should expose a TCP address");
    save_runtime_broker_registry(
        &paths,
        "doctor-timeout",
        &RuntimeBrokerRegistry {
            pid: std::process::id(),
            listen_addr: listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            current_profile: "main".to_string(),
            instance_token: "timeout-instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: env::current_exe()
                .ok()
                .map(|path| path.display().to_string()),
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        },
    )
    .expect("timeout broker registry should save");

    let health_thread = thread::spawn(move || {
        let request = server.recv().expect("timeout health request should arrive");
        thread::sleep(Duration::from_millis(80));
        let _ = request.respond(TinyResponse::from_string("{}").with_status_code(200));
    });

    let mut summary = RuntimeDoctorSummary {
        pointer_exists: true,
        log_exists: true,
        line_count: 1,
        ..RuntimeDoctorSummary::default()
    };
    collect_runtime_doctor_state(&paths, &mut summary);
    runtime_doctor_finalize_summary(&mut summary);
    health_thread
        .join()
        .expect("timeout health thread should join");

    assert!(
        summary
            .runtime_broker_identities
            .iter()
            .any(|line| line.contains("broker_key=doctor-timeout")
                && line.contains("status=health_timeout")),
        "doctor should classify timed-out health probes: {:?}",
        summary.runtime_broker_identities
    );
    assert!(
        summary.diagnosis.contains("health probe timed out"),
        "doctor should surface timeout-specific action text: {}",
        summary.diagnosis
    );
}

#[test]
fn collect_orphan_managed_profile_dirs_ignores_tracked_and_fresh_dirs() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    let tracked = paths.managed_profiles_root.join("tracked");
    fs::create_dir_all(&tracked).expect("tracked dir should exist");
    fs::write(tracked.join("auth.json"), "{}").expect("tracked auth should be written");

    let orphan = paths.managed_profiles_root.join("orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan auth should be written");

    let fresh = paths.managed_profiles_root.join("fresh");
    fs::create_dir_all(&fresh).expect("fresh dir should exist");
    fs::write(fresh.join("auth.json"), "{}").expect("fresh auth should be written");

    let state = AppState {
        active_profile: Some("tracked".to_string()),
        profiles: BTreeMap::from([(
            "tracked".to_string(),
            ProfileEntry {
                codex_home: tracked,
                managed: true,
                email: Some("tracked@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let future_now = SystemTime::now()
        + Duration::from_secs(ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS as u64 + 5);
    assert_eq!(
        collect_orphan_managed_profile_dirs_at(&paths, &state, future_now),
        vec!["fresh".to_string(), "orphan".to_string()]
    );
}

#[test]
fn runtime_doctor_state_collects_persisted_degradation_and_orphans() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = paths.managed_profiles_root.join("main");
    fs::create_dir_all(&main_home).expect("main home should exist");
    fs::write(main_home.join("auth.json"), "{}").expect("main auth should be written");
    let orphan = paths.managed_profiles_root.join("orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan auth should be written");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");
    let usage_snapshots = BTreeMap::from([(
        "main".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: Local::now().timestamp(),
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 50,
            five_hour_reset_at: 123,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 50,
            weekly_reset_at: 456,
        },
    )]);
    let mut saved_usage_snapshots = false;
    for _ in 0..20 {
        match save_runtime_usage_snapshots(&paths, &usage_snapshots) {
            Ok(()) => {
                saved_usage_snapshots = true;
                break;
            }
            Err(err) => {
                if !err
                    .to_string()
                    .contains("failed to read /tmp/prodex-runtime-test")
                {
                    panic!("usage snapshots should save: {err:#}");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    assert!(saved_usage_snapshots, "usage snapshots should save");
    save_runtime_profile_scores(
        &paths,
        &BTreeMap::from([(
            "__route_bad_pairing__:responses:main".to_string(),
            RuntimeProfileHealth {
                score: 3,
                updated_at: Local::now().timestamp(),
            },
        )]),
    )
    .expect("scores should save");
    save_runtime_profile_backoffs(
        &paths,
        &RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([(
                "main".to_string(),
                Local::now().timestamp() + 30,
            )]),
            transport_backoff_until: BTreeMap::new(),
            route_circuit_open_until: BTreeMap::from([(
                "__route_circuit__:responses:main".to_string(),
                Local::now().timestamp() + 30,
            )]),
        },
    )
    .expect("backoffs should save");
    let journal_saved_at = Local::now().timestamp();
    let recent_not_found_at = journal_saved_at + RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS;
    save_runtime_continuation_journal_for_profiles(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: journal_saved_at,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Suspect,
                        confidence: 1,
                        last_touched_at: Some(journal_saved_at),
                        last_verified_at: None,
                        last_verified_route: None,
                        last_not_found_at: Some(recent_not_found_at),
                        not_found_streak: 1,
                        success_count: 0,
                        failure_count: 1,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        &state.profiles,
        journal_saved_at,
    )
    .expect("continuation journal should save");

    let _guard = TestEnvVarGuard::set("PRODEX_HOME", paths.root.to_str().unwrap());
    let mut summary = RuntimeDoctorSummary::default();
    collect_runtime_doctor_state(&paths, &mut summary);

    assert_eq!(summary.persisted_retry_backoffs, 1);
    assert_eq!(summary.persisted_route_circuits, 1);
    assert_eq!(summary.persisted_usage_snapshots, 1);
    assert_eq!(summary.persisted_continuation_journal_response_bindings, 1);
    assert_eq!(summary.persisted_suspect_continuations, 1);
    assert_eq!(summary.persisted_dead_continuations, 0);
    assert_eq!(
        summary.continuation_journal_saved_at,
        Some(journal_saved_at)
    );
    assert_eq!(
        summary.suspect_continuation_bindings,
        vec!["resp-main:suspect".to_string()]
    );
    assert!(
        summary
            .degraded_routes
            .iter()
            .any(|line| line.contains("main/responses"))
    );
}

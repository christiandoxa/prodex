use super::*;

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

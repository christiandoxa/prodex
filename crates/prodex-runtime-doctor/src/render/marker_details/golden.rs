use super::*;
use std::collections::BTreeMap;

fn summary(marker: &'static str) -> RuntimeDoctorSummary {
    let marker_fields = BTreeMap::from([
        ("backlog".to_string(), "7".to_string()),
        ("body_bytes".to_string(), "2048".to_string()),
        ("class".to_string(), "quota".to_string()),
        ("cold_start_jobs".to_string(), "2".to_string()),
        ("delay_ms".to_string(), "125".to_string()),
        ("error".to_string(), "boom".to_string()),
        ("excluded_count".to_string(), "4".to_string()),
        ("exit".to_string(), "candidate_exhausted".to_string()),
        ("fallback".to_string(), "1".to_string()),
        ("from_model".to_string(), "model-a".to_string()),
        ("hard_limit".to_string(), "8".to_string()),
        ("last_failure".to_string(), "quota".to_string()),
        ("model".to_string(), "model-a".to_string()),
        ("overflow_max_pending".to_string(), "3".to_string()),
        ("overflow_pending".to_string(), "2".to_string()),
        ("pressure_mode".to_string(), "normal".to_string()),
        ("profile".to_string(), "alpha".to_string()),
        ("provider".to_string(), "gemini".to_string()),
        ("ready".to_string(), "3".to_string()),
        ("reason".to_string(), "rate_limit".to_string()),
        ("request".to_string(), "42".to_string()),
        ("request_shape".to_string(), "continuation_only".to_string()),
        ("retry".to_string(), "2".to_string()),
        ("route".to_string(), "responses".to_string()),
        ("score".to_string(), "43".to_string()),
        ("source".to_string(), "refresh".to_string()),
        ("status".to_string(), "429".to_string()),
        ("sync_probe_jobs".to_string(), "1".to_string()),
        ("sync_probe_mode".to_string(), "deferred".to_string()),
        ("to_model".to_string(), "model-b".to_string()),
    ]);
    let mut summary = RuntimeDoctorSummary {
        marker_counts: BTreeMap::from([(marker, 1)]),
        marker_last_fields: BTreeMap::from([(marker, marker_fields)]),
        previous_response_not_found_by_route: BTreeMap::from([("responses".to_string(), 2)]),
        previous_response_not_found_by_transport: BTreeMap::from([("http".to_string(), 2)]),
        previous_response_fresh_fallback_blocked_by_request_shape: BTreeMap::from([(
            "continuation_only".to_string(),
            1,
        )]),
        chain_retried_owner_by_reason: BTreeMap::from([("retry".to_string(), 1)]),
        chain_dead_upstream_confirmed_by_reason: BTreeMap::from([("dead".to_string(), 1)]),
        stale_continuation_by_reason: BTreeMap::from([("stale".to_string(), 1)]),
        latest_stale_continuation_reason: Some("stale".to_string()),
        latest_chain_event: Some("chain_dead".to_string()),
        state_save_queue_backlog: Some(5),
        state_save_lag_ms: Some(10),
        continuation_journal_save_backlog: Some(6),
        continuation_journal_save_lag_ms: Some(11),
        profile_probe_refresh_backlog: Some(7),
        profile_probe_refresh_lag_ms: Some(12),
        startup_audit_pressure: "elevated".to_string(),
        compat_warning_count: 4,
        top_client_family: Some("codex".to_string()),
        top_client: Some("codex-cli".to_string()),
        top_tool_surface: Some("shell".to_string()),
        top_compat_warning: Some("warning".to_string()),
        facet_counts: BTreeMap::from([
            (
                "lane".to_string(),
                BTreeMap::from([("responses".to_string(), 2)]),
            ),
            (
                "route".to_string(),
                BTreeMap::from([("responses".to_string(), 2)]),
            ),
            (
                "profile".to_string(),
                BTreeMap::from([("alpha".to_string(), 2)]),
            ),
            (
                "reason".to_string(),
                BTreeMap::from([("rate_limit".to_string(), 2)]),
            ),
            (
                "quota_source".to_string(),
                BTreeMap::from([("live".to_string(), 2)]),
            ),
        ]),
        ..RuntimeDoctorSummary::default()
    };
    if marker == "runtime_proxy_overload_backoff" {
        summary.marker_counts.insert("upstream_connect_timeout", 2);
        summary.marker_counts.insert("upstream_connect_error", 3);
    }
    summary
}

fn rows(marker: &'static str) -> Vec<(String, String)> {
    let mut fields = FieldRowsBuilder::new();
    runtime_doctor_push_marker_detail_rows(&mut fields, &summary(marker), marker);
    fields.build()
}

fn expected(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(label, value)| ((*label).to_string(), (*value).to_string()))
        .collect()
}

fn assert_rows(marker: &'static str, golden: &[(&str, &str)]) {
    assert_eq!(rows(marker), expected(golden), "marker {marker}");
}

#[test]
fn pressure_marker_rows_are_golden() {
    assert_rows(
        "runtime_proxy_active_limit_reached",
        &[(
            "Active next step",
            "Reduce concurrent fresh work or wait for in-flight requests to drain before retrying.",
        )],
    );
    assert_rows(
        "runtime_proxy_lane_limit_reached",
        &[(
            "Lane next step",
            "Inspect repeated lane=unknown markers and trim bursty unknown traffic if it is starving responses.",
        )],
    );
    assert_rows(
        "profile_inflight_saturated",
        &[
            ("In-flight profile", "alpha"),
            ("In-flight hard limit", "8"),
            (
                "In-flight next step",
                "Wait for in-flight work on profile alpha to drop below hard limit 8 before retrying, or let fresh selection land on another eligible profile.",
            ),
        ],
    );
    assert_rows(
        "runtime_proxy_overload_backoff",
        &[("Connect failures", "5")],
    );
    assert_rows(
        "profile_health",
        &[
            ("Health route", "responses"),
            ("Health profile", "alpha"),
            ("Health score", "43"),
            ("Health reason", "rate_limit"),
            (
                "Health next step",
                "Inspect recent transport or overload markers for alpha/responses, especially `rate_limit`, and wait for that route score to decay before expecting fresh selection to reuse it.",
            ),
        ],
    );
}

#[test]
fn provider_and_gemini_marker_rows_are_golden() {
    assert_rows(
        "local_rewrite_provider_model_fallback",
        &[
            ("Provider fallback", "gemini model-a -> model-b"),
            ("Provider fallback class", "quota"),
        ],
    );
    assert_rows(
        "local_rewrite_provider_auth_failure",
        &[
            ("Provider auth scope", "gemini profile=alpha status=429"),
            ("Provider auth error", "quota"),
        ],
    );
    for marker in [
        "local_rewrite_gemini_quota_rotate",
        "local_rewrite_gemini_rate_limit_retry",
    ] {
        assert_rows(
            marker,
            &[(
                "Gemini retry scope",
                "profile=alpha status=429 reason=rate_limit retry=2 delay_ms=125",
            )],
        );
    }
    for marker in [
        "local_rewrite_gemini_invalid_stream_retry",
        "local_rewrite_gemini_invalid_stream_model_fallback",
    ] {
        assert_rows(
            marker,
            &[(
                "Gemini stream retry",
                "profile=alpha model=model-a from=model-a to=model-b reason=rate_limit",
            )],
        );
    }
    assert_rows(
        "local_rewrite_gemini_compact_semantic",
        &[(
            "Gemini compact",
            "mode=semantic profile=alpha request=42 body_bytes=2048 reason=rate_limit",
        )],
    );
    assert_rows(
        "local_rewrite_gemini_compact_fallback",
        &[(
            "Gemini compact",
            "mode=local-fallback profile=alpha request=42 body_bytes=2048 reason=rate_limit",
        )],
    );
    for marker in [
        "local_rewrite_gemini_live_error",
        "local_rewrite_gemini_live_sidecar_error",
        "local_rewrite_gemini_live_sidecar_session_error",
    ] {
        assert_rows(
            marker,
            &[("Gemini Live error", "request=42 profile=alpha error=boom")],
        );
    }
}

#[test]
fn websocket_and_auth_recovery_marker_rows_are_golden() {
    for (marker, next_step) in [
        (
            "websocket_connect_overflow_rejected",
            "Reduce concurrent websocket session starts or wait for websocket connect workers to drain before retrying. Latest reason: rate_limit; pending=2/3, workers=-, queue_capacity=-.",
        ),
        (
            "websocket_connect_overflow_reject",
            "Reduce concurrent websocket session starts or wait for websocket connect workers to drain before retrying. Latest reason: rate_limit; pending=2/3, workers=-, queue_capacity=-.",
        ),
        (
            "websocket_connect_overflow_enqueue",
            "Watch for matching dispatch or rejected markers; repeated enqueue means websocket connect workers are saturated. Latest reason: rate_limit; pending=2/3, workers=-, queue_capacity=-.",
        ),
        (
            "websocket_connect_overflow_dispatch",
            "Overflow queued websocket connect work drained back into the bounded workers; inspect earlier enqueue/reject markers if dispatch repeats. Latest reason: rate_limit; pending=2/3, workers=-, queue_capacity=-.",
        ),
    ] {
        assert_rows(
            marker,
            &[
                ("WS overflow reason", "rate_limit"),
                ("WS overflow pending", "2/3"),
                ("WS overflow next step", next_step),
            ],
        );
    }
    assert_rows(
        "profile_auth_recovery_failed",
        &[
            ("Auth recovery profile", "alpha"),
            ("Auth recovery route", "responses"),
            ("Auth recovery source", "refresh"),
            ("Auth recovery error", "boom"),
            (
                "Auth recovery next step",
                "Refresh credentials for profile alpha with `prodex login --profile alpha` and retry route responses; latest recovery error: boom.",
            ),
        ],
    );
    assert_rows(
        "profile_auth_recovered",
        &[
            ("Auth recovery profile", "alpha"),
            ("Auth recovery route", "responses"),
            ("Auth recovery source", "refresh"),
            ("Auth recovery error", "boom"),
            (
                "Auth recovery next step",
                "Auth recovered for profile alpha on route responses via refresh (changed=-); if this repeats, restart active sessions after login refresh.",
            ),
        ],
    );
}

#[test]
fn continuation_marker_rows_are_golden() {
    assert_rows(
        "previous_response_not_found",
        &[
            ("Prev not found routes", "responses=2"),
            ("Prev not found xport", "http=2"),
        ],
    );
    assert_rows(
        "previous_response_fresh_fallback",
        &[
            ("Legacy fallback shape", "continuation_only"),
            ("Legacy fallback reason", "rate_limit"),
            (
                "Legacy fallback note",
                "Current runtime should fail closed; restart active prodex/codex sessions if this marker came from a live broker.",
            ),
        ],
    );
    assert_rows(
        "previous_response_fresh_fallback_blocked",
        &[
            ("Continuation shape", "continuation_only"),
            ("Continuation reason", "rate_limit"),
            ("Fail-closed shapes", "continuation_only=1"),
            (
                "Continuation next step",
                "Inspect `previous_response_not_found`, affinity bindings, and owning-profile chain markers before retrying; Prodex failed closed because this follow-up is context-dependent and cannot be replayed safely. Start a fresh turn only if context continuity can be abandoned. Latest guard: rate_limit.",
            ),
        ],
    );
    assert_rows(
        "stale_continuation",
        &[
            ("Chain retry reasons", "retry=1"),
            ("Chain dead reasons", "dead=1"),
            ("Stale reasons", "stale=1"),
            ("Latest stale reason", "stale"),
            ("Latest chain event", "chain_dead"),
        ],
    );
    assert_rows(
        "compact_final_failure",
        &[
            ("Compact exit", "candidate_exhausted"),
            ("Compact reason", "rate_limit"),
            ("Compact last fail", "quota"),
            (
                "Compact next step",
                "Inspect compact exit markers around `candidate_exhausted` on profile alpha and retry after the blocking condition clears.",
            ),
        ],
    );
}

#[test]
fn persistence_and_selection_marker_rows_are_golden() {
    assert_rows(
        "local_writer_error",
        &[
            ("State save backlog", "5"),
            ("State save lag", "10"),
            ("Cont journal backlog", "6"),
            ("Cont journal lag", "11"),
            ("Probe backlog", "7"),
            ("Probe lag", "12"),
        ],
    );
    assert_rows(
        "state_save_queue_backpressure",
        &[
            ("State pressure reason", "rate_limit"),
            ("State pressure backlog", "7"),
            (
                "Persistence next step",
                "Reduce rapid rotation or continuation churn and wait for background persistence queues to drain. Latest backlog: state=5 journal=6. Latest reason: rate_limit.",
            ),
        ],
    );
    assert_rows(
        "continuation_journal_queue_backpressure",
        &[
            ("Cont journal pressure reason", "rate_limit"),
            ("Cont journal pressure backlog", "7"),
        ],
    );
    assert_rows(
        "profile_probe_refresh_backpressure",
        &[
            ("Probe pressure profile", "alpha"),
            ("Probe pressure backlog", "7"),
            (
                "Probe next step",
                "Let the background quota-refresh queue drain for profile alpha before expecting cold-start profiles to become selectable again. Latest probe backlog: 7.",
            ),
        ],
    );
    assert_rows(
        "runtime_proxy_startup_audit",
        &[("Startup pressure", "elevated")],
    );
    assert_rows(
        "selection_skip_sync_probe",
        &[
            ("Sync-probe route", "responses"),
            ("Sync-probe reason", "rate_limit"),
            ("Sync-probe deferred", "2 job(s)"),
            (
                "Sync-probe next step",
                "Inspect `selection_skip_sync_probe`, `profile_probe_refresh_backpressure`, and `profile_probe_refresh_queued` markers for route responses; pressure mode (rate_limit) deferred 2 cold-start job(s), so cold-start profiles may stay on stale quota data until background probes finish.",
            ),
        ],
    );
    assert_rows(
        "selection_plan",
        &[
            ("Selection route", "responses"),
            ("Ready candidates", "3"),
            ("Fallback candidates", "1"),
            ("Excluded profiles", "4"),
            ("Cold-start jobs", "2"),
            ("Sync-probe jobs", "1"),
            ("Sync-probe mode", "deferred"),
            ("Pressure mode", "normal"),
        ],
    );
    assert_rows(
        "compat_request_surface",
        &[
            ("Compat warnings", "4"),
            ("Client family", "codex"),
            ("Top client", "codex-cli"),
            ("Tool surface", "shell"),
            ("Compat warning", "warning"),
            ("Hot lane", "responses (2)"),
            ("Hot route", "responses (2)"),
            ("Hot profile", "alpha (2)"),
            ("Hot reason", "rate_limit (2)"),
            ("Quota source", "live (2)"),
        ],
    );
    assert!(rows("unknown_marker").is_empty());
}

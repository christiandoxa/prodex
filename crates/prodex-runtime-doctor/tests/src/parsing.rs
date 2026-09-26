use super::*;

// Fixed expected IDs keep this caller test independent of the Mojo classifier.
const EXPECTED_RUNTIME_DOCTOR_MARKERS: &[&str] = &[
    "chain_retried_owner",
    "chain_dead_upstream_confirmed",
    "stale_continuation",
    "runtime_proxy_queue_overloaded",
    "runtime_proxy_active_limit_reached",
    "runtime_proxy_lane_limit_reached",
    "runtime_proxy_overload_backoff",
    "runtime_proxy_admission_wait_started",
    "runtime_proxy_admission_wait_exhausted",
    "runtime_proxy_admission_recovered",
    "runtime_proxy_queue_wait_started",
    "runtime_proxy_queue_wait_exhausted",
    "runtime_proxy_queue_recovered",
    "profile_inflight_saturated",
    "profile_inflight",
    "upstream_connect_timeout",
    "upstream_connect_dns_error",
    "upstream_tls_handshake_error",
    "upstream_connect_error",
    "upstream_connect_http",
    "upstream_close_before_completed",
    "upstream_connection_closed",
    "upstream_overload_passthrough",
    "upstream_overloaded",
    "upstream_read_error",
    "upstream_send_error",
    "upstream_stream_error",
    "precommit_budget_exhausted",
    "profile_retry_backoff",
    "profile_transport_backoff",
    "profile_transport_failure",
    "profile_circuit_open",
    "profile_circuit_half_open_probe",
    "profile_health",
    "profile_latency",
    "profile_bad_pairing",
    "profile_quota_quarantine",
    "profile_auth_backoff",
    "profile_auth_backoff_cleared",
    "profile_auth_proactive_sync",
    "profile_auth_proactive_sync_failed",
    "previous_response_not_found",
    "previous_response_negative_cache",
    "previous_response_fresh_fallback",
    "previous_response_fresh_fallback_blocked",
    "previous_response_binding_cleared",
    "previous_response_owner",
    "previous_response_release_affinity",
    "previous_response_release_deferred",
    "previous_response_turn_state_rehydrated",
    "compact_committed_owner",
    "compact_followup_owner",
    "compact_fresh_fallback_blocked",
    "compact_pressure_shed",
    "compact_lineage_released",
    "compact_committed",
    "compact_precommit_budget_exhausted",
    "compact_candidate_exhausted",
    "compact_retryable_failure",
    "compact_transport_failure",
    "compact_overload_conservative_retry",
    "compact_quota_unclassified",
    "compact_pre_send_allow_quota_exhausted",
    "compact_final_failure",
    "compact_exit_committed",
    "compact_exit_committed_owner",
    "compact_exit_followup_owner",
    "compact_exit_fresh_fallback_blocked",
    "compact_exit_pressure_shed",
    "compact_exit_lineage_released",
    "compact_exit_precommit_budget_exhausted",
    "compact_exit_candidate_exhausted",
    "compact_exit_retryable_failure",
    "compact_exit_overload_conservative_retry",
    "compact_exit_quota_unclassified",
    "selection_keep_affinity",
    "selection_keep_current",
    "selection_plan",
    "selection_pick",
    "selection_skip_current",
    "selection_skip_affinity",
    "local_selection_blocked",
    "responses_pre_send_skip",
    "websocket_pre_send_skip",
    "quota_release_profile_affinity",
    "quota_release_affinity",
    "quota_blocked",
    "quota_critical_floor_before_send",
    "upstream_usage_limit_passthrough",
    "local_rewrite_upstream_start",
    "local_rewrite_upstream_response",
    "local_rewrite_request_detail",
    "local_rewrite_web_search_options_fallback",
    "local_rewrite_provider_model_fallback",
    "local_rewrite_provider_auth_failure",
    "local_rewrite_gemini_builtin_tool_fallback",
    "local_rewrite_gemini_quota_rotate",
    "local_rewrite_gemini_rate_limit_retry",
    "local_rewrite_gemini_invalid_stream_retry",
    "local_rewrite_gemini_invalid_stream_model_fallback",
    "local_rewrite_gemini_quota_status_ready",
    "local_rewrite_gemini_quota_status_unavailable",
    "local_rewrite_gemini_compact_semantic",
    "local_rewrite_gemini_compact_fallback",
    "local_rewrite_gemini_synthetic_thought_signature",
    "local_rewrite_gemini_live_sidecar_started",
    "local_rewrite_gemini_live_sidecar_error",
    "local_rewrite_gemini_live_sidecar_accept_error",
    "local_rewrite_gemini_live_connected",
    "local_rewrite_gemini_live_error",
    "local_rewrite_gemini_live_sidecar_connected",
    "local_rewrite_gemini_live_sidecar_session_error",
    "local_rewrite_gemini_live_frame",
    "local_rewrite_gemini_live_duplex_pump",
    "compat_request_surface",
    "compat_warning",
    "smart_context_autopilot",
    "runtime_proxy_sync_probe_pressure_pause",
    "websocket_reuse_skip_quota_exhausted",
    "websocket_reuse_watchdog",
    "websocket_reuse_watchdog_timeout",
    "websocket_reuse_locked_affinity_owner_fresh_retry",
    "websocket_reuse_nonreplayable_fresh_retry",
    "websocket_reuse_owner_fresh_retry",
    "websocket_reuse_previous_response_blocked",
    "websocket_reuse_stale_previous_response_blocked",
    "websocket_precommit_frame_timeout",
    "websocket_precommit_hold_timeout",
    "websocket_dns_resolve_timeout",
    "websocket_dns_overflow_enqueue",
    "websocket_dns_overflow_dispatch",
    "websocket_dns_overflow_reject",
    "websocket_connect_local_pressure",
    "websocket_connect_overflow_enqueue",
    "websocket_connect_overflow_dispatch",
    "websocket_connect_overflow_reject",
    "websocket_connect_overflow_rejected",
    "websocket_proxy_connect_start",
    "websocket_proxy_tunnel_ok",
    "websocket_proxy_tunnel_failure",
    "profile_auth_recovered",
    "profile_auth_recovery_failed",
    "stream_read_error",
    "token_usage",
    "local_writer_error",
    "first_upstream_chunk",
    "first_local_chunk",
    "state_save_ok",
    "state_save_skipped",
    "state_save_error",
    "state_save_queued",
    "state_save_queue_backpressure",
    "continuation_journal_save_ok",
    "continuation_journal_save_error",
    "continuation_journal_save_queued",
    "continuation_journal_queue_backpressure",
    "runtime_proxy_restore_counts",
    "runtime_proxy_startup_audit",
    "runtime_proxy_upstream_proxy_mode",
    "profile_probe_refresh_queued",
    "profile_probe_refresh_start",
    "profile_probe_refresh_ok",
    "profile_probe_refresh_error",
    "profile_probe_refresh_backpressure",
    "profile_probe_refresh_panic",
    "selection_skip_sync_probe",
    "quota_blocked_affinity_released",
];

#[test]
fn runtime_doctor_parse_message_fields_match_fixed_values() {
    let fields = runtime_doctor_parse_message_fields(
        r#"selection_pick request=7 transport=http profile="alpha beta" note="say \"yes\"" city=東京 empty="" malformed="unterminated"#,
    );

    let expected = [
        ("city", "東京"),
        ("empty", ""),
        ("malformed", "unterminated"),
        ("note", "say \"yes\""),
        ("profile", "alpha beta"),
        ("request", "7"),
        ("transport", "http"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(fields, expected);
}

#[test]
fn runtime_doctor_prefers_json_event_and_fields() {
    let log = br#"{"timestamp":"2026-05-12T00:00:00Z","message":"runtime_proxy_queue_overloaded lane=responses active=1","event":"runtime_proxy_lane_limit_reached","fields":{"lane":"compact","active":6,"overflow":false}}"#;

    let summary = summarize_runtime_log_tail(log);

    assert_eq!(
        summary
            .marker_counts
            .get("runtime_proxy_lane_limit_reached")
            .copied(),
        Some(1)
    );
    assert_eq!(
        summary
            .marker_counts
            .get("runtime_proxy_queue_overloaded")
            .copied(),
        None
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("runtime_proxy_lane_limit_reached")
            .and_then(|fields| fields.get("lane"))
            .map(String::as_str),
        Some("compact")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("runtime_proxy_lane_limit_reached")
            .and_then(|fields| fields.get("active"))
            .map(String::as_str),
        Some("6")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("runtime_proxy_lane_limit_reached")
            .and_then(|fields| fields.get("overflow"))
            .map(String::as_str),
        Some("false")
    );
}

#[test]
fn runtime_doctor_finds_known_marker_after_unclassified_message_prefix() {
    let line = RuntimeDoctorParsedLogLine::new(
        "[2026-05-12 00:00:00Z] notice request_id=req-1 selection_pick profile=alpha",
    );

    assert_eq!(line.marker_name().as_deref(), Some("selection_pick"));
    assert_eq!(
        line.fields(),
        std::collections::BTreeMap::from([
            ("profile".to_string(), "alpha".to_string()),
            ("request_id".to_string(), "req-1".to_string()),
        ])
    );
}

#[test]
fn runtime_doctor_uses_message_event_when_json_event_is_unknown() {
    let log = br#"{"event":"unknown_marker","message":"selection_pick profile=alpha"}"#;

    let summary = summarize_runtime_log_tail(log);

    assert_eq!(
        summary.marker_counts.get("selection_pick").copied(),
        Some(1)
    );
    assert_eq!(summary.marker_counts.get("unknown_marker").copied(), None);
}

#[test]
fn runtime_doctor_redacts_secret_fields_and_terminal_controls() {
    let json_line = r#"{"timestamp":"2026-05-12T00:00:00Z","event":"runtime_proxy_lane_limit_reached","fields":{"route":"/v1/responses\u001b[31m","authorization":"Bearer fixture-secret-sentinel"}}"#;
    let text_line = format!(
        "[2026-05-12 00:00:01.000 +00:00] stream_read_error route=\"/v1/responses{}[31m\" authorization=\"Bearer fixture-secret-sentinel\"",
        '\u{1b}'
    );
    let log = format!("{json_line}\n{text_line}");

    let summary = summarize_runtime_log_tail(log.as_bytes());

    let json_fields = summary
        .marker_last_fields
        .get("runtime_proxy_lane_limit_reached")
        .unwrap();
    assert_eq!(json_fields["authorization"], "<redacted>");
    assert!(
        !json_fields["route"]
            .chars()
            .any(|character| character.is_control() || character == '\u{7f}')
    );
    let text_fields = summary.marker_last_fields.get("stream_read_error").unwrap();
    assert_eq!(text_fields["authorization"], "<redacted>");
    assert!(
        !text_fields["route"]
            .chars()
            .any(|character| character.is_control() || character == '\u{7f}')
    );
    let rendered = runtime_doctor_json_value(&summary).to_string();
    assert!(!rendered.contains("fixture-secret-sentinel"), "{rendered}");
    assert!(
        !rendered
            .chars()
            .any(|character| character.is_control() || character == '\u{7f}')
    );
}

#[test]
fn runtime_doctor_falls_back_to_typed_text_parser() {
    let log = br#"[2026-05-12 00:00:00.000 +00:00] stream_read_error request=7 transport=http error="failed with spaces""#;

    let summary = summarize_runtime_log_tail(log);

    assert_eq!(
        summary.marker_counts.get("stream_read_error").copied(),
        Some(1)
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("stream_read_error")
            .and_then(|fields| fields.get("error"))
            .map(String::as_str),
        Some("failed with spaces")
    );
}

#[test]
fn runtime_doctor_summarizes_marker_context_by_route_lane_and_profile() {
    let log = br#"[2026-05-12 00:00:00.000 +00:00] runtime_proxy_lane_limit_reached lane=compact route=/responses/compact profile=alpha active=4
[2026-05-12 00:00:01.000 +00:00] runtime_proxy_lane_limit_reached lane=compact route=/responses/compact profile=beta active=5
[2026-05-12 00:00:02.000 +00:00] profile_inflight_saturated route=responses profile=alpha active=8
"#;

    let summary = summarize_runtime_log_tail(log);

    let lane_limit = summary
        .marker_context_summary
        .iter()
        .find(|entry| entry.marker == "runtime_proxy_lane_limit_reached")
        .expect("lane limit marker context should be summarized");
    assert_eq!(lane_limit.total, 2);
    assert_eq!(lane_limit.lanes.get("compact").copied(), Some(2));
    assert_eq!(
        lane_limit.routes.get("/responses/compact").copied(),
        Some(2)
    );
    assert_eq!(lane_limit.profiles.get("alpha").copied(), Some(1));
    assert_eq!(lane_limit.profiles.get("beta").copied(), Some(1));

    let inflight = summary
        .marker_context_summary
        .iter()
        .find(|entry| entry.marker == "profile_inflight_saturated")
        .expect("profile inflight marker context should be summarized");
    assert_eq!(inflight.total, 1);
    assert_eq!(inflight.routes.get("responses").copied(), Some(1));
    assert_eq!(inflight.profiles.get("alpha").copied(), Some(1));
}

#[test]
fn runtime_doctor_marker_classification_matches_fixed_values_at_log_boundary() {
    assert_eq!(EXPECTED_RUNTIME_DOCTOR_MARKERS.len(), 167);
    let mut log = EXPECTED_RUNTIME_DOCTOR_MARKERS
        .iter()
        .map(|marker| format!(r#"{{"event":"{marker}"}}"#))
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes();
    log.extend_from_slice(b"\n{\"event\":\"not_a_runtime_marker\"}\n");
    let long_marker = format!("selection_pick{}", "x".repeat(257));
    log.extend_from_slice(format!("{{\"event\":\"{long_marker}\"}}\n").as_bytes());
    log.extend_from_slice(b"{\"event\":\"selection_pick\", malformed}\n{\"event\":\"bad \xff\"}\n");

    let summary = summarize_runtime_log_tail(&log);
    let mut expected = EXPECTED_RUNTIME_DOCTOR_MARKERS
        .iter()
        .map(|marker| (String::from(*marker), 1))
        .collect::<std::collections::BTreeMap<_, _>>();
    *expected
        .get_mut("selection_pick")
        .expect("selection marker belongs to the expected catalog") += 1;
    assert_eq!(summary.marker_counts, expected);
}

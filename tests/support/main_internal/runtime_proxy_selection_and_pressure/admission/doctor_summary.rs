use super::*;

#[test]
fn runtime_doctor_summary_counts_recent_runtime_markers() {
    let summary = summarize_runtime_log_tail(
            br#"[2026-03-20 12:00:00.000 +07:00] request=1 transport=http first_upstream_chunk bytes=128
[2026-03-20 12:00:00.010 +07:00] request=1 transport=http first_local_chunk profile=main bytes=128 elapsed_ms=10
[2026-03-20 12:00:00.015 +07:00] runtime_proxy_admission_wait_started transport=http path=/backend-api/codex/responses budget_ms=120 poll_ms=10 reason=responses
[2026-03-20 12:00:00.020 +07:00] profile_transport_backoff profile=main route=responses until=123 seconds=15 context=stream_read_error
[2026-03-20 12:00:00.030 +07:00] profile_health profile=main score=4 delta=4 reason=stream_read_error
[2026-03-20 12:00:00.035 +07:00] selection_skip_affinity route=responses affinity=session profile=main reason=quota_exhausted quota_source=persisted_snapshot
[2026-03-20 12:00:00.040 +07:00] runtime_proxy_active_limit_reached transport=http path=/backend-api/codex/responses active=12 limit=12
[2026-03-20 12:00:00.050 +07:00] runtime_proxy_lane_limit_reached transport=http path=/backend-api/codex/responses lane=responses active=9 limit=9
[2026-03-20 12:00:00.055 +07:00] runtime_proxy_admission_recovered transport=http path=/backend-api/codex/responses waited_ms=20
[2026-03-20 12:00:00.056 +07:00] runtime_proxy_overload_backoff until=123 reason=active_request_limit
[2026-03-20 12:00:00.060 +07:00] profile_inflight_saturated profile=main hard_limit=8
[2026-03-20 12:00:00.070 +07:00] runtime_proxy_queue_overloaded transport=http path=/backend-api/codex/responses reason=long_lived_queue_full
[2026-03-20 12:00:00.072 +07:00] runtime_proxy_queue_wait_started transport=http path=/backend-api/codex/responses budget_ms=120 poll_ms=10 reason=long_lived_queue_full
[2026-03-20 12:00:00.074 +07:00] runtime_proxy_queue_recovered transport=http path=/backend-api/codex/responses waited_ms=18
[2026-03-20 12:00:00.075 +07:00] state_save_queued revision=2 reason=session_id:main backlog=3 ready_in_ms=5
[2026-03-20 12:00:00.078 +07:00] continuation_journal_save_queued reason=session_id:main backlog=2
[2026-03-20 12:00:00.080 +07:00] profile_probe_refresh_queued profile=second reason=queued backlog=4
[2026-03-20 12:00:00.085 +07:00] state_save_skipped revision=2 reason=session_id:main lag_ms=7
[2026-03-20 12:00:00.086 +07:00] continuation_journal_save_ok saved_at=123 reason=session_id:main lag_ms=11
[2026-03-20 12:00:00.090 +07:00] profile_probe_refresh_start profile=second
[2026-03-20 12:00:00.095 +07:00] profile_probe_refresh_ok profile=second lag_ms=13
[2026-03-20 12:00:00.100 +07:00] profile_probe_refresh_error profile=third lag_ms=8 error=timeout
[2026-03-20 12:00:00.094 +07:00] profile_circuit_open profile=main route=responses until=123 reason=stream_read_error score=4
[2026-03-20 12:00:00.095 +07:00] profile_circuit_half_open_probe profile=main route=responses until=128 health=3
[2026-03-20 12:00:00.096 +07:00] websocket_reuse_watchdog profile=main event=read_error elapsed_ms=33 committed=true
[2026-03-20 12:00:00.096 +07:00] request=4 transport=websocket websocket_connect_overflow_enqueue reason=queue_full overflow_pending=1 overflow_max_pending=1 overflow_total_enqueued=1 overflow_total_dispatched=0 worker_count=1 queue_capacity=1
[2026-03-20 12:00:00.096 +07:00] request=4 transport=websocket websocket_connect_overflow_dispatch reason=queue_available overflow_pending=0 overflow_max_pending=1 overflow_total_enqueued=1 overflow_total_dispatched=1 worker_count=1 queue_capacity=1
[2026-03-20 12:00:00.096 +07:00] request=5 transport=websocket websocket_connect_overflow_reject reason=overflow_capacity_reached overflow_pending=2 overflow_max_pending=2 overflow_total_enqueued=3 overflow_total_dispatched=1 worker_count=1 queue_capacity=1
[2026-03-20 12:00:00.096 +07:00] request=5 transport=websocket websocket_connect_overflow_rejected reason=queue_full overflow_pending=3 overflow_max_pending=3 overflow_total_enqueued=4 overflow_total_dispatched=1 worker_count=1 queue_capacity=1
[2026-03-20 12:00:00.096 +07:00] request=8 transport=websocket websocket_dns_resolve_timeout host=slow.example.test port=443 timeout_ms=25
[2026-03-20 12:00:00.096 +07:00] request=6 profile_auth_recovered profile=main route=responses source=refresh changed=true
[2026-03-20 12:00:00.096 +07:00] request=7 profile_auth_recovery_failed profile=second route=websocket error=refresh_failed
[2026-03-20 12:00:00.097 +07:00] request=2 transport=http route=responses previous_response_not_found profile=second response_id=resp-second retry_index=0
[2026-03-20 12:00:00.098 +07:00] request=3 transport=websocket route=websocket previous_response_not_found profile=second response_id=resp-second retry_index=0
[2026-03-20 12:00:00.099 +07:00] request=3 transport=websocket route=websocket websocket_session=sess-1 chain_retried_owner profile=second previous_response_id=resp-second delay_ms=20 reason=previous_response_not_found_locked_affinity via=-
[2026-03-20 12:00:00.100 +07:00] request=3 transport=websocket route=websocket websocket_session=sess-1 stale_continuation reason=previous_response_not_found_locked_affinity profile=second
[2026-03-20 12:00:00.101 +07:00] request=3 transport=websocket route=websocket websocket_session=sess-1 chain_dead_upstream_confirmed profile=second previous_response_id=resp-second reason=previous_response_not_found_locked_affinity via=- event=-
[2026-03-20 12:00:00.102 +07:00] local_writer_error request=1 transport=http profile=main stage=chunk_flush chunks=1 bytes=128 elapsed_ms=20 error=broken_pipe
[2026-03-20 12:00:00.105 +07:00] runtime_proxy_startup_audit missing_managed_dirs=1 stale_response_bindings=2 stale_session_bindings=1 active_profile_missing_dir=false
"#,
        );

    assert!((38..=39).contains(&summary.line_count));
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_queue_overloaded"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_active_limit_reached"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_lane_limit_reached"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_admission_wait_started"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_admission_recovered"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_overload_backoff"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_inflight_saturated"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_transport_backoff"),
        1
    );
    assert_eq!(runtime_doctor_marker_count(&summary, "profile_health"), 1);
    assert_eq!(
        runtime_doctor_marker_count(&summary, "selection_skip_affinity"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_queue_wait_started"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_queue_recovered"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "first_upstream_chunk"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "first_local_chunk"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_start"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_ok"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "state_save_queued"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "continuation_journal_save_queued"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "continuation_journal_save_ok"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "state_save_skipped"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_error"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_circuit_open"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_circuit_half_open_probe"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "websocket_reuse_watchdog"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "websocket_connect_overflow_enqueue"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "websocket_connect_overflow_dispatch"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "websocket_connect_overflow_reject"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "websocket_connect_overflow_rejected"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "websocket_dns_resolve_timeout"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_auth_recovered"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_auth_recovery_failed"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "previous_response_not_found"),
        2
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "chain_retried_owner"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "stale_continuation"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "chain_dead_upstream_confirmed"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "local_writer_error"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_startup_audit"),
        1
    );
    assert_eq!(
        runtime_doctor_top_facet(&summary, "quota_source").as_deref(),
        Some("persisted_snapshot (1)")
    );
    assert_eq!(summary.state_save_queue_backlog, Some(3));
    assert_eq!(summary.state_save_lag_ms, Some(7));
    assert_eq!(summary.continuation_journal_save_backlog, Some(2));
    assert_eq!(summary.continuation_journal_save_lag_ms, Some(11));
    assert_eq!(summary.profile_probe_refresh_backlog, Some(4));
    assert_eq!(summary.profile_probe_refresh_lag_ms, Some(13));
    assert_eq!(
        summary.failure_class_counts,
        BTreeMap::from([
            ("admission".to_string(), 9),
            ("auth".to_string(), 1),
            ("continuation".to_string(), 5),
            ("persistence".to_string(), 1),
            ("quota".to_string(), 5),
            ("transport".to_string(), 4),
        ])
    );
    assert_eq!(
        summary.previous_response_not_found_by_route,
        BTreeMap::from([("responses".to_string(), 1), ("websocket".to_string(), 1)])
    );
    assert_eq!(
        summary.previous_response_not_found_by_transport,
        BTreeMap::from([("http".to_string(), 1), ("websocket".to_string(), 1)])
    );
    assert_eq!(
        summary
            .previous_response_not_found_by_route
            .get("websocket"),
        Some(&1)
    );
    assert_eq!(
        summary.latest_stale_continuation_reason.as_deref(),
        Some("previous_response_not_found_locked_affinity")
    );
    assert_eq!(
        summary.stale_continuation_by_reason,
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1)])
    );
    assert!(summary
        .latest_chain_event
        .as_deref()
        .is_some_and(|event| event.contains("chain_dead_upstream_confirmed")
            && event.contains("previous_response_id=resp-second")));
    assert_eq!(
        summary
            .marker_last_fields
            .get("state_save_queued")
            .and_then(|fields| fields.get("backlog"))
            .map(String::as_str),
        Some("3")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("continuation_journal_save_ok")
            .and_then(|fields| fields.get("lag_ms"))
            .map(String::as_str),
        Some("11")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("profile_probe_refresh_error")
            .and_then(|fields| fields.get("lag_ms"))
            .map(String::as_str),
        Some("8")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("websocket_connect_overflow_rejected")
            .and_then(|fields| fields.get("overflow_pending"))
            .map(String::as_str),
        Some("3")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("profile_auth_recovery_failed")
            .and_then(|fields| fields.get("error"))
            .map(String::as_str),
        Some("refresh_failed")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("selection_skip_affinity")
            .and_then(|fields| fields.get("affinity"))
            .map(String::as_str),
        Some("session")
    );
    assert!(summary
        .last_marker_line
        .as_deref()
        .is_some_and(|line| line.contains("runtime_proxy_startup_audit")));
}

use super::*;

mod diagnosis;
mod parsing;
mod render;
mod state;
mod types;

#[allow(unused_imports)]
pub(crate) use diagnosis::{
    runtime_doctor_finalize_summary, runtime_doctor_marker_count, runtime_doctor_top_facet,
};
#[allow(unused_imports)]
pub(crate) use parsing::{read_runtime_log_tail, summarize_runtime_log_tail};
#[allow(unused_imports)]
pub(crate) use render::{
    runtime_doctor_fields, runtime_doctor_fields_for_summary, runtime_doctor_json_value,
};
#[allow(unused_imports)]
pub(crate) use state::{
    collect_runtime_doctor_state, collect_runtime_doctor_summary, runtime_doctor_degraded_routes,
};
#[allow(unused_imports)]
pub(crate) use types::{
    RuntimeDoctorProfileSummary, RuntimeDoctorRouteSummary, RuntimeDoctorSummary,
};

const RUNTIME_DOCTOR_FACETS: &[&str] = &[
    "lane",
    "route",
    "profile",
    "reason",
    "transport",
    "family",
    "client",
    "tool_surface",
    "continuation",
    "origin",
    "warning",
    "quota_source",
    "quota_band",
    "five_hour_status",
    "weekly_status",
    "affinity",
    "context",
    "event",
    "stage",
    "state",
    "source",
    "request_shape",
    "exit",
];

const RUNTIME_DOCTOR_MARKERS: &[&str] = &[
    "chain_retried_owner",
    "chain_dead_upstream_confirmed",
    "stale_continuation",
    "runtime_proxy_queue_overloaded",
    "runtime_proxy_active_limit_reached",
    "runtime_proxy_lane_limit_reached",
    "runtime_proxy_overload_backoff",
    "runtime_proxy_admission_wait_started",
    "runtime_proxy_admission_recovered",
    "runtime_proxy_queue_wait_started",
    "runtime_proxy_queue_recovered",
    "profile_inflight_saturated",
    "upstream_connect_timeout",
    "upstream_connect_dns_error",
    "upstream_tls_handshake_error",
    "upstream_connect_error",
    "precommit_budget_exhausted",
    "profile_retry_backoff",
    "profile_transport_backoff",
    "profile_circuit_open",
    "profile_circuit_half_open_probe",
    "profile_health",
    "profile_latency",
    "profile_bad_pairing",
    "previous_response_not_found",
    "previous_response_negative_cache",
    "previous_response_fresh_fallback",
    "previous_response_fresh_fallback_blocked",
    "compact_committed_owner",
    "compact_followup_owner",
    "compact_fresh_fallback_blocked",
    "compact_pressure_shed",
    "compact_lineage_released",
    "compact_committed",
    "compact_precommit_budget_exhausted",
    "compact_candidate_exhausted",
    "compact_retryable_failure",
    "compact_overload_conservative_retry",
    "compact_quota_unclassified",
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
    "selection_pick",
    "selection_skip_current",
    "selection_skip_affinity",
    "responses_pre_send_skip",
    "websocket_pre_send_skip",
    "quota_release_profile_affinity",
    "quota_critical_floor_before_send",
    "upstream_usage_limit_passthrough",
    "compat_request_surface",
    "compat_warning",
    "websocket_reuse_skip_quota_exhausted",
    "websocket_reuse_watchdog",
    "websocket_precommit_frame_timeout",
    "stream_read_error",
    "local_writer_error",
    "first_upstream_chunk",
    "first_local_chunk",
    "state_save_ok",
    "state_save_skipped",
    "state_save_error",
    "state_save_queued",
    "continuation_journal_save_ok",
    "continuation_journal_save_error",
    "continuation_journal_save_queued",
    "runtime_proxy_restore_counts",
    "runtime_proxy_startup_audit",
    "profile_probe_refresh_queued",
    "profile_probe_refresh_start",
    "profile_probe_refresh_ok",
    "profile_probe_refresh_error",
    "quota_blocked_affinity_released",
];

const RUNTIME_DOCTOR_COUNT_FIELD_ROWS: &[(&str, &str)] = &[
    ("Queue overload", "runtime_proxy_queue_overloaded"),
    ("Active limit", "runtime_proxy_active_limit_reached"),
    ("Lane limit", "runtime_proxy_lane_limit_reached"),
    ("Overload backoff", "runtime_proxy_overload_backoff"),
    ("Pre-commit budget", "precommit_budget_exhausted"),
    ("Responses pre-send skips", "responses_pre_send_skip"),
    ("Websocket pre-send skips", "websocket_pre_send_skip"),
    (
        "Quota critical floor pre-send",
        "quota_critical_floor_before_send",
    ),
    (
        "Upstream usage-limit passthrough",
        "upstream_usage_limit_passthrough",
    ),
    ("Retry backoff", "profile_retry_backoff"),
    ("Transport backoff", "profile_transport_backoff"),
    ("Route circuits", "profile_circuit_open"),
    ("Health penalties", "profile_health"),
    ("Latency penalties", "profile_latency"),
    ("Bad pairing", "profile_bad_pairing"),
    ("Chain owner retries", "chain_retried_owner"),
    ("Chain dead upstream", "chain_dead_upstream_confirmed"),
    ("Stale continuations", "stale_continuation"),
    ("Prev not found", "previous_response_not_found"),
    ("Prev negative cache", "previous_response_negative_cache"),
    ("Legacy prev recovery", "previous_response_fresh_fallback"),
    (
        "Prev fail-closed",
        "previous_response_fresh_fallback_blocked",
    ),
    ("Compact guard", "compact_fresh_fallback_blocked"),
    ("Compact shed", "compact_pressure_shed"),
    ("Compact committed", "compact_committed"),
    ("Compact budget", "compact_precommit_budget_exhausted"),
    ("Compact exhausted", "compact_candidate_exhausted"),
    ("Compact retry", "compact_retryable_failure"),
    ("Compact owner retry", "compact_overload_conservative_retry"),
    ("Compact quota misc", "compact_quota_unclassified"),
    ("Compact final", "compact_final_failure"),
    ("Selection picks", "selection_pick"),
    ("Selection skips", "selection_skip_current"),
    ("WS reuse watchdog", "websocket_reuse_watchdog"),
    (
        "WS first-frame timeouts",
        "websocket_precommit_frame_timeout",
    ),
    ("Stream read errors", "stream_read_error"),
    ("Writer errors", "local_writer_error"),
    ("State save errors", "state_save_error"),
    ("Cont journal err", "continuation_journal_save_error"),
    ("State save ok", "state_save_ok"),
    ("Cont journal ok", "continuation_journal_save_ok"),
    ("State save skipped", "state_save_skipped"),
    ("Startup audit", "runtime_proxy_startup_audit"),
    ("Admission recovered", "runtime_proxy_admission_recovered"),
    ("Queue recovered", "runtime_proxy_queue_recovered"),
    ("Probe refresh", "profile_probe_refresh_start"),
    ("Probe refresh errors", "profile_probe_refresh_error"),
    ("Compat samples", "compat_request_surface"),
];

const RUNTIME_DOCTOR_SELECTION_PRESSURE_MARKERS: &[&str] = &[
    "selection_pick",
    "selection_skip_current",
    "selection_skip_affinity",
    "precommit_budget_exhausted",
    "compact_precommit_budget_exhausted",
    "compact_candidate_exhausted",
];

const RUNTIME_DOCTOR_TRANSPORT_PRESSURE_MARKERS: &[&str] = &[
    "stream_read_error",
    "upstream_connect_timeout",
    "upstream_connect_dns_error",
    "upstream_tls_handshake_error",
    "upstream_connect_error",
    "profile_transport_backoff",
    "profile_circuit_open",
    "profile_circuit_half_open_probe",
    "websocket_precommit_frame_timeout",
    "local_writer_error",
];

const RUNTIME_DOCTOR_PERSISTENCE_PRESSURE_MARKERS: &[&str] =
    &["state_save_error", "continuation_journal_save_error"];

const RUNTIME_DOCTOR_ACTIVE_PERSISTENCE_MARKERS: &[&str] = &["state_save_skipped"];

const RUNTIME_DOCTOR_ACTIVE_QUOTA_REFRESH_MARKERS: &[&str] =
    &["profile_probe_refresh_start", "profile_probe_refresh_ok"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_runtime_log_tail_understands_json_lines() {
        let tail = br#"{"timestamp":"2026-04-08 10:00:00.000 +00:00","message":"request=7 profile_health profile=main route=responses score=4","fields":{"request":"7","profile":"main","route":"responses","score":"4"}}"#;
        let summary = summarize_runtime_log_tail(tail);

        assert_eq!(summary.line_count, 1);
        assert_eq!(
            summary.marker_counts.get("profile_health").copied(),
            Some(1)
        );
        assert_eq!(
            summary.first_timestamp.as_deref(),
            Some("2026-04-08 10:00:00.000 +00:00")
        );
        assert_eq!(
            summary
                .marker_last_fields
                .get("profile_health")
                .and_then(|fields| fields.get("profile"))
                .map(String::as_str),
            Some("main")
        );
    }
}

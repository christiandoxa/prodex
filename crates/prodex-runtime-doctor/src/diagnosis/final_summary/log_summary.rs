use std::collections::BTreeMap;

use crate::RuntimeDoctorSummary;

use super::super::marker_accessors::*;
use super::runtime_doctor_top_facet;

fn runtime_doctor_failure_class_counts(summary: &RuntimeDoctorSummary) -> BTreeMap<String, usize> {
    let classes: [(&str, &[&str]); 6] = [
        (
            "admission",
            &[
                "runtime_proxy_queue_overloaded",
                "runtime_proxy_active_limit_reached",
                "runtime_proxy_lane_limit_reached",
                "runtime_proxy_overload_backoff",
                "runtime_proxy_admission_wait_started",
                "runtime_proxy_admission_wait_exhausted",
                "runtime_proxy_queue_wait_started",
                "runtime_proxy_queue_wait_exhausted",
                "profile_inflight_saturated",
                "websocket_connect_overflow_reject",
                "websocket_connect_overflow_rejected",
                "compact_precommit_budget_exhausted",
                "compact_candidate_exhausted",
            ],
        ),
        (
            "auth",
            &[
                "profile_auth_recovery_failed",
                "profile_auth_proactive_sync_failed",
                "local_rewrite_provider_auth_failure",
            ],
        ),
        (
            "continuation",
            &[
                "previous_response_not_found",
                "previous_response_negative_cache",
                "previous_response_fresh_fallback",
                "chain_retried_owner",
                "chain_dead_upstream_confirmed",
                "stale_continuation",
                "previous_response_fresh_fallback_blocked",
                "compact_fresh_fallback_blocked",
                "compact_pressure_shed",
            ],
        ),
        (
            "persistence",
            &[
                "state_save_error",
                "state_save_queue_backpressure",
                "state_save_skipped",
                "continuation_journal_save_error",
                "continuation_journal_queue_backpressure",
            ],
        ),
        (
            "quota",
            &[
                "profile_retry_backoff",
                "profile_transport_backoff",
                "profile_circuit_open",
                "profile_circuit_half_open_probe",
                "profile_health",
                "profile_latency",
                "profile_bad_pairing",
                "profile_quota_quarantine",
                "profile_auth_backoff",
                "profile_probe_refresh_error",
                "profile_probe_refresh_panic",
                "profile_probe_refresh_backpressure",
                "selection_skip_sync_probe",
                "runtime_proxy_sync_probe_pressure_pause",
                "local_selection_blocked",
                "responses_pre_send_skip",
                "websocket_pre_send_skip",
                "quota_blocked",
                "quota_critical_floor_before_send",
                "upstream_usage_limit_passthrough",
                "upstream_overload_passthrough",
                "upstream_overloaded",
                "compact_retryable_failure",
                "compact_overload_conservative_retry",
                "compact_quota_unclassified",
                "local_rewrite_gemini_quota_rotate",
                "local_rewrite_gemini_rate_limit_retry",
                "local_rewrite_gemini_quota_status_unavailable",
            ],
        ),
        (
            "transport",
            &[
                "upstream_connect_timeout",
                "upstream_connect_dns_error",
                "upstream_tls_handshake_error",
                "upstream_connect_error",
                "upstream_connect_http",
                "upstream_close_before_completed",
                "upstream_connection_closed",
                "upstream_read_error",
                "upstream_send_error",
                "upstream_stream_error",
                "profile_transport_failure",
                "stream_read_error",
                "local_writer_error",
                "websocket_precommit_frame_timeout",
                "websocket_precommit_hold_timeout",
                "websocket_dns_resolve_timeout",
                "websocket_dns_overflow_enqueue",
                "websocket_dns_overflow_dispatch",
                "websocket_dns_overflow_reject",
                "websocket_connect_local_pressure",
                "websocket_connect_overflow_enqueue",
                "websocket_connect_overflow_dispatch",
                "local_rewrite_gemini_invalid_stream_retry",
                "local_rewrite_gemini_invalid_stream_model_fallback",
                "local_rewrite_gemini_live_error",
                "local_rewrite_gemini_live_sidecar_error",
                "local_rewrite_gemini_live_sidecar_accept_error",
                "local_rewrite_gemini_live_sidecar_session_error",
            ],
        ),
    ];

    classes
        .into_iter()
        .map(|(class, markers)| {
            (
                class.to_string(),
                markers
                    .iter()
                    .map(|marker| runtime_doctor_marker_count(summary, marker))
                    .sum(),
            )
        })
        .filter(|(_, count)| *count > 0)
        .collect()
}

pub fn runtime_doctor_finalize_log_summary(summary: &mut RuntimeDoctorSummary) {
    summary.state_save_queue_backlog =
        runtime_doctor_marker_last_usize_field(summary, "state_save_queue_backpressure", "backlog")
            .or_else(|| {
                runtime_doctor_marker_last_usize_field(summary, "state_save_queued", "backlog")
            });
    summary.state_save_lag_ms =
        runtime_doctor_marker_last_u64_field(summary, "state_save_ok", "lag_ms")
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(summary, "state_save_skipped", "lag_ms")
            })
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(summary, "state_save_error", "lag_ms")
            });
    summary.continuation_journal_save_backlog = runtime_doctor_marker_last_usize_field(
        summary,
        "continuation_journal_queue_backpressure",
        "backlog",
    )
    .or_else(|| {
        runtime_doctor_marker_last_usize_field(
            summary,
            "continuation_journal_save_queued",
            "backlog",
        )
    });
    summary.continuation_journal_save_lag_ms =
        runtime_doctor_marker_last_u64_field(summary, "continuation_journal_save_ok", "lag_ms")
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(
                    summary,
                    "continuation_journal_save_error",
                    "lag_ms",
                )
            });
    summary.profile_probe_refresh_backlog = runtime_doctor_marker_last_usize_field(
        summary,
        "profile_probe_refresh_backpressure",
        "backlog",
    )
    .or_else(|| {
        runtime_doctor_marker_last_usize_field(summary, "profile_probe_refresh_queued", "backlog")
    });
    summary.profile_probe_refresh_lag_ms =
        runtime_doctor_marker_last_u64_field(summary, "profile_probe_refresh_ok", "lag_ms")
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(
                    summary,
                    "profile_probe_refresh_error",
                    "lag_ms",
                )
            });
    // Count quota-floor pre-send skips via the reason facet so the doctor keeps
    // exposing the hardening signal even though the runtime logs it as a reason,
    // not as a standalone marker.
    let quota_floor_before_send_count =
        runtime_doctor_facet_count(summary, "reason", "quota_critical_floor_before_send");
    if quota_floor_before_send_count > 0 {
        *summary
            .marker_counts
            .entry("quota_critical_floor_before_send")
            .or_insert(0) += quota_floor_before_send_count;
    }
    summary.compat_warning_count = runtime_doctor_marker_count(summary, "compat_warning");
    summary.top_client_family = runtime_doctor_top_facet(summary, "family");
    summary.top_client = runtime_doctor_top_facet(summary, "client");
    summary.top_tool_surface = runtime_doctor_top_facet(summary, "tool_surface");
    summary.top_compat_warning = runtime_doctor_top_facet(summary, "warning");
    summary.failure_class_counts = runtime_doctor_failure_class_counts(summary);
}

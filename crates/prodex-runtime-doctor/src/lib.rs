use std::collections::BTreeMap;

pub mod diagnosis;
#[cfg(test)]
#[path = "../tests/src/marker_guard.rs"]
mod marker_guard;
pub mod parsing;
pub mod render;
pub mod state_summary;
pub mod suggestions;
pub mod types;

pub use parsing::{read_runtime_log_tail, summarize_runtime_log_tail};
pub use render::{
    runtime_doctor_fields_for_summary, runtime_doctor_json_value,
    runtime_doctor_json_value_with_policy_suggestions,
};
pub use state_summary::{
    RuntimeDoctorBackoffMaps, RuntimeDoctorBinaryIdentity, RuntimeDoctorBindingSourceInput,
    RuntimeDoctorBindingStateInput, RuntimeDoctorHealthScore, RuntimeDoctorQuotaPressureBand,
    RuntimeDoctorQuotaWindowStatus, RuntimeDoctorRouteKind, RuntimeDoctorStateSummaryConfig,
    RuntimeDoctorUsageSnapshot, runtime_doctor_backoff_maps_from_runtime,
    runtime_doctor_binding_state_summary, runtime_doctor_degraded_routes,
    runtime_doctor_health_scores_from_runtime, runtime_doctor_profile_summaries,
    runtime_doctor_quota_freshness_label, runtime_doctor_quota_window_status_from_runtime,
    runtime_doctor_route_circuit_state, runtime_doctor_route_kind_label,
    runtime_doctor_runtime_broker_mismatch_reason, runtime_doctor_usage_snapshot_from_runtime,
    runtime_doctor_usage_snapshots_from_runtime,
};
pub use suggestions::{
    RuntimeDoctorPolicySettingSuggestion, RuntimeDoctorPolicySuggestion,
    runtime_doctor_policy_suggestion_lines, runtime_doctor_policy_suggestions,
};
pub use types::{
    RuntimeDoctorBindingProfileSummary, RuntimeDoctorBindingSourceSummary,
    RuntimeDoctorBindingStateSummary, RuntimeDoctorProfileSummary,
    RuntimeDoctorRequestTimelineEvent, RuntimeDoctorRouteSummary, RuntimeDoctorSummary,
};

pub const RUNTIME_DOCTOR_FACETS: &[&str] = &[
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
    "mode",
];

pub const RUNTIME_DOCTOR_MARKERS: &[&str] = &[
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
    "compat_request_surface",
    "compat_warning",
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

pub const RUNTIME_DOCTOR_COUNT_FIELD_ROWS: &[(&str, &str)] = &[
    ("Queue overload", "runtime_proxy_queue_overloaded"),
    ("Active limit", "runtime_proxy_active_limit_reached"),
    ("Lane limit", "runtime_proxy_lane_limit_reached"),
    ("In-flight saturated", "profile_inflight_saturated"),
    ("Overload backoff", "runtime_proxy_overload_backoff"),
    (
        "Admission wait exhausted",
        "runtime_proxy_admission_wait_exhausted",
    ),
    ("Queue wait exhausted", "runtime_proxy_queue_wait_exhausted"),
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
    ("Sync-probe skips", "selection_skip_sync_probe"),
    ("WS reuse watchdog", "websocket_reuse_watchdog"),
    (
        "WS first-frame timeouts",
        "websocket_precommit_frame_timeout",
    ),
    (
        "WS precommit hold timeouts",
        "websocket_precommit_hold_timeout",
    ),
    ("WS DNS timeouts", "websocket_dns_resolve_timeout"),
    ("WS DNS overflow enqueue", "websocket_dns_overflow_enqueue"),
    (
        "WS DNS overflow dispatch",
        "websocket_dns_overflow_dispatch",
    ),
    ("WS DNS overflow reject", "websocket_dns_overflow_reject"),
    (
        "WS connect local pressure",
        "websocket_connect_local_pressure",
    ),
    (
        "WS connect overflow enqueue",
        "websocket_connect_overflow_enqueue",
    ),
    (
        "WS connect overflow dispatch",
        "websocket_connect_overflow_dispatch",
    ),
    (
        "WS connect overflow reject",
        "websocket_connect_overflow_reject",
    ),
    (
        "WS connect overflow rejected",
        "websocket_connect_overflow_rejected",
    ),
    ("WS proxy tunnel ok", "websocket_proxy_tunnel_ok"),
    ("WS proxy tunnel failure", "websocket_proxy_tunnel_failure"),
    ("Auth recovered", "profile_auth_recovered"),
    ("Auth recovery failed", "profile_auth_recovery_failed"),
    ("Stream read errors", "stream_read_error"),
    ("Writer errors", "local_writer_error"),
    ("State save errors", "state_save_error"),
    ("State save pressure", "state_save_queue_backpressure"),
    ("Cont journal err", "continuation_journal_save_error"),
    (
        "Cont journal pressure",
        "continuation_journal_queue_backpressure",
    ),
    ("State save ok", "state_save_ok"),
    ("Cont journal ok", "continuation_journal_save_ok"),
    ("State save skipped", "state_save_skipped"),
    ("Startup audit", "runtime_proxy_startup_audit"),
    ("Admission recovered", "runtime_proxy_admission_recovered"),
    ("Queue recovered", "runtime_proxy_queue_recovered"),
    ("Probe refresh", "profile_probe_refresh_start"),
    ("Probe refresh errors", "profile_probe_refresh_error"),
    (
        "Probe refresh pressure",
        "profile_probe_refresh_backpressure",
    ),
    ("Compat samples", "compat_request_surface"),
];

pub const RUNTIME_DOCTOR_SELECTION_PRESSURE_MARKERS: &[&str] = &[
    "selection_keep_affinity",
    "selection_keep_current",
    "selection_pick",
    "selection_skip_current",
    "selection_skip_affinity",
    "selection_skip_sync_probe",
    "local_selection_blocked",
    "precommit_budget_exhausted",
    "compact_precommit_budget_exhausted",
    "compact_candidate_exhausted",
];

pub const RUNTIME_DOCTOR_TRANSPORT_PRESSURE_MARKERS: &[&str] = &[
    "stream_read_error",
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
    "profile_transport_backoff",
    "profile_circuit_open",
    "profile_circuit_half_open_probe",
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
    "websocket_proxy_tunnel_failure",
    "local_writer_error",
];

pub const RUNTIME_DOCTOR_PERSISTENCE_PRESSURE_MARKERS: &[&str] = &[
    "state_save_error",
    "state_save_queue_backpressure",
    "continuation_journal_save_error",
    "continuation_journal_queue_backpressure",
];

pub const RUNTIME_DOCTOR_ACTIVE_PERSISTENCE_MARKERS: &[&str] = &["state_save_skipped"];

pub const RUNTIME_DOCTOR_ACTIVE_QUOTA_REFRESH_MARKERS: &[&str] =
    &["profile_probe_refresh_start", "profile_probe_refresh_ok"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorTuningLaneLimits {
    pub responses: usize,
    pub compact: usize,
    pub websocket: usize,
    pub standard: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorTuningSnapshot {
    pub active_request_limit: usize,
    pub lane_limits: RuntimeDoctorTuningLaneLimits,
    pub admission_wait_budget_ms: u64,
    pub pressure_admission_wait_budget_ms: u64,
    pub websocket_connect_worker_count: usize,
    pub websocket_connect_queue_capacity: usize,
    pub websocket_connect_overflow_capacity: usize,
    pub websocket_dns_worker_count: usize,
    pub websocket_dns_queue_capacity: usize,
    pub websocket_dns_overflow_capacity: usize,
    pub profile_inflight_soft_limit: usize,
    pub profile_inflight_hard_limit: usize,
}

impl Default for RuntimeDoctorTuningSnapshot {
    fn default() -> Self {
        Self {
            active_request_limit: 8,
            lane_limits: RuntimeDoctorTuningLaneLimits {
                responses: 6,
                compact: 1,
                websocket: 1,
                standard: 2,
            },
            admission_wait_budget_ms: 0,
            pressure_admission_wait_budget_ms: 250,
            websocket_connect_worker_count: 4,
            websocket_connect_queue_capacity: 8,
            websocket_connect_overflow_capacity: 16,
            websocket_dns_worker_count: 4,
            websocket_dns_queue_capacity: 8,
            websocket_dns_overflow_capacity: 16,
            profile_inflight_soft_limit: 2,
            profile_inflight_hard_limit: 4,
        }
    }
}

fn runtime_proxy_skip_log_whitespace(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_proxy_skip_log_field_value(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    if index >= bytes.len() {
        return index;
    }
    if bytes[index] == b'"' {
        index += 1;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if escaped {
                escaped = false;
                index += 1;
                continue;
            }
            match byte {
                b'\\' => {
                    escaped = true;
                    index += 1;
                }
                b'"' => {
                    index += 1;
                    break;
                }
                _ => index += 1,
            }
        }
        return index;
    }
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_proxy_parse_log_field_value(raw_value: &str) -> String {
    if raw_value.starts_with('"') {
        serde_json::from_str::<String>(raw_value)
            .unwrap_or_else(|_| raw_value.trim_matches('"').to_string())
    } else {
        raw_value.trim_matches('"').to_string()
    }
}

pub fn runtime_proxy_log_fields(message: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let bytes = message.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        index = runtime_proxy_skip_log_whitespace(message, index);
        if index >= bytes.len() {
            break;
        }

        let key_start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            continue;
        }

        let key = &message[key_start..index];
        index += 1;
        let value_start = index;
        let value_end = runtime_proxy_skip_log_field_value(message, index);
        index = value_end;
        let raw_value = &message[value_start..value_end];
        if key.is_empty() || raw_value.is_empty() {
            continue;
        }
        fields.insert(
            key.to_string(),
            runtime_proxy_parse_log_field_value(raw_value),
        );
    }
    fields
}

use std::collections::BTreeMap;

use crate::{RuntimeDoctorRequestTimelineEvent, RuntimeDoctorSummary};

const RUNTIME_DOCTOR_REQUEST_TIMELINE_MAX_EVENTS: usize = 12;

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeDoctorRequestTimelineBuilder {
    last_index: usize,
    events: Vec<RuntimeDoctorRequestTimelineEvent>,
}

fn runtime_doctor_request_id(fields: &BTreeMap<String, String>) -> Option<String> {
    fields
        .get("request_id")
        .or_else(|| fields.get("request"))
        .filter(|request_id| !request_id.is_empty())
        .cloned()
}

fn runtime_doctor_request_timeline_phase(marker: &str) -> Option<&'static str> {
    match marker {
        "selection_keep_affinity"
        | "selection_keep_current"
        | "selection_pick"
        | "selection_skip_current"
        | "selection_skip_affinity"
        | "selection_skip_sync_probe"
        | "local_selection_blocked" => Some("selection"),
        "responses_pre_send_skip"
        | "websocket_pre_send_skip"
        | "quota_critical_floor_before_send"
        | "compact_pre_send_allow_quota_exhausted" => Some("pre_send"),
        "upstream_connect_timeout"
        | "upstream_connect_dns_error"
        | "upstream_tls_handshake_error"
        | "upstream_connect_error"
        | "upstream_connect_http"
        | "upstream_overload_passthrough"
        | "upstream_overloaded"
        | "upstream_read_error"
        | "upstream_send_error"
        | "upstream_stream_error"
        | "upstream_close_before_completed"
        | "upstream_connection_closed"
        | "upstream_usage_limit_passthrough"
        | "first_upstream_chunk" => Some("upstream"),
        "first_local_chunk"
        | "previous_response_owner"
        | "compact_committed"
        | "compact_committed_owner"
        | "compact_followup_owner"
        | "compact_exit_committed"
        | "compact_exit_committed_owner"
        | "compact_exit_followup_owner" => Some("commit"),
        "runtime_proxy_queue_overloaded"
        | "runtime_proxy_active_limit_reached"
        | "runtime_proxy_lane_limit_reached"
        | "runtime_proxy_overload_backoff"
        | "runtime_proxy_admission_wait_exhausted"
        | "runtime_proxy_queue_wait_exhausted"
        | "profile_inflight_saturated"
        | "precommit_budget_exhausted"
        | "profile_retry_backoff"
        | "profile_transport_backoff"
        | "profile_transport_failure"
        | "profile_circuit_open"
        | "profile_bad_pairing"
        | "profile_auth_recovery_failed"
        | "previous_response_not_found"
        | "previous_response_negative_cache"
        | "previous_response_fresh_fallback_blocked"
        | "compact_fresh_fallback_blocked"
        | "compact_pressure_shed"
        | "compact_precommit_budget_exhausted"
        | "compact_candidate_exhausted"
        | "compact_retryable_failure"
        | "compact_transport_failure"
        | "compact_final_failure"
        | "compact_exit_fresh_fallback_blocked"
        | "compact_exit_pressure_shed"
        | "compact_exit_precommit_budget_exhausted"
        | "compact_exit_candidate_exhausted"
        | "compact_exit_retryable_failure"
        | "websocket_precommit_frame_timeout"
        | "websocket_precommit_hold_timeout"
        | "websocket_dns_resolve_timeout"
        | "websocket_dns_overflow_reject"
        | "websocket_connect_overflow_reject"
        | "websocket_connect_overflow_rejected"
        | "stream_read_error"
        | "local_writer_error"
        | "chain_dead_upstream_confirmed"
        | "stale_continuation"
        | "quota_blocked" => Some("fail"),
        _ => None,
    }
}

fn runtime_doctor_truncate_value(value: &str, limit: usize) -> String {
    let count = value.chars().count();
    if count <= limit {
        return value.to_string();
    }
    value
        .chars()
        .take(limit.saturating_sub(3))
        .collect::<String>()
        + "..."
}

pub(super) fn runtime_doctor_request_timeline_detail(fields: &BTreeMap<String, String>) -> String {
    let mut parts = Vec::new();
    for key in [
        "profile",
        "route",
        "transport",
        "reason",
        "status",
        "code",
        "exit",
        "outcome",
        "quota_source",
        "affinity",
        "request_shape",
        "retry_index",
        "attempt",
        "response_id",
        "previous_response_id",
        "session_id",
    ] {
        if let Some(value) = fields.get(key) {
            parts.push(format!(
                "{key}={}",
                runtime_doctor_truncate_value(value, 48)
            ));
        }
        if parts.len() >= 5 {
            break;
        }
    }
    parts.join(" ")
}

pub(super) fn runtime_doctor_record_request_timeline_event(
    request_timelines: &mut BTreeMap<String, RuntimeDoctorRequestTimelineBuilder>,
    line_index: usize,
    timestamp: Option<&str>,
    marker: &'static str,
    fields: &BTreeMap<String, String>,
) {
    let Some(phase) = runtime_doctor_request_timeline_phase(marker) else {
        return;
    };
    let Some(request_id) = runtime_doctor_request_id(fields) else {
        return;
    };
    let builder = request_timelines.entry(request_id).or_default();
    builder.last_index = line_index;
    builder.events.push(RuntimeDoctorRequestTimelineEvent {
        timestamp: timestamp.map(ToString::to_string),
        phase: phase.to_string(),
        marker: marker.to_string(),
        detail: runtime_doctor_request_timeline_detail(fields),
    });
    if builder.events.len() > RUNTIME_DOCTOR_REQUEST_TIMELINE_MAX_EVENTS {
        builder.events.remove(0);
    }
}

pub(super) fn runtime_doctor_set_latest_request_timeline(
    summary: &mut RuntimeDoctorSummary,
    request_timelines: BTreeMap<String, RuntimeDoctorRequestTimelineBuilder>,
) {
    let Some((request_id, builder)) = request_timelines
        .into_iter()
        .max_by_key(|(_, builder)| builder.last_index)
    else {
        return;
    };
    summary.latest_request_id = Some(request_id);
    summary.latest_request_timeline = builder.events;
}

use anyhow::{Context, Result};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::*;

const RUNTIME_DOCTOR_REQUEST_TIMELINE_MAX_EVENTS: usize = 12;

#[derive(Debug, Clone, Default)]
struct RuntimeDoctorRequestTimelineBuilder {
    last_index: usize,
    events: Vec<RuntimeDoctorRequestTimelineEvent>,
}

pub(crate) fn read_runtime_log_tail(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("failed to inspect {}", path.display()))?
        .len();
    let start = len.saturating_sub(max_bytes as u64);
    file.seek(SeekFrom::Start(start))
        .with_context(|| format!("failed to seek {}", path.display()))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if start > 0
        && let Some(position) = buffer.iter().position(|byte| *byte == b'\n')
    {
        buffer.drain(..=position);
    }
    Ok(buffer)
}

pub(crate) fn summarize_runtime_log_tail(tail: &[u8]) -> RuntimeDoctorSummary {
    let text = String::from_utf8_lossy(tail);
    let mut summary = RuntimeDoctorSummary::default();
    let mut request_timelines: BTreeMap<String, RuntimeDoctorRequestTimelineBuilder> =
        BTreeMap::new();
    for (line_index, line) in text.lines().enumerate() {
        summary.line_count += 1;
        let line_timestamp = runtime_doctor_line_timestamp(line);
        if let Some(timestamp) = line_timestamp.clone() {
            if summary.first_timestamp.is_none() {
                summary.first_timestamp = Some(timestamp.clone());
            }
            summary.last_timestamp = Some(timestamp);
        }
        if let Some(marker) = runtime_doctor_marker_name(line) {
            *summary.marker_counts.entry(marker).or_insert(0) += 1;
            summary.last_marker_line = Some(runtime_doctor_truncate_line(line, 160));
            let fields = runtime_doctor_parse_fields(line);
            if matches!(
                marker,
                "chain_retried_owner" | "chain_dead_upstream_confirmed" | "stale_continuation"
            ) {
                summary.latest_chain_event =
                    Some(runtime_doctor_chain_event_summary(marker, &fields));
            }
            if let Some(reason) = fields.get("reason").cloned() {
                match marker {
                    "chain_retried_owner" => {
                        *summary
                            .chain_retried_owner_by_reason
                            .entry(reason)
                            .or_insert(0) += 1;
                    }
                    "chain_dead_upstream_confirmed" => {
                        *summary
                            .chain_dead_upstream_confirmed_by_reason
                            .entry(reason)
                            .or_insert(0) += 1;
                    }
                    "stale_continuation" => {
                        summary.latest_stale_continuation_reason = Some(reason.clone());
                        *summary
                            .stale_continuation_by_reason
                            .entry(reason)
                            .or_insert(0) += 1;
                    }
                    _ => {}
                }
            }
            if marker == "previous_response_not_found" {
                if let Some(route) = fields.get("route").cloned() {
                    *summary
                        .previous_response_not_found_by_route
                        .entry(route)
                        .or_insert(0) += 1;
                }
                if let Some(transport) = fields.get("transport").cloned() {
                    *summary
                        .previous_response_not_found_by_transport
                        .entry(transport)
                        .or_insert(0) += 1;
                }
            }
            if marker == "previous_response_fresh_fallback_blocked"
                && let Some(request_shape) = fields.get("request_shape").cloned()
            {
                *summary
                    .previous_response_fresh_fallback_blocked_by_request_shape
                    .entry(request_shape)
                    .or_insert(0) += 1;
            }
            let timeline_fields = fields.clone();
            for facet in RUNTIME_DOCTOR_FACETS {
                if let Some(value) = fields.get(*facet).cloned() {
                    *summary
                        .facet_counts
                        .entry((*facet).to_string())
                        .or_default()
                        .entry(value)
                        .or_insert(0) += 1;
                }
            }
            if !fields.is_empty() {
                summary.marker_last_fields.insert(marker, fields);
            }
            runtime_doctor_record_request_timeline_event(
                &mut request_timelines,
                line_index,
                line_timestamp.as_deref(),
                marker,
                &timeline_fields,
            );
        }
    }
    runtime_doctor_set_latest_request_timeline(&mut summary, request_timelines);
    diagnosis::runtime_doctor_finalize_log_summary(&mut summary);
    summary
}

fn runtime_doctor_line_timestamp(line: &str) -> Option<String> {
    if let Some(value) = runtime_doctor_json_line_value(line) {
        return value
            .get("timestamp")
            .or_else(|| value.get("ts"))
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);
    }
    let end = line.find("] ")?;
    line.strip_prefix('[')
        .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
        .map(ToString::to_string)
}

fn runtime_doctor_json_line_value(line: &str) -> Option<serde_json::Value> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn runtime_doctor_line_message<'a>(line: &'a str) -> Cow<'a, str> {
    if let Some(value) = runtime_doctor_json_line_value(line)
        && let Some(message) = value.get("message").and_then(serde_json::Value::as_str)
    {
        return Cow::Owned(message.to_string());
    }
    Cow::Borrowed(
        line.split_once("] ")
            .map(|(_, message)| message)
            .unwrap_or(line)
            .trim(),
    )
}

fn runtime_doctor_parse_fields(line: &str) -> BTreeMap<String, String> {
    let message = runtime_doctor_line_message(line);
    let mut fields = runtime_doctor_parse_message_fields(&message);
    if let Some(value) = runtime_doctor_json_line_value(line)
        && let Some(json_fields) = value.get("fields").and_then(serde_json::Value::as_object)
    {
        for (key, value) in json_fields {
            let string_value = match value {
                serde_json::Value::String(value) => value.clone(),
                serde_json::Value::Number(value) => value.to_string(),
                serde_json::Value::Bool(value) => value.to_string(),
                _ => continue,
            };
            fields.insert(key.clone(), string_value);
        }
    }

    fields
}

fn runtime_doctor_parse_message_fields(message: &str) -> BTreeMap<String, String> {
    runtime_proxy_log_fields(message)
}

fn runtime_doctor_marker_name(line: &str) -> Option<&'static str> {
    let message = runtime_doctor_line_message(line);
    for token in message.split_whitespace() {
        if token.contains('=') {
            continue;
        }
        if let Some(marker) = RUNTIME_DOCTOR_MARKERS
            .iter()
            .copied()
            .find(|marker| *marker == token)
        {
            return Some(marker);
        }
    }
    RUNTIME_DOCTOR_MARKERS
        .iter()
        .copied()
        .find(|marker| message.contains(marker))
}

fn runtime_doctor_chain_event_summary(marker: &str, fields: &BTreeMap<String, String>) -> String {
    let mut parts = vec![marker.to_string()];
    for key in [
        "reason",
        "profile",
        "transport",
        "route",
        "websocket_session",
        "previous_response_id",
        "event",
        "via",
    ] {
        if let Some(value) = fields.get(key) {
            parts.push(format!("{key}={value}"));
        }
    }
    parts.join(" ")
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

fn runtime_doctor_request_timeline_detail(fields: &BTreeMap<String, String>) -> String {
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

fn runtime_doctor_record_request_timeline_event(
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

fn runtime_doctor_set_latest_request_timeline(
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

fn runtime_doctor_truncate_line(line: &str, limit: usize) -> String {
    let trimmed = line.trim();
    let count = trimmed.chars().count();
    if count <= limit {
        return trimmed.to_string();
    }
    trimmed
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>()
        + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_doctor_parse_message_fields_handles_quoted_structured_values() {
        let fields = runtime_doctor_parse_message_fields(
            "stream_read_error request=7 transport=http error=\"failed with spaces\" empty=\"\"",
        );

        assert_eq!(fields.get("request").map(String::as_str), Some("7"));
        assert_eq!(
            fields.get("error").map(String::as_str),
            Some("failed with spaces")
        );
        assert_eq!(fields.get("empty").map(String::as_str), Some(""));
    }
}

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::*;

mod log_line;
mod request_timeline;
mod route_profile;
mod selection;

pub(crate) use log_line::RuntimeDoctorParsedLogLine;
#[cfg(test)]
use log_line::runtime_doctor_parse_message_fields;
use log_line::{runtime_doctor_chain_event_summary, runtime_doctor_truncate_line};
use request_timeline::{
    RuntimeDoctorRequestTimelineBuilder, runtime_doctor_record_request_timeline_event,
    runtime_doctor_set_latest_request_timeline,
};
use route_profile::runtime_doctor_record_route_profile_event;
use selection::runtime_doctor_record_selection_summary;

fn runtime_doctor_count_context_value(
    counts: &mut BTreeMap<String, usize>,
    fields: &BTreeMap<String, String>,
    key: &str,
) {
    let Some(value) = fields.get(key) else {
        return;
    };
    if value.is_empty() || value == "-" {
        return;
    }
    *counts.entry(value.clone()).or_insert(0) += 1;
}

fn runtime_doctor_record_marker_context(
    context: &mut BTreeMap<&'static str, RuntimeDoctorMarkerContextSummary>,
    marker: &'static str,
    fields: &BTreeMap<String, String>,
) {
    let entry = context
        .entry(marker)
        .or_insert_with(|| RuntimeDoctorMarkerContextSummary {
            marker: marker.to_string(),
            ..RuntimeDoctorMarkerContextSummary::default()
        });
    entry.total += 1;
    runtime_doctor_count_context_value(&mut entry.routes, fields, "route");
    runtime_doctor_count_context_value(&mut entry.lanes, fields, "lane");
    runtime_doctor_count_context_value(&mut entry.profiles, fields, "profile");
}

fn runtime_doctor_marker_context_summary(
    context: BTreeMap<&'static str, RuntimeDoctorMarkerContextSummary>,
) -> Vec<RuntimeDoctorMarkerContextSummary> {
    let mut summary = context
        .into_values()
        .filter(|entry| {
            !entry.routes.is_empty() || !entry.lanes.is_empty() || !entry.profiles.is_empty()
        })
        .collect::<Vec<_>>();
    summary.sort_by(|left, right| {
        right
            .total
            .cmp(&left.total)
            .then_with(|| left.marker.cmp(&right.marker))
    });
    summary
}

fn runtime_doctor_record_marker_reason(
    summary: &mut RuntimeDoctorSummary,
    marker: &'static str,
    fields: &BTreeMap<String, String>,
) {
    let Some(reason) = fields.get("reason").cloned() else {
        return;
    };
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

fn runtime_doctor_record_continuation_fields(
    summary: &mut RuntimeDoctorSummary,
    marker: &'static str,
    fields: &BTreeMap<String, String>,
) {
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
}

fn runtime_doctor_record_marker_facets(
    summary: &mut RuntimeDoctorSummary,
    fields: &BTreeMap<String, String>,
) {
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
}

fn runtime_doctor_record_parsed_marker(
    summary: &mut RuntimeDoctorSummary,
    request_timelines: &mut BTreeMap<String, RuntimeDoctorRequestTimelineBuilder>,
    marker_context: &mut BTreeMap<&'static str, RuntimeDoctorMarkerContextSummary>,
    line: (usize, Option<&str>, &str),
    marker: &'static str,
    fields: BTreeMap<String, String>,
) {
    let (line_index, line_timestamp, line) = line;
    *summary.marker_counts.entry(marker).or_insert(0) += 1;
    summary.last_marker_line = Some(runtime_doctor_truncate_line(line, 160));
    if matches!(
        marker,
        "chain_retried_owner" | "chain_dead_upstream_confirmed" | "stale_continuation"
    ) {
        summary.latest_chain_event = Some(runtime_doctor_chain_event_summary(marker, &fields));
    }
    runtime_doctor_record_marker_reason(summary, marker, &fields);
    runtime_doctor_record_continuation_fields(summary, marker, &fields);
    runtime_doctor_record_marker_context(marker_context, marker, &fields);
    runtime_doctor_record_marker_facets(summary, &fields);
    if !fields.is_empty() {
        summary.marker_last_fields.insert(marker, fields.clone());
    }
    runtime_doctor_record_selection_summary(summary, marker, &fields);
    runtime_doctor_record_route_profile_event(summary, line_timestamp, marker, &fields);
    runtime_doctor_record_request_timeline_event(
        request_timelines,
        line_index,
        line_timestamp,
        marker,
        &fields,
    );
}

pub fn read_runtime_log_tail(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
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

pub fn summarize_runtime_log_tail(tail: &[u8]) -> RuntimeDoctorSummary {
    let text = String::from_utf8_lossy(tail);
    let mut summary = RuntimeDoctorSummary::default();
    let mut request_timelines: BTreeMap<String, RuntimeDoctorRequestTimelineBuilder> =
        BTreeMap::new();
    let mut marker_context: BTreeMap<&'static str, RuntimeDoctorMarkerContextSummary> =
        BTreeMap::new();
    for (line_index, line) in text.lines().enumerate() {
        let parsed_line = RuntimeDoctorParsedLogLine::new(line);
        summary.line_count += 1;
        let line_timestamp = parsed_line.timestamp();
        if let Some(timestamp) = line_timestamp.clone() {
            if summary.first_timestamp.is_none() {
                summary.first_timestamp = Some(timestamp.clone());
            }
            summary.last_timestamp = Some(timestamp);
        }
        if let Some(marker) = parsed_line.marker_name() {
            let fields = parsed_line.fields();
            runtime_doctor_record_parsed_marker(
                &mut summary,
                &mut request_timelines,
                &mut marker_context,
                (line_index, line_timestamp.as_deref(), line),
                marker,
                fields,
            );
        }
    }
    summary.marker_context_summary = runtime_doctor_marker_context_summary(marker_context);
    runtime_doctor_set_latest_request_timeline(&mut summary, request_timelines);
    diagnosis::runtime_doctor_finalize_log_summary(&mut summary);
    summary
}

#[cfg(test)]
#[path = "../tests/src/parsing.rs"]
mod tests;

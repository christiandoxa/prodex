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

#[cfg(test)]
use log_line::runtime_doctor_parse_message_fields;
use log_line::{
    runtime_doctor_chain_event_summary, runtime_doctor_line_timestamp, runtime_doctor_marker_name,
    runtime_doctor_parse_fields, runtime_doctor_truncate_line,
};
use request_timeline::{
    RuntimeDoctorRequestTimelineBuilder, runtime_doctor_record_request_timeline_event,
    runtime_doctor_set_latest_request_timeline,
};
use route_profile::runtime_doctor_record_route_profile_event;
use selection::runtime_doctor_record_selection_summary;

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
            runtime_doctor_record_selection_summary(&mut summary, marker, &timeline_fields);
            runtime_doctor_record_route_profile_event(
                &mut summary,
                line_timestamp.as_deref(),
                marker,
                &timeline_fields,
            );
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

#[cfg(test)]
#[path = "../tests/src/parsing.rs"]
mod tests;

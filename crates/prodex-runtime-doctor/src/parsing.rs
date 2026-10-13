use anyhow::{Context, Result};
#[cfg(feature = "runtime-log-mojo")]
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[cfg(feature = "runtime-log-mojo")]
use super::*;

mod log_line;
#[cfg(feature = "runtime-log-mojo")]
mod request_timeline;
#[cfg(feature = "runtime-log-mojo")]
mod route_profile;
#[cfg(feature = "runtime-log-mojo")]
mod selection;

pub(crate) use log_line::RuntimeDoctorParsedLogLine;
#[cfg(all(test, feature = "runtime-log-mojo"))]
use log_line::runtime_doctor_parse_message_fields;
#[cfg(feature = "runtime-log-mojo")]
use log_line::{runtime_doctor_chain_event_summary, runtime_doctor_truncate_line};
#[cfg(feature = "runtime-log-mojo")]
use request_timeline::{
    RuntimeDoctorRequestTimelineBuilder, runtime_doctor_record_request_timeline_event,
    runtime_doctor_set_latest_request_timeline,
};
#[cfg(feature = "runtime-log-mojo")]
use route_profile::runtime_doctor_record_route_profile_event;
#[cfg(feature = "runtime-log-mojo")]
use selection::runtime_doctor_record_selection_summary;

#[cfg(feature = "runtime-log-mojo")]
fn runtime_doctor_marker_context_summary(
    context: BTreeMap<String, RuntimeDoctorMarkerContextSummary>,
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

#[cfg(feature = "runtime-log-mojo")]
fn runtime_doctor_record_parsed_marker(
    summary: &mut RuntimeDoctorSummary,
    request_timelines: &mut BTreeMap<String, RuntimeDoctorRequestTimelineBuilder>,
    line: (usize, Option<&str>, &str),
    marker: &str,
    fields: &BTreeMap<String, String>,
) {
    let (line_index, line_timestamp, line) = line;
    *summary.marker_counts.entry(marker.to_string()).or_insert(0) += 1;
    summary.last_marker_line = Some(runtime_doctor_truncate_line(line));
    if matches!(
        marker,
        "chain_retried_owner" | "chain_dead_upstream_confirmed" | "stale_continuation"
    ) {
        summary.latest_chain_event = Some(runtime_doctor_chain_event_summary(marker, fields));
    }
    if !fields.is_empty() {
        summary
            .marker_last_fields
            .insert(marker.to_string(), fields.clone());
    }
    runtime_doctor_record_selection_summary(summary, marker, fields);
    runtime_doctor_record_route_profile_event(summary, line_timestamp, marker, fields);
    runtime_doctor_record_request_timeline_event(
        request_timelines,
        line_index,
        line_timestamp,
        marker,
        fields,
    );
}

#[cfg(feature = "runtime-log-mojo")]
struct RuntimeDoctorMarkerAggregationEvent {
    marker: String,
    fields: BTreeMap<String, String>,
}

#[cfg(feature = "runtime-log-mojo")]
fn runtime_doctor_add_aggregated_count(
    counts: &mut BTreeMap<String, usize>,
    key: String,
    amount: usize,
) {
    let entry = counts.entry(key).or_insert(0);
    *entry = entry
        .checked_add(amount)
        .expect("runtime-doctor aggregation counter overflow");
}

#[cfg(feature = "runtime-log-mojo")]
fn runtime_doctor_apply_marker_aggregation(
    summary: &mut RuntimeDoctorSummary,
    context: &mut BTreeMap<String, RuntimeDoctorMarkerContextSummary>,
    batch: &[RuntimeDoctorMarkerAggregationEvent],
) {
    let facet_values = batch
        .iter()
        .map(|event| {
            RUNTIME_DOCTOR_FACETS
                .iter()
                .map(|facet| event.fields.get(*facet).map(String::as_str))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let inputs = batch
        .iter()
        .zip(&facet_values)
        .map(
            |(event, facets)| prodex_mojo_core::rich::RuntimeDoctorMarkerAggregationInput {
                marker: &event.marker,
                route: event.fields.get("route").map(String::as_str),
                lane: event.fields.get("lane").map(String::as_str),
                profile: event.fields.get("profile").map(String::as_str),
                reason: event.fields.get("reason").map(String::as_str),
                transport: event.fields.get("transport").map(String::as_str),
                request_shape: event.fields.get("request_shape").map(String::as_str),
                facets,
            },
        )
        .collect::<Vec<_>>();
    let aggregation = prodex_mojo_core::rich::runtime_doctor_marker_aggregation(&inputs)
        .expect("Mojo runtime-doctor marker aggregation returned invalid output");

    for row in aggregation.rows {
        let event = batch
            .get(row.representative_index)
            .expect("Mojo runtime-doctor aggregation returned an invalid event index");
        let amount = row.count;
        match row.kind {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_MARKER_TOTAL => {
                let entry = context.entry(event.marker.clone()).or_insert_with(|| {
                    RuntimeDoctorMarkerContextSummary {
                        marker: event.marker.clone(),
                        ..RuntimeDoctorMarkerContextSummary::default()
                    }
                });
                entry.total = entry
                    .total
                    .checked_add(amount)
                    .expect("runtime-doctor marker total overflow");
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_ROUTE
            | prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_LANE
            | prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_PROFILE => {
                let key = match row.kind {
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_ROUTE => {
                        "route"
                    }
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_LANE => {
                        "lane"
                    }
                    _ => "profile",
                };
                let value = event
                    .fields
                    .get(key)
                    .expect("Mojo context row lost its source field")
                    .clone();
                let entry = context.entry(event.marker.clone()).or_insert_with(|| {
                    RuntimeDoctorMarkerContextSummary {
                        marker: event.marker.clone(),
                        ..RuntimeDoctorMarkerContextSummary::default()
                    }
                });
                let counts = match row.kind {
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_ROUTE => {
                        &mut entry.routes
                    }
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_LANE => {
                        &mut entry.lanes
                    }
                    _ => &mut entry.profiles,
                };
                runtime_doctor_add_aggregated_count(counts, value, amount);
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_RETRIED_REASON
            | prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_DEAD_REASON
            | prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_STALE_REASON => {
                let reason = event
                    .fields
                    .get("reason")
                    .expect("Mojo reason row lost its source field")
                    .clone();
                let counts = match row.kind {
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_RETRIED_REASON => {
                        &mut summary.chain_retried_owner_by_reason
                    }
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_DEAD_REASON => {
                        &mut summary.chain_dead_upstream_confirmed_by_reason
                    }
                    _ => &mut summary.stale_continuation_by_reason,
                };
                runtime_doctor_add_aggregated_count(counts, reason, amount);
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_ROUTE
            | prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_TRANSPORT
            | prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_REQUEST_SHAPE => {
                let key = match row.kind {
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_ROUTE => {
                        "route"
                    }
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_TRANSPORT => {
                        "transport"
                    }
                    _ => "request_shape",
                };
                let value = event
                    .fields
                    .get(key)
                    .expect("Mojo continuation row lost its source field")
                    .clone();
                let counts = match row.kind {
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_ROUTE => {
                        &mut summary.previous_response_not_found_by_route
                    }
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_TRANSPORT => {
                        &mut summary.previous_response_not_found_by_transport
                    }
                    _ => &mut summary.previous_response_fresh_fallback_blocked_by_request_shape,
                };
                runtime_doctor_add_aggregated_count(counts, value, amount);
            }
            kind if (prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_FACET_BASE
                ..prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_FACET_BASE
                    + prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_FACET_COUNT as i64)
                .contains(&kind) => {
                let facet_index = usize::try_from(
                    kind - prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_FACET_BASE,
                )
                .expect("Mojo facet index must fit usize");
                let facet = RUNTIME_DOCTOR_FACETS
                    .get(facet_index)
                    .expect("Mojo facet index exceeds the Rust facet catalog");
                let value = event
                    .fields
                    .get(*facet)
                    .expect("Mojo facet row lost its source field")
                    .clone();
                runtime_doctor_add_aggregated_count(
                    summary
                        .facet_counts
                        .entry((*facet).to_string())
                        .or_default(),
                    value,
                    amount,
                );
            }
            _ => panic!("Mojo runtime-doctor aggregation returned an unknown row kind"),
        }
    }
    if let Some(index) = aggregation.latest_stale_reason_index {
        let reason = batch
            .get(index)
            .and_then(|event| event.fields.get("reason"))
            .expect("Mojo stale-reason index lost its source field");
        summary.latest_stale_continuation_reason = Some(reason.clone());
    }
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

#[cfg(feature = "runtime-log-mojo")]
pub fn summarize_runtime_log_tail(tail: &[u8]) -> RuntimeDoctorSummary {
    let text = String::from_utf8_lossy(tail);
    let mut summary = RuntimeDoctorSummary::default();
    let mut request_timelines: BTreeMap<String, RuntimeDoctorRequestTimelineBuilder> =
        BTreeMap::new();
    let mut marker_context: BTreeMap<String, RuntimeDoctorMarkerContextSummary> = BTreeMap::new();
    let mut aggregation_batch =
        Vec::with_capacity(prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_BATCH);
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
                (line_index, line_timestamp.as_deref(), line),
                &marker,
                &fields,
            );
            aggregation_batch.push(RuntimeDoctorMarkerAggregationEvent { marker, fields });
            if aggregation_batch.len()
                == prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_BATCH
            {
                runtime_doctor_apply_marker_aggregation(
                    &mut summary,
                    &mut marker_context,
                    &aggregation_batch,
                );
                aggregation_batch.clear();
            }
        }
    }
    runtime_doctor_apply_marker_aggregation(&mut summary, &mut marker_context, &aggregation_batch);
    summary.marker_context_summary = runtime_doctor_marker_context_summary(marker_context);
    runtime_doctor_set_latest_request_timeline(&mut summary, request_timelines);
    diagnosis::runtime_doctor_finalize_log_summary(&mut summary);
    summary
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
#[path = "../tests/src/parsing.rs"]
mod tests;

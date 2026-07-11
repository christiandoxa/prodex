use std::path::Path;

use terminal_ui::FieldRowsBuilder;

use super::*;

mod broker;
mod incident_rows;
mod json;
mod marker_details;
mod summary_tail;

pub use json::{runtime_doctor_json_value, runtime_doctor_json_value_with_policy_suggestions};

use broker::runtime_doctor_runtime_broker_issue_lines;
use incident_rows::runtime_doctor_push_incident_explainer_rows;
use marker_details::runtime_doctor_push_marker_detail_rows;
use summary_tail::runtime_doctor_push_summary_tail_rows;

fn runtime_doctor_push_actionable_route_rows(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
) {
    if !summary.marker_context_summary.is_empty() {
        let contexts = summary
            .marker_context_summary
            .iter()
            .take(5)
            .map(runtime_doctor_marker_context_line)
            .collect::<Vec<_>>()
            .join("; ");
        fields.push("Marker context hotspots", contexts);
    }

    let selection = &summary.selection_summary;
    let total_selection = selection
        .picked
        .saturating_add(selection.kept)
        .saturating_add(selection.skipped)
        .saturating_add(selection.blocked);
    if total_selection > 0 {
        fields.push(
            "Selection decisions",
            format!(
                "picked={} kept={} skipped={} blocked={}",
                selection.picked, selection.kept, selection.skipped, selection.blocked
            ),
        );
        if !selection.selected_profiles.is_empty() {
            fields.push(
                "Selection selected",
                diagnosis::runtime_doctor_count_breakdown(&selection.selected_profiles),
            );
        }
        if !selection.rejection_reasons.is_empty() {
            fields.push(
                "Selection rejection reasons",
                diagnosis::runtime_doctor_count_breakdown(&selection.rejection_reasons),
            );
        }
    }

    if let Some(route) = summary.route_health.first() {
        let score = route
            .health_score
            .map(|score| score.to_string())
            .unwrap_or_else(|| "-".to_string());
        let marker = route.latest_marker.as_deref().unwrap_or("-");
        let reason = route
            .latest_reason
            .as_deref()
            .or(route.health_reason.as_deref())
            .unwrap_or("-");
        fields.push(
            "Route health focus",
            format!(
                "{}/{} events={} health={} latest={} reason={}",
                route.profile, route.route, route.event_count, score, marker, reason
            ),
        );
    }
}

fn runtime_doctor_marker_context_part(
    label: &str,
    counts: &std::collections::BTreeMap<String, usize>,
) -> Option<String> {
    if counts.is_empty() {
        return None;
    }
    Some(format!(
        "{label}:{}",
        diagnosis::runtime_doctor_count_breakdown(counts)
    ))
}

fn runtime_doctor_marker_context_line(entry: &RuntimeDoctorMarkerContextSummary) -> String {
    let mut parts = vec![format!("{} total={}", entry.marker, entry.total)];
    for part in [
        runtime_doctor_marker_context_part("route", &entry.routes),
        runtime_doctor_marker_context_part("lane", &entry.lanes),
        runtime_doctor_marker_context_part("profile", &entry.profiles),
    ]
    .into_iter()
    .flatten()
    {
        parts.push(part);
    }
    parts.join(" ")
}

pub fn runtime_doctor_fields_for_summary(
    summary: &RuntimeDoctorSummary,
    pointer_path: &Path,
) -> Vec<(String, String)> {
    let latest_log = summary
        .log_path
        .as_ref()
        .map(|path| {
            format!(
                "{} ({})",
                path.display(),
                if summary.log_exists {
                    "exists"
                } else {
                    "missing"
                }
            )
        })
        .unwrap_or_else(|| "-".to_string());
    let suspect_continuations = if summary.suspect_continuation_bindings.is_empty() {
        "-".to_string()
    } else {
        format!(
            "count={} bindings={}",
            summary.persisted_suspect_continuations,
            summary.suspect_continuation_bindings.join(", ")
        )
    };
    let broker_issues = runtime_doctor_runtime_broker_issue_lines(summary);
    let mut fields = FieldRowsBuilder::new();
    fields
        .push(
            "Log pointer",
            format!(
                "{} ({})",
                pointer_path.display(),
                if summary.pointer_exists {
                    "exists"
                } else {
                    "missing"
                }
            ),
        )
        .push("Latest log", latest_log)
        .push("Log sample", format!("{} lines", summary.line_count));
    runtime_doctor_push_incident_explainer_rows(&mut fields, summary);
    runtime_doctor_push_actionable_route_rows(&mut fields, summary);
    for (label, marker) in RUNTIME_DOCTOR_COUNT_FIELD_ROWS {
        fields.push(
            *label,
            diagnosis::runtime_doctor_marker_count(summary, marker).to_string(),
        );
        runtime_doctor_push_marker_detail_rows(&mut fields, summary, marker);
    }
    runtime_doctor_push_summary_tail_rows(
        &mut fields,
        summary,
        &broker_issues,
        &suspect_continuations,
    );
    fields.build()
}

#[cfg(test)]
#[path = "../tests/src/render.rs"]
mod tests;

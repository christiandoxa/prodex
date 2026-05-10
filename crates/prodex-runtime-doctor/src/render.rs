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

use std::path::Path;

use super::*;

fn display_path(path: Option<&std::path::PathBuf>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn compact_list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
    }
}

pub fn runtime_doctor_fields_for_summary(
    summary: &RuntimeDoctorSummary,
    pointer_path: &Path,
) -> Vec<(String, String)> {
    vec![
        (
            "Log pointer".to_string(),
            format!(
                "{} ({})",
                pointer_path.display(),
                if summary.pointer_exists {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Latest log".to_string(),
            format!(
                "{} ({})",
                display_path(summary.log_path.as_ref()),
                if summary.log_exists {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Log sample".to_string(),
            format!("{} lines", summary.line_count),
        ),
        ("Diagnosis".to_string(), summary.diagnosis.clone()),
        (
            "Selection pressure".to_string(),
            summary.selection_pressure.clone(),
        ),
        (
            "Transport pressure".to_string(),
            summary.transport_pressure.clone(),
        ),
        (
            "Persistence pressure".to_string(),
            summary.persistence_pressure.clone(),
        ),
        (
            "Quota freshness pressure".to_string(),
            summary.quota_freshness_pressure.clone(),
        ),
        ("Profiles".to_string(), summary.profiles.len().to_string()),
        (
            "Degraded routes".to_string(),
            compact_list(&summary.degraded_routes),
        ),
        (
            "Suspect continuations".to_string(),
            compact_list(&summary.suspect_continuation_bindings),
        ),
        (
            "Runtime brokers".to_string(),
            compact_list(&summary.runtime_broker_identities),
        ),
    ]
}

pub fn runtime_doctor_json_value(summary: &RuntimeDoctorSummary) -> serde_json::Value {
    serde_json::to_value(summary).expect("runtime doctor serialization should always succeed")
}

pub fn runtime_doctor_json_value_with_policy_suggestions(
    summary: &RuntimeDoctorSummary,
    snapshot: RuntimeDoctorTuningSnapshot,
) -> serde_json::Value {
    let mut value = runtime_doctor_json_value(summary);
    if let Some(object) = value.as_object_mut() {
        let suggestions = suggestions::runtime_doctor_policy_suggestions(summary, snapshot);
        object.insert(
            "policy_suggestion_count".to_string(),
            serde_json::Value::from(suggestions.len()),
        );
        object.insert(
            "policy_suggestions".to_string(),
            serde_json::to_value(suggestions)
                .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
        );
    }
    value
}

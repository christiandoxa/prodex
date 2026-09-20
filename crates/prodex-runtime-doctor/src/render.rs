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

fn binding_source_text(source: &RuntimeDoctorBindingSourceSummary) -> String {
    let top_profile = source
        .profiles
        .first()
        .map(|profile| format!("{}={}", profile.profile, profile.total_bindings))
        .unwrap_or_else(|| "-".to_string());
    format!(
        "r={} s={} t={} sid={} total={} profiles={} top={}",
        source.response_bindings,
        source.session_bindings,
        source.turn_state_bindings,
        source.session_id_bindings,
        source.total_bindings,
        source.profile_count,
        top_profile
    )
}

fn binding_state_text(summary: &RuntimeDoctorBindingStateSummary) -> String {
    let missing = summary.state.missing_profile_bindings
        + summary.runtime_continuations.missing_profile_bindings
        + summary.continuation_journal.missing_profile_bindings
        + summary.merged_continuations.missing_profile_bindings;
    format!(
        "active={} profiles={} selected={} | state {} | runtime {} | journal {} | merged {} | missing={}",
        summary.active_profile.as_deref().unwrap_or("-"),
        summary.profile_count,
        summary.last_run_selected_profiles,
        binding_source_text(&summary.state),
        binding_source_text(&summary.runtime_continuations),
        binding_source_text(&summary.continuation_journal),
        binding_source_text(&summary.merged_continuations),
        missing
    )
}

fn marker_field(summary: &RuntimeDoctorSummary, marker: &str, field: &str) -> String {
    summary
        .marker_last_fields
        .get(marker)
        .and_then(|fields| fields.get(field))
        .cloned()
        .unwrap_or_else(|| "-".to_string())
}

pub fn runtime_doctor_fields_for_summary(
    summary: &RuntimeDoctorSummary,
    pointer_path: &Path,
) -> Vec<(String, String)> {
    let mut fields = vec![
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
            "Binding state".to_string(),
            binding_state_text(&summary.binding_state),
        ),
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
    ];

    let legacy_prev =
        diagnosis::runtime_doctor_marker_count(summary, "previous_response_fresh_fallback");
    if legacy_prev > 0 {
        fields.push(("Legacy prev recovery".to_string(), legacy_prev.to_string()));
    }

    let compact_final = diagnosis::runtime_doctor_marker_count(summary, "compact_final_failure");
    if compact_final > 0 {
        fields.push(("Compact final".to_string(), compact_final.to_string()));
        fields.push((
            "Compact exit".to_string(),
            marker_field(summary, "compact_final_failure", "exit"),
        ));
        fields.push((
            "Compact reason".to_string(),
            marker_field(summary, "compact_final_failure", "reason"),
        ));
        fields.push((
            "Compact last fail".to_string(),
            marker_field(summary, "compact_final_failure", "last_failure"),
        ));
    }

    if diagnosis::runtime_doctor_marker_count(summary, "selection_skip_sync_probe") > 0 {
        let deferred = summary
            .marker_last_fields
            .get("selection_skip_sync_probe")
            .and_then(|fields| {
                fields
                    .get("cold_start_jobs")
                    .map(|count| format!("{count} job(s)"))
                    .or_else(|| {
                        fields
                            .get("cold_start_profiles")
                            .map(|count| format!("{count} profile(s)"))
                    })
            })
            .unwrap_or_else(|| "-".to_string());
        fields.push(("Sync-probe deferred".to_string(), deferred));
    }

    if diagnosis::runtime_doctor_marker_count(summary, "profile_probe_refresh_backpressure") > 0 {
        fields.push((
            "Probe pressure profile".to_string(),
            marker_field(summary, "profile_probe_refresh_backpressure", "profile"),
        ));
    }

    if let Some(reason) = diagnosis::runtime_doctor_top_facet(summary, "reason") {
        fields.push(("Hot reason".to_string(), reason));
    }

    fields
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

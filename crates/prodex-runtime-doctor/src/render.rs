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
    let compact_final = diagnosis::runtime_doctor_marker_count(summary, "compact_final_failure");
    let sync_probe = diagnosis::runtime_doctor_marker_count(summary, "selection_skip_sync_probe");
    let probe_pressure =
        diagnosis::runtime_doctor_marker_count(summary, "profile_probe_refresh_backpressure");
    let mut hot_reason = diagnosis::runtime_doctor_top_facet(summary, "reason");
    let sync_fields = summary.marker_last_fields.get("selection_skip_sync_probe");
    let report_plan = prodex_mojo_core::rich::runtime_doctor_report_field_plan(
        legacy_prev > 0,
        compact_final > 0,
        sync_probe > 0,
        probe_pressure > 0,
        hot_reason.is_some(),
        sync_fields.is_some_and(|fields| fields.contains_key("cold_start_jobs")),
        sync_fields.is_some_and(|fields| fields.contains_key("cold_start_profiles")),
    )
    .expect("Mojo runtime-doctor report field plan returned invalid output");
    for field in report_plan.fields[..report_plan.field_count]
        .iter()
        .copied()
    {
        match field {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_FIELD_LEGACY_PREV => {
                fields.push(("Legacy prev recovery".to_string(), legacy_prev.to_string()));
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_FINAL => {
                fields.push(("Compact final".to_string(), compact_final.to_string()));
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_EXIT => {
                fields.push((
                    "Compact exit".to_string(),
                    marker_field(summary, "compact_final_failure", "exit"),
                ));
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_REASON => {
                fields.push((
                    "Compact reason".to_string(),
                    marker_field(summary, "compact_final_failure", "reason"),
                ));
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_LAST_FAILURE => {
                fields.push((
                    "Compact last fail".to_string(),
                    marker_field(summary, "compact_final_failure", "last_failure"),
                ));
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_FIELD_SYNC_DEFERRED => {
                let deferred = match report_plan.sync_source {
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_SYNC_SOURCE_JOBS => sync_fields
                        .and_then(|fields| fields.get("cold_start_jobs"))
                        .map(|count| format!("{count} job(s)"))
                        .unwrap_or_else(|| "-".to_string()),
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_SYNC_SOURCE_PROFILES => {
                        sync_fields
                            .and_then(|fields| fields.get("cold_start_profiles"))
                            .map(|count| format!("{count} profile(s)"))
                            .unwrap_or_else(|| "-".to_string())
                    }
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_SYNC_SOURCE_NONE => {
                        "-".to_string()
                    }
                    _ => unreachable!("validated Mojo sync-probe detail source"),
                };
                fields.push(("Sync-probe deferred".to_string(), deferred));
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_FIELD_PROBE_PRESSURE_PROFILE => {
                fields.push((
                    "Probe pressure profile".to_string(),
                    marker_field(summary, "profile_probe_refresh_backpressure", "profile"),
                ));
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_REPORT_FIELD_HOT_REASON => {
                fields.push((
                    "Hot reason".to_string(),
                    hot_reason
                        .take()
                        .expect("Mojo report plan selected an absent hot reason"),
                ));
            }
            _ => unreachable!("validated Mojo runtime-doctor report field"),
        }
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

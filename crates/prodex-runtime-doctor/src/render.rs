use std::path::Path;

use super::*;

fn marker_field<'a>(
    summary: &'a RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> Option<&'a str> {
    summary
        .marker_last_fields
        .get(marker)?
        .get(field)
        .map(String::as_str)
}

pub fn runtime_doctor_fields_for_summary(
    summary: &RuntimeDoctorSummary,
    pointer_path: &Path,
) -> Vec<(String, String)> {
    let pointer_path = pointer_path.display().to_string();
    let log_path = summary
        .log_path
        .as_ref()
        .map(|path| path.display().to_string());
    let binding = &summary.binding_state;
    let binding_state = prodex_mojo_core::rich::runtime_doctor_binding_display(
        prodex_mojo_core::rich::RuntimeDoctorBindingDisplayInput {
            active_profile: binding.active_profile.as_deref(),
            profile_count: binding.profile_count as u64,
            selected_profiles: binding.last_run_selected_profiles as u64,
            sources: [
                &binding.state,
                &binding.runtime_continuations,
                &binding.continuation_journal,
                &binding.merged_continuations,
            ]
            .map(
                |source| prodex_mojo_core::rich::RuntimeDoctorBindingDisplaySource {
                    response_bindings: source.response_bindings as u64,
                    session_bindings: source.session_bindings as u64,
                    turn_state_bindings: source.turn_state_bindings as u64,
                    session_id_bindings: source.session_id_bindings as u64,
                    total_bindings: source.total_bindings as u64,
                    profile_count: source.profile_count as u64,
                    missing_profile_bindings: source.missing_profile_bindings as u64,
                    top_profile: source
                        .profiles
                        .first()
                        .map(|profile| (profile.profile.as_str(), profile.total_bindings as u64)),
                },
            ),
        },
    )
    .expect("Mojo runtime-doctor binding display failed");
    let hot_reason = diagnosis::runtime_doctor_top_facet(summary, "reason");
    let body = prodex_mojo_core::rich::runtime_doctor_report_fields_json(
        prodex_mojo_core::rich::RuntimeDoctorReportInput {
            pointer_path: &pointer_path,
            pointer_exists: summary.pointer_exists,
            log_path: log_path.as_deref(),
            log_exists: summary.log_exists,
            line_count: summary.line_count as u64,
            diagnosis: &summary.diagnosis,
            selection_pressure: &summary.selection_pressure,
            transport_pressure: &summary.transport_pressure,
            persistence_pressure: &summary.persistence_pressure,
            quota_freshness_pressure: &summary.quota_freshness_pressure,
            profile_count: summary.profiles.len() as u64,
            binding_state: &binding_state,
            degraded_routes: &summary.degraded_routes,
            suspect_continuations: &summary.suspect_continuation_bindings,
            runtime_brokers: &summary.runtime_broker_identities,
            legacy_previous: diagnosis::runtime_doctor_marker_count(
                summary,
                "previous_response_fresh_fallback",
            ) as u64,
            compact_final: diagnosis::runtime_doctor_marker_count(summary, "compact_final_failure")
                as u64,
            sync_probe: diagnosis::runtime_doctor_marker_count(summary, "selection_skip_sync_probe")
                as u64,
            probe_pressure: diagnosis::runtime_doctor_marker_count(
                summary,
                "profile_probe_refresh_backpressure",
            ) as u64,
            compact_exit: marker_field(summary, "compact_final_failure", "exit"),
            compact_reason: marker_field(summary, "compact_final_failure", "reason"),
            compact_last_failure: marker_field(summary, "compact_final_failure", "last_failure"),
            sync_jobs: marker_field(summary, "selection_skip_sync_probe", "cold_start_jobs"),
            sync_profiles: marker_field(
                summary,
                "selection_skip_sync_probe",
                "cold_start_profiles",
            ),
            probe_profile: marker_field(summary, "profile_probe_refresh_backpressure", "profile"),
            hot_reason: hot_reason.as_deref(),
        },
    )
    .expect("Mojo runtime-doctor report renderer failed");
    serde_json::from_str(&body).expect("Mojo runtime-doctor report returned invalid field pairs")
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

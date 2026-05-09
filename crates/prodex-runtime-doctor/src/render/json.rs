use std::collections::BTreeMap;

use crate::{
    RuntimeDoctorBindingStateSummary, RuntimeDoctorIncidentExplanation,
    RuntimeDoctorProfileSummary, RuntimeDoctorRequestTimelineEvent, RuntimeDoctorSummary,
    RuntimeDoctorTuningSnapshot,
};

use super::broker::{
    RuntimeDoctorBrokerArtifactJsonView, runtime_doctor_broker_artifact_json_view,
};
use super::suggestions;

#[derive(serde::Serialize)]
struct RuntimeDoctorJsonView {
    log_path: Option<String>,
    pointer_exists: bool,
    log_exists: bool,
    line_count: usize,
    first_timestamp: Option<String>,
    last_timestamp: Option<String>,
    compat_warning_count: usize,
    top_client_family: Option<String>,
    top_client: Option<String>,
    top_tool_surface: Option<String>,
    top_compat_warning: Option<String>,
    marker_counts: BTreeMap<String, usize>,
    marker_last_fields: BTreeMap<String, BTreeMap<String, String>>,
    facet_counts: BTreeMap<String, BTreeMap<String, usize>>,
    previous_response_not_found_by_route: BTreeMap<String, usize>,
    previous_response_not_found_by_transport: BTreeMap<String, usize>,
    chain_retried_owner_by_reason: BTreeMap<String, usize>,
    chain_dead_upstream_confirmed_by_reason: BTreeMap<String, usize>,
    stale_continuation_by_reason: BTreeMap<String, usize>,
    latest_chain_event: Option<String>,
    latest_stale_continuation_reason: Option<String>,
    latest_request_id: Option<String>,
    latest_request_timeline: Vec<RuntimeDoctorRequestTimelineEvent>,
    last_marker_line: Option<String>,
    selection_pressure: String,
    transport_pressure: String,
    persistence_pressure: String,
    quota_freshness_pressure: String,
    startup_audit_pressure: String,
    persisted_retry_backoffs: usize,
    persisted_transport_backoffs: usize,
    persisted_route_circuits: usize,
    persisted_usage_snapshots: usize,
    persisted_response_bindings: usize,
    persisted_session_bindings: usize,
    persisted_turn_state_bindings: usize,
    persisted_session_id_bindings: usize,
    persisted_verified_continuations: usize,
    persisted_warm_continuations: usize,
    persisted_suspect_continuations: usize,
    persisted_dead_continuations: usize,
    persisted_continuation_journal_response_bindings: usize,
    persisted_continuation_journal_session_bindings: usize,
    persisted_continuation_journal_turn_state_bindings: usize,
    persisted_continuation_journal_session_id_bindings: usize,
    persisted_turn_state_coverage_percent: Option<u8>,
    binding_state: RuntimeDoctorBindingStateSummary,
    state_save_queue_backlog: Option<usize>,
    state_save_lag_ms: Option<u64>,
    continuation_journal_save_backlog: Option<usize>,
    continuation_journal_save_lag_ms: Option<u64>,
    profile_probe_refresh_backlog: Option<usize>,
    profile_probe_refresh_lag_ms: Option<u64>,
    continuation_journal_saved_at: Option<i64>,
    suspect_continuation_bindings: Vec<String>,
    stale_persisted_usage_snapshots: usize,
    recovered_state_file: bool,
    recovered_continuations_file: bool,
    recovered_continuation_journal_file: bool,
    recovered_scores_file: bool,
    recovered_usage_snapshots_file: bool,
    recovered_backoffs_file: bool,
    last_good_backups_present: usize,
    degraded_routes: Vec<String>,
    orphan_managed_dirs: Vec<String>,
    prodex_binary_identities: Vec<String>,
    runtime_broker_identities: Vec<String>,
    runtime_broker_artifacts: Vec<RuntimeDoctorBrokerArtifactJsonView>,
    prodex_binary_mismatch: bool,
    runtime_broker_mismatch: bool,
    failure_class_counts: BTreeMap<String, usize>,
    incident_explainer: Vec<RuntimeDoctorIncidentExplanation>,
    profiles: Vec<RuntimeDoctorProfileSummary>,
    diagnosis: String,
}

impl From<&RuntimeDoctorSummary> for RuntimeDoctorJsonView {
    fn from(summary: &RuntimeDoctorSummary) -> Self {
        Self {
            log_path: summary
                .log_path
                .as_ref()
                .map(|path| path.display().to_string()),
            pointer_exists: summary.pointer_exists,
            log_exists: summary.log_exists,
            line_count: summary.line_count,
            first_timestamp: summary.first_timestamp.clone(),
            last_timestamp: summary.last_timestamp.clone(),
            compat_warning_count: summary.compat_warning_count,
            top_client_family: summary.top_client_family.clone(),
            top_client: summary.top_client.clone(),
            top_tool_surface: summary.top_tool_surface.clone(),
            top_compat_warning: summary.top_compat_warning.clone(),
            marker_counts: summary
                .marker_counts
                .iter()
                .map(|(marker, count)| ((*marker).to_string(), *count))
                .collect(),
            marker_last_fields: summary
                .marker_last_fields
                .iter()
                .map(|(marker, fields)| ((*marker).to_string(), fields.clone()))
                .collect(),
            facet_counts: summary.facet_counts.clone(),
            previous_response_not_found_by_route: summary
                .previous_response_not_found_by_route
                .clone(),
            previous_response_not_found_by_transport: summary
                .previous_response_not_found_by_transport
                .clone(),
            chain_retried_owner_by_reason: summary.chain_retried_owner_by_reason.clone(),
            chain_dead_upstream_confirmed_by_reason: summary
                .chain_dead_upstream_confirmed_by_reason
                .clone(),
            stale_continuation_by_reason: summary.stale_continuation_by_reason.clone(),
            latest_chain_event: summary.latest_chain_event.clone(),
            latest_stale_continuation_reason: summary.latest_stale_continuation_reason.clone(),
            latest_request_id: summary.latest_request_id.clone(),
            latest_request_timeline: summary.latest_request_timeline.clone(),
            last_marker_line: summary.last_marker_line.clone(),
            selection_pressure: summary.selection_pressure.clone(),
            transport_pressure: summary.transport_pressure.clone(),
            persistence_pressure: summary.persistence_pressure.clone(),
            quota_freshness_pressure: summary.quota_freshness_pressure.clone(),
            startup_audit_pressure: summary.startup_audit_pressure.clone(),
            persisted_retry_backoffs: summary.persisted_retry_backoffs,
            persisted_transport_backoffs: summary.persisted_transport_backoffs,
            persisted_route_circuits: summary.persisted_route_circuits,
            persisted_usage_snapshots: summary.persisted_usage_snapshots,
            persisted_response_bindings: summary.persisted_response_bindings,
            persisted_session_bindings: summary.persisted_session_bindings,
            persisted_turn_state_bindings: summary.persisted_turn_state_bindings,
            persisted_session_id_bindings: summary.persisted_session_id_bindings,
            persisted_verified_continuations: summary.persisted_verified_continuations,
            persisted_warm_continuations: summary.persisted_warm_continuations,
            persisted_suspect_continuations: summary.persisted_suspect_continuations,
            persisted_dead_continuations: summary.persisted_dead_continuations,
            persisted_continuation_journal_response_bindings: summary
                .persisted_continuation_journal_response_bindings,
            persisted_continuation_journal_session_bindings: summary
                .persisted_continuation_journal_session_bindings,
            persisted_continuation_journal_turn_state_bindings: summary
                .persisted_continuation_journal_turn_state_bindings,
            persisted_continuation_journal_session_id_bindings: summary
                .persisted_continuation_journal_session_id_bindings,
            persisted_turn_state_coverage_percent: summary.persisted_turn_state_coverage_percent,
            binding_state: summary.binding_state.clone(),
            state_save_queue_backlog: summary.state_save_queue_backlog,
            state_save_lag_ms: summary.state_save_lag_ms,
            continuation_journal_save_backlog: summary.continuation_journal_save_backlog,
            continuation_journal_save_lag_ms: summary.continuation_journal_save_lag_ms,
            profile_probe_refresh_backlog: summary.profile_probe_refresh_backlog,
            profile_probe_refresh_lag_ms: summary.profile_probe_refresh_lag_ms,
            continuation_journal_saved_at: summary.continuation_journal_saved_at,
            suspect_continuation_bindings: summary.suspect_continuation_bindings.clone(),
            stale_persisted_usage_snapshots: summary.stale_persisted_usage_snapshots,
            recovered_state_file: summary.recovered_state_file,
            recovered_continuations_file: summary.recovered_continuations_file,
            recovered_continuation_journal_file: summary.recovered_continuation_journal_file,
            recovered_scores_file: summary.recovered_scores_file,
            recovered_usage_snapshots_file: summary.recovered_usage_snapshots_file,
            recovered_backoffs_file: summary.recovered_backoffs_file,
            last_good_backups_present: summary.last_good_backups_present,
            degraded_routes: summary.degraded_routes.clone(),
            orphan_managed_dirs: summary.orphan_managed_dirs.clone(),
            prodex_binary_identities: summary.prodex_binary_identities.clone(),
            runtime_broker_identities: summary.runtime_broker_identities.clone(),
            runtime_broker_artifacts: summary
                .runtime_broker_identities
                .iter()
                .map(|line| runtime_doctor_broker_artifact_json_view(line))
                .collect(),
            prodex_binary_mismatch: summary.prodex_binary_mismatch,
            runtime_broker_mismatch: summary.runtime_broker_mismatch,
            failure_class_counts: summary.failure_class_counts.clone(),
            incident_explainer: crate::diagnosis::runtime_doctor_incident_explainer(summary),
            profiles: summary.profiles.clone(),
            diagnosis: summary.diagnosis.clone(),
        }
    }
}

pub fn runtime_doctor_json_value(summary: &RuntimeDoctorSummary) -> serde_json::Value {
    serde_json::to_value(RuntimeDoctorJsonView::from(summary))
        .expect("runtime doctor serialization should always succeed")
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
            serde_json::to_value(&suggestions)
                .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
        );
    }
    value
}

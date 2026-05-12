pub mod diagnosis;
mod log_fields;
#[cfg(test)]
#[path = "../tests/src/marker_guard.rs"]
mod marker_guard;
mod markers;
pub mod parsing;
pub mod render;
pub mod smart_context;
pub mod state_summary;
pub mod suggestions;
pub mod tuning;
pub mod types;

pub use log_fields::runtime_proxy_log_fields;
pub use markers::{
    RUNTIME_DOCTOR_ACTIVE_PERSISTENCE_MARKERS, RUNTIME_DOCTOR_ACTIVE_QUOTA_REFRESH_MARKERS,
    RUNTIME_DOCTOR_COUNT_FIELD_ROWS, RUNTIME_DOCTOR_FACETS, RUNTIME_DOCTOR_MARKERS,
    RUNTIME_DOCTOR_PERSISTENCE_PRESSURE_MARKERS, RUNTIME_DOCTOR_SELECTION_PRESSURE_MARKERS,
    RUNTIME_DOCTOR_TRANSPORT_PRESSURE_MARKERS,
};
pub use parsing::{read_runtime_log_tail, summarize_runtime_log_tail};
pub use render::{
    runtime_doctor_fields_for_summary, runtime_doctor_json_value,
    runtime_doctor_json_value_with_policy_suggestions,
};
pub use smart_context::{
    RuntimeDoctorSmartContextAutopilotEvent, RuntimeDoctorSmartContextAutopilotSummary,
    runtime_doctor_estimate_smart_context_tokens_saved,
    runtime_doctor_parse_smart_context_autopilot_line,
    runtime_doctor_summarize_smart_context_autopilot_tail,
};
pub use state_summary::{
    RuntimeDoctorBackoffMaps, RuntimeDoctorBinaryIdentity, RuntimeDoctorBindingSourceInput,
    RuntimeDoctorBindingStateInput, RuntimeDoctorHealthScore, RuntimeDoctorQuotaPressureBand,
    RuntimeDoctorQuotaWindowStatus, RuntimeDoctorRouteKind, RuntimeDoctorStateSummaryConfig,
    RuntimeDoctorUsageSnapshot, runtime_doctor_backoff_maps_from_runtime,
    runtime_doctor_binding_state_summary, runtime_doctor_degraded_routes,
    runtime_doctor_health_scores_from_runtime, runtime_doctor_profile_summaries,
    runtime_doctor_quota_freshness_label, runtime_doctor_quota_window_status_from_runtime,
    runtime_doctor_route_circuit_state, runtime_doctor_route_kind_label,
    runtime_doctor_runtime_broker_mismatch_reason, runtime_doctor_usage_snapshot_from_runtime,
    runtime_doctor_usage_snapshots_from_runtime,
};
pub use suggestions::{
    RuntimeDoctorPolicySettingSuggestion, RuntimeDoctorPolicySuggestion,
    runtime_doctor_policy_suggestion_lines, runtime_doctor_policy_suggestions,
};
pub use tuning::{RuntimeDoctorTuningLaneLimits, RuntimeDoctorTuningSnapshot};
pub use types::{
    RuntimeDoctorBindingProfileSummary, RuntimeDoctorBindingSourceSummary,
    RuntimeDoctorBindingStateSummary, RuntimeDoctorIncidentExplanation,
    RuntimeDoctorProfileSummary, RuntimeDoctorRequestTimelineEvent,
    RuntimeDoctorRouteHealthSummary, RuntimeDoctorRouteProfileEvent, RuntimeDoctorRouteSummary,
    RuntimeDoctorSelectionSummary, RuntimeDoctorSummary,
};

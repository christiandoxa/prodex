use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeDoctorBindingProfileSummary {
    pub profile: String,
    pub response_bindings: usize,
    pub session_bindings: usize,
    pub turn_state_bindings: usize,
    pub session_id_bindings: usize,
    pub total_bindings: usize,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeDoctorBindingSourceSummary {
    pub response_bindings: usize,
    pub session_bindings: usize,
    pub turn_state_bindings: usize,
    pub session_id_bindings: usize,
    pub total_bindings: usize,
    pub profile_count: usize,
    pub profiles: Vec<RuntimeDoctorBindingProfileSummary>,
    pub missing_profile_bindings: usize,
    pub missing_profile_binding_samples: Vec<String>,
    pub oldest_bound_at: Option<i64>,
    pub newest_bound_at: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeDoctorBindingStateSummary {
    pub active_profile: Option<String>,
    pub profile_count: usize,
    pub last_run_selected_profiles: usize,
    pub state: RuntimeDoctorBindingSourceSummary,
    pub runtime_continuations: RuntimeDoctorBindingSourceSummary,
    pub continuation_journal: RuntimeDoctorBindingSourceSummary,
    pub merged_continuations: RuntimeDoctorBindingSourceSummary,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeDoctorSummary {
    pub log_path: Option<PathBuf>,
    pub pointer_exists: bool,
    pub log_exists: bool,
    pub line_count: usize,
    pub marker_counts: BTreeMap<&'static str, usize>,
    pub last_marker_line: Option<String>,
    pub marker_last_fields: BTreeMap<&'static str, BTreeMap<String, String>>,
    pub facet_counts: BTreeMap<String, BTreeMap<String, usize>>,
    pub previous_response_not_found_by_route: BTreeMap<String, usize>,
    pub previous_response_not_found_by_transport: BTreeMap<String, usize>,
    pub previous_response_fresh_fallback_blocked_by_request_shape: BTreeMap<String, usize>,
    pub chain_retried_owner_by_reason: BTreeMap<String, usize>,
    pub chain_dead_upstream_confirmed_by_reason: BTreeMap<String, usize>,
    pub stale_continuation_by_reason: BTreeMap<String, usize>,
    pub latest_chain_event: Option<String>,
    pub latest_stale_continuation_reason: Option<String>,
    pub latest_request_id: Option<String>,
    pub latest_request_timeline: Vec<RuntimeDoctorRequestTimelineEvent>,
    pub first_timestamp: Option<String>,
    pub last_timestamp: Option<String>,
    pub compat_warning_count: usize,
    pub top_client_family: Option<String>,
    pub top_client: Option<String>,
    pub top_tool_surface: Option<String>,
    pub top_compat_warning: Option<String>,
    pub selection_pressure: String,
    pub transport_pressure: String,
    pub persistence_pressure: String,
    pub quota_freshness_pressure: String,
    pub startup_audit_pressure: String,
    pub persisted_retry_backoffs: usize,
    pub persisted_transport_backoffs: usize,
    pub persisted_route_circuits: usize,
    pub persisted_usage_snapshots: usize,
    pub persisted_response_bindings: usize,
    pub persisted_session_bindings: usize,
    pub persisted_turn_state_bindings: usize,
    pub persisted_session_id_bindings: usize,
    pub persisted_verified_continuations: usize,
    pub persisted_warm_continuations: usize,
    pub persisted_suspect_continuations: usize,
    pub persisted_dead_continuations: usize,
    pub persisted_continuation_journal_response_bindings: usize,
    pub persisted_continuation_journal_session_bindings: usize,
    pub persisted_continuation_journal_turn_state_bindings: usize,
    pub persisted_continuation_journal_session_id_bindings: usize,
    pub persisted_turn_state_coverage_percent: Option<u8>,
    pub binding_state: RuntimeDoctorBindingStateSummary,
    pub state_save_queue_backlog: Option<usize>,
    pub state_save_lag_ms: Option<u64>,
    pub continuation_journal_save_backlog: Option<usize>,
    pub continuation_journal_save_lag_ms: Option<u64>,
    pub profile_probe_refresh_backlog: Option<usize>,
    pub profile_probe_refresh_lag_ms: Option<u64>,
    pub continuation_journal_saved_at: Option<i64>,
    pub suspect_continuation_bindings: Vec<String>,
    pub failure_class_counts: BTreeMap<String, usize>,
    pub stale_persisted_usage_snapshots: usize,
    pub recovered_state_file: bool,
    pub recovered_scores_file: bool,
    pub recovered_usage_snapshots_file: bool,
    pub recovered_backoffs_file: bool,
    pub recovered_continuations_file: bool,
    pub recovered_continuation_journal_file: bool,
    pub last_good_backups_present: usize,
    pub degraded_routes: Vec<String>,
    pub orphan_managed_dirs: Vec<String>,
    pub prodex_binary_identities: Vec<String>,
    pub runtime_broker_identities: Vec<String>,
    pub prodex_binary_mismatch: bool,
    pub runtime_broker_mismatch: bool,
    pub profiles: Vec<RuntimeDoctorProfileSummary>,
    pub diagnosis: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RuntimeDoctorIncidentExplanation {
    pub id: String,
    pub cause: String,
    pub evidence: Vec<String>,
    pub markers: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RuntimeDoctorRequestTimelineEvent {
    pub timestamp: Option<String>,
    pub phase: String,
    pub marker: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RuntimeDoctorProfileSummary {
    pub profile: String,
    pub quota_freshness: String,
    pub quota_age_seconds: i64,
    pub retry_backoff_until: Option<i64>,
    pub transport_backoff_until: Option<i64>,
    pub routes: Vec<RuntimeDoctorRouteSummary>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RuntimeDoctorRouteSummary {
    pub route: String,
    pub circuit_state: String,
    pub circuit_until: Option<i64>,
    pub transport_backoff_until: Option<i64>,
    pub health_score: u32,
    pub bad_pairing_score: u32,
    pub performance_score: u32,
    pub quota_band: String,
    pub five_hour_status: String,
    pub weekly_status: String,
}

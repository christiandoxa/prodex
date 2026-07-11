use super::*;

mod broker;
mod final_summary;
mod incident_explainer;
mod marker_accessors;
mod next_steps;
mod route_health;

pub use final_summary::{
    runtime_doctor_append_pointer_note, runtime_doctor_finalize_log_summary,
    runtime_doctor_finalize_summary, runtime_doctor_top_facet,
};
pub use incident_explainer::runtime_doctor_incident_explainer;
pub use marker_accessors::{runtime_doctor_count_breakdown, runtime_doctor_marker_count};
pub use next_steps::{
    runtime_doctor_active_pressure_next_step, runtime_doctor_compact_final_failure_next_step,
    runtime_doctor_lane_pressure_next_step, runtime_doctor_persistence_backpressure_next_step,
    runtime_doctor_precommit_budget_next_step,
    runtime_doctor_previous_response_fail_closed_next_step,
    runtime_doctor_probe_refresh_backpressure_next_step,
    runtime_doctor_profile_auth_recovery_next_step,
    runtime_doctor_profile_inflight_saturated_next_step, runtime_doctor_quota_pressure_next_step,
    runtime_doctor_route_health_next_step, runtime_doctor_sync_probe_skip_next_step,
    runtime_doctor_transport_backoff_next_step,
    runtime_doctor_websocket_connect_overflow_next_step,
};

#[cfg(test)]
#[path = "../tests/src/diagnosis.rs"]
mod tests;

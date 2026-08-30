use super::*;

#[cfg(not(feature = "mojo"))]
mod compatibility;
#[cfg(feature = "mojo")]
mod mojo_render;

#[cfg(feature = "mojo")]
pub fn runtime_doctor_previous_response_fail_closed_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    mojo_render::previous_response(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_previous_response_fail_closed_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    compatibility::runtime_doctor_previous_response_fail_closed_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_compact_final_failure_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::compact_final_failure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_compact_final_failure_next_step(summary: &RuntimeDoctorSummary) -> String {
    compatibility::runtime_doctor_compact_final_failure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_lane_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::lane_pressure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_lane_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    compatibility::runtime_doctor_lane_pressure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_active_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::active_pressure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_active_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    compatibility::runtime_doctor_active_pressure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_profile_inflight_saturated_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    mojo_render::profile_inflight(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_profile_inflight_saturated_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    compatibility::runtime_doctor_profile_inflight_saturated_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_route_health_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::route_health(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_route_health_next_step(summary: &RuntimeDoctorSummary) -> String {
    compatibility::runtime_doctor_route_health_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_websocket_connect_overflow_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    mojo_render::websocket_connect(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_websocket_connect_overflow_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    compatibility::runtime_doctor_websocket_connect_overflow_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_profile_auth_recovery_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::profile_auth(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_profile_auth_recovery_next_step(summary: &RuntimeDoctorSummary) -> String {
    compatibility::runtime_doctor_profile_auth_recovery_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_persistence_backpressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::persistence_backpressure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_persistence_backpressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    compatibility::runtime_doctor_persistence_backpressure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_sync_probe_skip_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::sync_probe_skip(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_sync_probe_skip_next_step(summary: &RuntimeDoctorSummary) -> String {
    compatibility::runtime_doctor_sync_probe_skip_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_probe_refresh_backpressure_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    mojo_render::probe_refresh_backpressure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_probe_refresh_backpressure_next_step(
    summary: &RuntimeDoctorSummary,
) -> String {
    compatibility::runtime_doctor_probe_refresh_backpressure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_transport_backoff_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::transport_backoff(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_transport_backoff_next_step(summary: &RuntimeDoctorSummary) -> String {
    compatibility::runtime_doctor_transport_backoff_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_quota_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::quota_pressure(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_quota_pressure_next_step(summary: &RuntimeDoctorSummary) -> String {
    compatibility::runtime_doctor_quota_pressure_next_step(summary)
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_precommit_budget_next_step(summary: &RuntimeDoctorSummary) -> String {
    mojo_render::precommit_budget(summary)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_precommit_budget_next_step(summary: &RuntimeDoctorSummary) -> String {
    compatibility::runtime_doctor_precommit_budget_next_step(summary)
}

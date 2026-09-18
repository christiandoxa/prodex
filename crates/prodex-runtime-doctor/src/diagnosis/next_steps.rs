use super::*;

#[cfg(any(not(feature = "mojo"), test))]
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

#[cfg(all(test, feature = "mojo"))]
mod mojo_parity_tests {
    use super::*;

    fn assert_matches(summary: &RuntimeDoctorSummary) {
        assert_eq!(
            runtime_doctor_previous_response_fail_closed_next_step(summary),
            compatibility::runtime_doctor_previous_response_fail_closed_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_compact_final_failure_next_step(summary),
            compatibility::runtime_doctor_compact_final_failure_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_lane_pressure_next_step(summary),
            compatibility::runtime_doctor_lane_pressure_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_active_pressure_next_step(summary),
            compatibility::runtime_doctor_active_pressure_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_profile_inflight_saturated_next_step(summary),
            compatibility::runtime_doctor_profile_inflight_saturated_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_route_health_next_step(summary),
            compatibility::runtime_doctor_route_health_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_websocket_connect_overflow_next_step(summary),
            compatibility::runtime_doctor_websocket_connect_overflow_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_profile_auth_recovery_next_step(summary),
            compatibility::runtime_doctor_profile_auth_recovery_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_persistence_backpressure_next_step(summary),
            compatibility::runtime_doctor_persistence_backpressure_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_sync_probe_skip_next_step(summary),
            compatibility::runtime_doctor_sync_probe_skip_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_probe_refresh_backpressure_next_step(summary),
            compatibility::runtime_doctor_probe_refresh_backpressure_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_transport_backoff_next_step(summary),
            compatibility::runtime_doctor_transport_backoff_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_quota_pressure_next_step(summary),
            compatibility::runtime_doctor_quota_pressure_next_step(summary)
        );
        assert_eq!(
            runtime_doctor_precommit_budget_next_step(summary),
            compatibility::runtime_doctor_precommit_budget_next_step(summary)
        );
    }

    #[test]
    fn mojo_next_step_renderers_match_rust_oracle() {
        assert_matches(&RuntimeDoctorSummary::default());

        let mut summary = RuntimeDoctorSummary {
            marker_counts: [
                ("runtime_proxy_lane_limit_reached", 2),
                ("runtime_proxy_active_limit_reached", 3),
                ("profile_inflight_saturated", 4),
                ("profile_health", 5),
                ("websocket_connect_overflow_rejected", 6),
                ("profile_auth_recovery_failed", 7),
                ("state_save_queue_backpressure", 8),
                ("continuation_journal_queue_backpressure", 9),
                ("selection_skip_sync_probe", 10),
                ("profile_probe_refresh_backpressure", 11),
                ("profile_transport_backoff", 12),
                ("quota_blocked", 13),
                ("compact_precommit_budget_exhausted", 14),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
            marker_last_fields: [
                (
                    "runtime_proxy_lane_limit_reached",
                    [
                        ("lane".to_string(), "compact".to_string()),
                        ("active".to_string(), "5".to_string()),
                        ("limit".to_string(), "4".to_string()),
                    ]
                    .into(),
                ),
                (
                    "runtime_proxy_active_limit_reached",
                    [
                        ("active".to_string(), "9".to_string()),
                        ("limit".to_string(), "8".to_string()),
                    ]
                    .into(),
                ),
                (
                    "profile_inflight_saturated",
                    [
                        ("profile".to_string(), "alpha".to_string()),
                        ("hard_limit".to_string(), "3".to_string()),
                    ]
                    .into(),
                ),
                (
                    "profile_health",
                    [
                        ("profile".to_string(), "alpha".to_string()),
                        ("route".to_string(), "responses".to_string()),
                        ("reason".to_string(), "transport".to_string()),
                    ]
                    .into(),
                ),
                (
                    "websocket_connect_overflow_rejected",
                    [
                        ("reason".to_string(), "saturated".to_string()),
                        ("overflow_pending".to_string(), "2".to_string()),
                        ("overflow_max_pending".to_string(), "3".to_string()),
                        ("worker_count".to_string(), "4".to_string()),
                        ("queue_capacity".to_string(), "5".to_string()),
                    ]
                    .into(),
                ),
                (
                    "profile_auth_recovery_failed",
                    [
                        ("profile".to_string(), "alpha".to_string()),
                        ("route".to_string(), "responses".to_string()),
                        ("error".to_string(), "expired".to_string()),
                    ]
                    .into(),
                ),
                (
                    "state_save_queue_backpressure",
                    [("reason".to_string(), "full".to_string())].into(),
                ),
                (
                    "selection_skip_sync_probe",
                    [
                        ("route".to_string(), "responses".to_string()),
                        ("reason".to_string(), "pressure".to_string()),
                        ("cold_start_jobs".to_string(), "2".to_string()),
                    ]
                    .into(),
                ),
                (
                    "profile_probe_refresh_backpressure",
                    [
                        ("profile".to_string(), "alpha".to_string()),
                        ("backlog".to_string(), "7".to_string()),
                    ]
                    .into(),
                ),
                (
                    "profile_transport_backoff",
                    [
                        ("profile".to_string(), "alpha".to_string()),
                        ("route".to_string(), "responses".to_string()),
                    ]
                    .into(),
                ),
                (
                    "quota_blocked",
                    [("profile".to_string(), "alpha".to_string())].into(),
                ),
                (
                    "compact_precommit_budget_exhausted",
                    [("route".to_string(), "compact".to_string())].into(),
                ),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
            facet_counts: [("reason".to_string(), [("transport".to_string(), 4)].into())].into(),
            state_save_queue_backlog: Some(5),
            continuation_journal_save_backlog: Some(6),
            profile_probe_refresh_backlog: Some(7),
            ..RuntimeDoctorSummary::default()
        };
        assert_matches(&summary);

        for (exit, reason) in [
            ("pressure", "unknown"),
            ("candidate_exhausted", "quota"),
            ("candidate_exhausted", "overload"),
            ("candidate_exhausted", "transport"),
            ("candidate_exhausted", "inflight_saturation"),
        ] {
            summary.marker_last_fields.insert(
                "compact_final_failure".to_string(),
                [
                    ("exit".to_string(), exit.to_string()),
                    ("reason".to_string(), reason.to_string()),
                    ("profile".to_string(), "alpha".to_string()),
                ]
                .into(),
            );
            assert_eq!(
                runtime_doctor_compact_final_failure_next_step(&summary),
                compatibility::runtime_doctor_compact_final_failure_next_step(&summary)
            );
        }

        summary.marker_last_fields.insert(
            "state_save_queue_backpressure".to_string(),
            [("reason".to_string(), "-".to_string())].into(),
        );
        assert_matches(&summary);

        let long_lane = "界".repeat(2_800_000);
        summary.marker_last_fields.insert(
            "runtime_proxy_lane_limit_reached".to_string(),
            [("lane".to_string(), long_lane)].into(),
        );
        assert_eq!(
            runtime_doctor_lane_pressure_next_step(&summary),
            compatibility::runtime_doctor_lane_pressure_next_step(&summary)
        );

        let long_profile = "配置".repeat(1_500_000);
        summary.marker_last_fields.insert(
            "profile_auth_recovery_failed".to_string(),
            [
                ("profile".to_string(), long_profile),
                ("route".to_string(), "responses".to_string()),
                ("error".to_string(), "expired".to_string()),
            ]
            .into(),
        );
        assert_eq!(
            runtime_doctor_profile_auth_recovery_next_step(&summary),
            compatibility::runtime_doctor_profile_auth_recovery_next_step(&summary)
        );
    }
}

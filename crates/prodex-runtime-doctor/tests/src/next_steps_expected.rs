use super::*;

#[test]
fn next_steps_have_stable_default_guidance() {
    let summary = RuntimeDoctorSummary::default();
    let cases = [
        (
            runtime_doctor_previous_response_fail_closed_next_step
                as fn(&RuntimeDoctorSummary) -> String,
            "Inspect `previous_response_not_found` and `chain_dead_upstream_confirmed` for the owning context before retrying; fail-closed stale continuation handling blocks fresh replay when continuity is unverified. Start a fresh turn instead of forcing rotation if the owner cannot be recovered. Latest guard: unknown_reason.",
        ),
        (
            runtime_doctor_compact_final_failure_next_step,
            "Inspect compact exit markers around `-` and retry after the blocking condition clears.",
        ),
        (
            runtime_doctor_lane_pressure_next_step,
            "Inspect repeated lane=unknown markers and trim bursty unknown traffic if it is starving responses.",
        ),
        (
            runtime_doctor_active_pressure_next_step,
            "Reduce concurrent fresh work or wait for in-flight requests to drain before retrying.",
        ),
        (
            runtime_doctor_profile_inflight_saturated_next_step,
            "Wait for in-flight work to drain before retrying, or let fresh selection land on another eligible profile.",
        ),
        (
            runtime_doctor_route_health_next_step,
            "Inspect recent transport or overload markers for that route, especially `unknown_reason`, and wait for that route score to decay before expecting fresh selection to reuse it.",
        ),
        (
            runtime_doctor_websocket_connect_overflow_next_step,
            "Overflow queued websocket connect work drained back into the bounded workers; inspect earlier enqueue/reject markers if dispatch repeats. Latest reason: unknown_reason; pending=-/-, workers=-, queue_capacity=-.",
        ),
        (
            runtime_doctor_profile_auth_recovery_next_step,
            "Auth recovered for profile - on route - via - (changed=-); if this repeats, restart active sessions after login refresh.",
        ),
        (
            runtime_doctor_persistence_backpressure_next_step,
            "Reduce rapid rotation or continuation churn and wait for background persistence queues to drain.",
        ),
        (
            runtime_doctor_sync_probe_skip_next_step,
            "Inspect `selection_skip_sync_probe`, `profile_probe_refresh_backpressure`, and `profile_probe_refresh_queued` markers for route unknown; pressure mode (unknown_reason) deferred cold-start work, so cold-start profiles may stay on stale quota data until background probes finish.",
        ),
        (
            runtime_doctor_probe_refresh_backpressure_next_step,
            "Let the background quota-refresh queue drain before expecting cold-start profiles to become selectable again.",
        ),
        (
            runtime_doctor_transport_backoff_next_step,
            "Inspect network/proxy and upstream transport markers for affected route; wait for short transport backoff to expire before retrying fresh work. Top reason: inspect latest transport marker.",
        ),
        (
            runtime_doctor_quota_pressure_next_step,
            "Wait for quota reset or use another eligible profile; verify current limits with `prodex quota --all --once`.",
        ),
        (
            runtime_doctor_precommit_budget_next_step,
            "Inspect selection skip, quota, and transport backoff markers for affected route; retry after an eligible profile becomes available.",
        ),
    ];
    for (render, expected) in cases {
        assert_eq!(render(&summary), expected);
    }
}

#[test]
fn next_steps_preserve_long_unicode_fields() {
    let mut summary = RuntimeDoctorSummary::default();
    let long_lane = "界".repeat(2_800_000);
    summary.marker_last_fields.insert(
        "runtime_proxy_lane_limit_reached".to_string(),
        [("lane".to_string(), long_lane.clone())].into(),
    );
    assert!(runtime_doctor_lane_pressure_next_step(&summary).contains(&long_lane));

    let long_profile = "配置".repeat(1_500_000);
    summary.marker_last_fields.insert(
        "profile_auth_recovery_failed".to_string(),
        [("profile".to_string(), long_profile.clone())].into(),
    );
    summary
        .marker_counts
        .insert("profile_auth_recovery_failed".to_string(), 1);
    assert!(runtime_doctor_profile_auth_recovery_next_step(&summary).contains(&long_profile));
}

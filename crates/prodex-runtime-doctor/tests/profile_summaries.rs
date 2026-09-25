#![cfg(feature = "state-summary-mojo")]

use std::collections::BTreeMap;

use prodex_runtime_doctor::{
    RuntimeDoctorBackoffMaps, RuntimeDoctorHealthScore, RuntimeDoctorQuotaWindowStatus,
    RuntimeDoctorStateSummaryConfig, RuntimeDoctorUsageSnapshot, runtime_doctor_profile_summaries,
};

#[test]
fn public_profile_summaries_preserve_order_and_runtime_state() {
    let profile_names = vec!["beta".to_string(), "alpha".to_string()];
    let usage = BTreeMap::from([
        (
            "alpha".to_string(),
            RuntimeDoctorUsageSnapshot {
                checked_at: 900,
                five_hour_status: RuntimeDoctorQuotaWindowStatus::Thin,
                five_hour_remaining_percent: 12,
                five_hour_reset_at: i64::MAX,
                weekly_status: RuntimeDoctorQuotaWindowStatus::Ready,
                weekly_remaining_percent: 90,
                weekly_reset_at: i64::MAX,
            },
        ),
        (
            "beta".to_string(),
            RuntimeDoctorUsageSnapshot {
                checked_at: 600,
                five_hour_status: RuntimeDoctorQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: 999,
                weekly_status: RuntimeDoctorQuotaWindowStatus::Critical,
                weekly_remaining_percent: 20,
                weekly_reset_at: i64::MAX,
            },
        ),
    ]);
    let scores = BTreeMap::from([
        (
            "__route_health__:responses:alpha".to_string(),
            RuntimeDoctorHealthScore {
                score: 6,
                updated_at: 980,
            },
        ),
        (
            "__route_bad_pairing__:responses:alpha".to_string(),
            RuntimeDoctorHealthScore {
                score: 4,
                updated_at: 995,
            },
        ),
        (
            "__route_performance__:responses:alpha".to_string(),
            RuntimeDoctorHealthScore {
                score: 7,
                updated_at: 960,
            },
        ),
    ]);
    let retry = BTreeMap::from([("alpha".to_string(), 1_020), ("beta".to_string(), 900)]);
    let transport = BTreeMap::from([
        ("alpha".to_string(), 1_030),
        (
            "__route_transport_backoff__:responses:alpha".to_string(),
            1_040,
        ),
        (
            "__route_transport_backoff__:websocket:alpha".to_string(),
            1_050,
        ),
        (
            "__route_transport_backoff__:compact:alpha".to_string(),
            1_000,
        ),
        ("beta".to_string(), 950),
    ]);
    let circuits = BTreeMap::from([
        ("__route_circuit__:responses:alpha".to_string(), 1_050),
        ("__route_circuit__:websocket:alpha".to_string(), 1_000),
        ("__route_circuit__:compact:alpha".to_string(), 999),
    ]);
    let summaries = runtime_doctor_profile_summaries(
        &profile_names,
        &usage,
        &scores,
        RuntimeDoctorBackoffMaps {
            retry_backoff_until: &retry,
            transport_backoff_until: &transport,
            route_circuit_open_until: &circuits,
        },
        1_000,
        RuntimeDoctorStateSummaryConfig {
            health_decay_seconds: 10,
            bad_pairing_decay_seconds: 5,
            performance_decay_seconds: 20,
            usage_snapshot_stale_grace_seconds: 300,
        },
    );

    let observed = summaries
        .iter()
        .map(|profile| {
            (
                profile.profile.as_str(),
                profile.quota_freshness.as_str(),
                profile.quota_age_seconds,
                profile.retry_backoff_until,
                profile.transport_backoff_until,
                profile
                    .routes
                    .iter()
                    .map(|route| {
                        (
                            route.route.as_str(),
                            route.circuit_state.as_str(),
                            route.circuit_until,
                            route.transport_backoff_until,
                            route.health_score,
                            route.bad_pairing_score,
                            route.performance_score,
                            route.quota_band.as_str(),
                            route.five_hour_status.as_str(),
                            route.weekly_status.as_str(),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        observed,
        vec![
            (
                "beta",
                "stale",
                400,
                Some(900),
                None,
                vec![
                    (
                        "responses",
                        "closed",
                        None,
                        None,
                        0,
                        0,
                        0,
                        "quota_critical",
                        "ready",
                        "critical",
                    ),
                    (
                        "websocket",
                        "closed",
                        None,
                        None,
                        0,
                        0,
                        0,
                        "quota_critical",
                        "ready",
                        "critical",
                    ),
                    (
                        "compact",
                        "closed",
                        None,
                        None,
                        0,
                        0,
                        0,
                        "quota_critical",
                        "ready",
                        "critical",
                    ),
                    (
                        "standard",
                        "closed",
                        None,
                        None,
                        0,
                        0,
                        0,
                        "quota_critical",
                        "ready",
                        "critical",
                    ),
                ],
            ),
            (
                "alpha",
                "fresh",
                100,
                Some(1_020),
                Some(1_050),
                vec![
                    (
                        "responses",
                        "open",
                        Some(1_050),
                        Some(1_040),
                        4,
                        3,
                        5,
                        "quota_thin",
                        "thin",
                        "ready",
                    ),
                    (
                        "websocket",
                        "half_open",
                        Some(1_000),
                        Some(1_050),
                        0,
                        0,
                        0,
                        "quota_thin",
                        "thin",
                        "ready",
                    ),
                    (
                        "compact",
                        "half_open",
                        Some(999),
                        Some(1_030),
                        0,
                        0,
                        0,
                        "quota_thin",
                        "thin",
                        "ready",
                    ),
                    (
                        "standard",
                        "closed",
                        None,
                        Some(1_030),
                        0,
                        0,
                        0,
                        "quota_thin",
                        "thin",
                        "ready",
                    ),
                ],
            ),
        ]
    );
}

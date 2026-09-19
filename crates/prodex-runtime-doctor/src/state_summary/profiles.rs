use std::collections::BTreeMap;

use crate::{RuntimeDoctorProfileSummary, RuntimeDoctorRouteSummary};

#[cfg(any(not(feature = "mojo"), test))]
use super::quota::{
    runtime_doctor_quota_freshness_label, runtime_doctor_quota_summary_from_usage_snapshot_at,
    runtime_doctor_unknown_quota_summary,
};
#[cfg(feature = "mojo")]
use super::quota::{
    runtime_doctor_quota_pressure_band_from_code, runtime_doctor_quota_route_code,
    runtime_doctor_quota_status_code, runtime_doctor_quota_status_from_code,
};
use super::quota::{
    runtime_doctor_quota_pressure_band_reason, runtime_doctor_quota_window_status_reason,
};
#[cfg(any(not(feature = "mojo"), test))]
use super::routes::{
    runtime_doctor_effective_health_score_from_map, runtime_doctor_effective_score_from_map,
};
use super::routes::{
    runtime_doctor_route_bad_pairing_key, runtime_doctor_route_circuit_key,
    runtime_doctor_route_health_key, runtime_doctor_route_kind_label,
    runtime_doctor_route_performance_key, runtime_doctor_transport_backoff_key,
    runtime_doctor_transport_backoff_profile_name,
};
use super::{
    RuntimeDoctorBackoffMaps, RuntimeDoctorHealthScore, RuntimeDoctorRouteKind,
    RuntimeDoctorStateSummaryConfig, RuntimeDoctorUsageSnapshot,
};

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_route_circuit_state(until: Option<i64>, now: i64) -> &'static str {
    match until {
        Some(until) if until > now => "open",
        Some(_) => "half_open",
        None => "closed",
    }
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_route_circuit_state(until: Option<i64>, now: i64) -> &'static str {
    match prodex_mojo_core::rich::runtime_doctor_state_plan(
        prodex_mojo_core::rich::RuntimeDoctorStatePlanInput {
            operation: prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_OP_CIRCUIT,
            now,
            circuit_until: until.unwrap_or(-1),
            ..prodex_mojo_core::rich::RuntimeDoctorStatePlanInput::default()
        },
    )
    .expect("Mojo runtime-doctor circuit plan returned invalid output")
    .circuit_state
    {
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_CIRCUIT_OPEN => "open",
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_CIRCUIT_HALF_OPEN => "half_open",
        _ => "closed",
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_doctor_transport_backoff_until_from_map(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
    now: i64,
) -> Option<i64> {
    let route_key = runtime_doctor_transport_backoff_key(profile_name, route_kind);
    [
        transport_backoff_until.get(&route_key).copied(),
        transport_backoff_until.get(profile_name).copied(),
    ]
    .into_iter()
    .flatten()
    .filter(|until| *until > now)
    .max()
}

fn runtime_doctor_transport_backoff_max_until(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    now: i64,
) -> Option<i64> {
    transport_backoff_until
        .iter()
        .filter(|(key, until)| {
            runtime_doctor_transport_backoff_profile_name(key) == profile_name && **until > now
        })
        .map(|(_, until)| *until)
        .max()
}

#[cfg(feature = "mojo")]
fn runtime_doctor_route_plan_for_profile(
    profile_name: &str,
    snapshot: Option<&RuntimeDoctorUsageSnapshot>,
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
    route_kind: RuntimeDoctorRouteKind,
) -> prodex_mojo_core::rich::RuntimeDoctorRoutePlan {
    let health = scores
        .get(&runtime_doctor_route_health_key(profile_name, route_kind))
        .copied()
        .unwrap_or_default();
    let bad_pairing = scores
        .get(&runtime_doctor_route_bad_pairing_key(
            profile_name,
            route_kind,
        ))
        .copied()
        .unwrap_or_default();
    let performance = scores
        .get(&runtime_doctor_route_performance_key(
            profile_name,
            route_kind,
        ))
        .copied()
        .unwrap_or_default();
    let circuit_key = runtime_doctor_route_circuit_key(profile_name, route_kind);
    let route_transport_key = runtime_doctor_transport_backoff_key(profile_name, route_kind);
    let (
        snapshot_present,
        checked_at,
        five_hour_status,
        five_hour_reset_at,
        weekly_status,
        weekly_reset_at,
    ) = snapshot.map_or(
        (
            0,
            0,
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_UNKNOWN,
            i64::MAX,
            prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_STATUS_UNKNOWN,
            i64::MAX,
        ),
        |snapshot| {
            (
                1,
                snapshot.checked_at,
                runtime_doctor_quota_status_code(snapshot.five_hour_status),
                snapshot.five_hour_reset_at,
                runtime_doctor_quota_status_code(snapshot.weekly_status),
                snapshot.weekly_reset_at,
            )
        },
    );
    prodex_mojo_core::rich::runtime_doctor_route_plan(
        prodex_mojo_core::rich::RuntimeDoctorRoutePlanInput {
            route_kind: runtime_doctor_quota_route_code(route_kind),
            now,
            snapshot_present,
            checked_at,
            five_hour_status,
            five_hour_reset_at,
            weekly_status,
            weekly_reset_at,
            stale_grace_seconds: config.usage_snapshot_stale_grace_seconds,
            health_score: i64::from(health.score),
            health_updated_at: health.updated_at,
            health_decay_seconds: config.health_decay_seconds,
            bad_pairing_score: i64::from(bad_pairing.score),
            bad_pairing_updated_at: bad_pairing.updated_at,
            bad_pairing_decay_seconds: config.bad_pairing_decay_seconds,
            performance_score: i64::from(performance.score),
            performance_updated_at: performance.updated_at,
            performance_decay_seconds: config.performance_decay_seconds,
            circuit_until: backoffs
                .route_circuit_open_until
                .get(&circuit_key)
                .copied()
                .unwrap_or(-1),
            route_transport_until: backoffs
                .transport_backoff_until
                .get(&route_transport_key)
                .copied()
                .unwrap_or(i64::MIN),
            profile_transport_until: backoffs
                .transport_backoff_until
                .get(profile_name)
                .copied()
                .unwrap_or(i64::MIN),
        },
    )
    .expect("Mojo runtime-doctor route plan returned invalid output")
}

#[cfg(feature = "mojo")]
fn runtime_doctor_route_circuit_state_from_code(value: i64) -> &'static str {
    match value {
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_CIRCUIT_OPEN => "open",
        prodex_mojo_core::rich::RUNTIME_DOCTOR_STATE_CIRCUIT_HALF_OPEN => "half_open",
        _ => "closed",
    }
}

#[cfg(feature = "mojo")]
pub fn runtime_doctor_profile_summaries(
    profile_names: &[String],
    usage_snapshots: &BTreeMap<String, RuntimeDoctorUsageSnapshot>,
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> Vec<RuntimeDoctorProfileSummary> {
    let route_kinds = [
        RuntimeDoctorRouteKind::Responses,
        RuntimeDoctorRouteKind::Websocket,
        RuntimeDoctorRouteKind::Compact,
        RuntimeDoctorRouteKind::Standard,
    ];
    profile_names
        .iter()
        .map(|profile_name| {
            let snapshot = usage_snapshots.get(profile_name);
            let planned_routes = route_kinds
                .iter()
                .copied()
                .map(|route_kind| {
                    (
                        route_kind,
                        runtime_doctor_route_plan_for_profile(
                            profile_name,
                            snapshot,
                            scores,
                            backoffs,
                            now,
                            config,
                            route_kind,
                        ),
                    )
                })
                .collect::<Vec<_>>();
            let freshness = planned_routes
                .first()
                .map(|(_, plan)| plan.freshness)
                .unwrap_or(prodex_mojo_core::rich::RUNTIME_DOCTOR_ROUTE_FRESHNESS_MISSING);
            let quota_age_seconds = planned_routes
                .first()
                .map(|(_, plan)| plan.quota_age_seconds)
                .unwrap_or(i64::MAX);
            let routes = planned_routes
                .into_iter()
                .map(|(route_kind, plan)| {
                    let circuit_key = runtime_doctor_route_circuit_key(profile_name, route_kind);
                    RuntimeDoctorRouteSummary {
                        route: runtime_doctor_route_kind_label(route_kind).to_string(),
                        circuit_state: runtime_doctor_route_circuit_state_from_code(
                            plan.circuit_state,
                        )
                        .to_string(),
                        circuit_until: backoffs.route_circuit_open_until.get(&circuit_key).copied(),
                        transport_backoff_until: (plan.transport_backoff_present == 1)
                            .then_some(plan.transport_backoff_until),
                        health_score: u32::try_from(plan.health_score)
                            .expect("validated Mojo health score fits u32"),
                        bad_pairing_score: u32::try_from(plan.bad_pairing_score)
                            .expect("validated Mojo bad-pairing score fits u32"),
                        performance_score: u32::try_from(plan.performance_score)
                            .expect("validated Mojo performance score fits u32"),
                        quota_band: runtime_doctor_quota_pressure_band_reason(
                            runtime_doctor_quota_pressure_band_from_code(plan.route_band),
                        )
                        .to_string(),
                        five_hour_status: runtime_doctor_quota_window_status_reason(
                            runtime_doctor_quota_status_from_code(plan.five_hour_status),
                        )
                        .to_string(),
                        weekly_status: runtime_doctor_quota_window_status_reason(
                            runtime_doctor_quota_status_from_code(plan.weekly_status),
                        )
                        .to_string(),
                    }
                })
                .collect();
            RuntimeDoctorProfileSummary {
                profile: profile_name.clone(),
                quota_freshness: match freshness {
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_ROUTE_FRESHNESS_FRESH => "fresh",
                    prodex_mojo_core::rich::RUNTIME_DOCTOR_ROUTE_FRESHNESS_STALE => "stale",
                    _ => "missing",
                }
                .to_string(),
                quota_age_seconds,
                retry_backoff_until: backoffs.retry_backoff_until.get(profile_name).copied(),
                transport_backoff_until: runtime_doctor_transport_backoff_max_until(
                    backoffs.transport_backoff_until,
                    profile_name,
                    now,
                ),
                routes,
            }
        })
        .collect()
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_profile_summaries(
    profile_names: &[String],
    usage_snapshots: &BTreeMap<String, RuntimeDoctorUsageSnapshot>,
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> Vec<RuntimeDoctorProfileSummary> {
    runtime_doctor_profile_summaries_rust(
        profile_names,
        usage_snapshots,
        scores,
        backoffs,
        now,
        config,
    )
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_doctor_profile_summaries_rust(
    profile_names: &[String],
    usage_snapshots: &BTreeMap<String, RuntimeDoctorUsageSnapshot>,
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> Vec<RuntimeDoctorProfileSummary> {
    let mut profiles = Vec::new();
    for profile_name in profile_names {
        let snapshot = usage_snapshots.get(profile_name);
        let quota_age_seconds = snapshot
            .map(|snapshot| now.saturating_sub(snapshot.checked_at))
            .unwrap_or(i64::MAX);
        let routes = [
            RuntimeDoctorRouteKind::Responses,
            RuntimeDoctorRouteKind::Websocket,
            RuntimeDoctorRouteKind::Compact,
            RuntimeDoctorRouteKind::Standard,
        ]
        .into_iter()
        .map(|route_kind| {
            let quota_summary = snapshot
                .map(|snapshot| {
                    runtime_doctor_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now)
                })
                .unwrap_or_else(runtime_doctor_unknown_quota_summary);
            let circuit_key = runtime_doctor_route_circuit_key(profile_name, route_kind);
            RuntimeDoctorRouteSummary {
                route: runtime_doctor_route_kind_label(route_kind).to_string(),
                circuit_state: runtime_doctor_route_circuit_state(
                    backoffs.route_circuit_open_until.get(&circuit_key).copied(),
                    now,
                )
                .to_string(),
                circuit_until: backoffs.route_circuit_open_until.get(&circuit_key).copied(),
                transport_backoff_until: runtime_doctor_transport_backoff_until_from_map(
                    backoffs.transport_backoff_until,
                    profile_name,
                    route_kind,
                    now,
                ),
                health_score: runtime_doctor_effective_health_score_from_map(
                    scores,
                    &runtime_doctor_route_health_key(profile_name, route_kind),
                    now,
                    config,
                ),
                bad_pairing_score: runtime_doctor_effective_score_from_map(
                    scores,
                    &runtime_doctor_route_bad_pairing_key(profile_name, route_kind),
                    now,
                    config.bad_pairing_decay_seconds,
                ),
                performance_score: runtime_doctor_effective_score_from_map(
                    scores,
                    &runtime_doctor_route_performance_key(profile_name, route_kind),
                    now,
                    config.performance_decay_seconds,
                ),
                quota_band: runtime_doctor_quota_pressure_band_reason(quota_summary.route_band)
                    .to_string(),
                five_hour_status: runtime_doctor_quota_window_status_reason(
                    quota_summary.five_hour.status,
                )
                .to_string(),
                weekly_status: runtime_doctor_quota_window_status_reason(
                    quota_summary.weekly.status,
                )
                .to_string(),
            }
        })
        .collect::<Vec<_>>();
        profiles.push(RuntimeDoctorProfileSummary {
            profile: profile_name.clone(),
            quota_freshness: runtime_doctor_quota_freshness_label(
                snapshot,
                now,
                config.usage_snapshot_stale_grace_seconds,
            )
            .to_string(),
            quota_age_seconds,
            retry_backoff_until: backoffs.retry_backoff_until.get(profile_name).copied(),
            transport_backoff_until: runtime_doctor_transport_backoff_max_until(
                backoffs.transport_backoff_until,
                profile_name,
                now,
            ),
            routes,
        });
    }
    profiles
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_route_plan_parity_tests {
    use super::*;

    #[test]
    fn profile_summary_route_plan_matches_rust_oracle() {
        let profiles = vec!["alpha".to_string()];
        let usage = BTreeMap::from([(
            "alpha".to_string(),
            RuntimeDoctorUsageSnapshot {
                checked_at: 90,
                five_hour_status: super::super::RuntimeDoctorQuotaWindowStatus::Thin,
                five_hour_remaining_percent: 20,
                five_hour_reset_at: 200,
                weekly_status: super::super::RuntimeDoctorQuotaWindowStatus::Critical,
                weekly_remaining_percent: 8,
                weekly_reset_at: 300,
            },
        )]);
        let mut scores = BTreeMap::new();
        for route in [
            RuntimeDoctorRouteKind::Responses,
            RuntimeDoctorRouteKind::Websocket,
            RuntimeDoctorRouteKind::Compact,
            RuntimeDoctorRouteKind::Standard,
        ] {
            scores.insert(
                runtime_doctor_route_health_key("alpha", route),
                RuntimeDoctorHealthScore {
                    score: 7,
                    updated_at: 95,
                },
            );
            scores.insert(
                runtime_doctor_route_bad_pairing_key("alpha", route),
                RuntimeDoctorHealthScore {
                    score: 4,
                    updated_at: 94,
                },
            );
            scores.insert(
                runtime_doctor_route_performance_key("alpha", route),
                RuntimeDoctorHealthScore {
                    score: 9,
                    updated_at: 93,
                },
            );
        }
        let retry = BTreeMap::from([("alpha".to_string(), 150)]);
        let transport = BTreeMap::from([
            ("alpha".to_string(), 120),
            (
                runtime_doctor_transport_backoff_key("alpha", RuntimeDoctorRouteKind::Responses),
                130,
            ),
        ]);
        let circuits = BTreeMap::from([(
            runtime_doctor_route_circuit_key("alpha", RuntimeDoctorRouteKind::Responses),
            140,
        )]);
        let backoffs = RuntimeDoctorBackoffMaps {
            retry_backoff_until: &retry,
            transport_backoff_until: &transport,
            route_circuit_open_until: &circuits,
        };
        let config = RuntimeDoctorStateSummaryConfig {
            health_decay_seconds: 60,
            bad_pairing_decay_seconds: 180,
            performance_decay_seconds: 300,
            usage_snapshot_stale_grace_seconds: 300,
        };
        assert_eq!(
            serde_json::to_value(runtime_doctor_profile_summaries(
                &profiles, &usage, &scores, backoffs, 100, config
            ))
            .unwrap(),
            serde_json::to_value(runtime_doctor_profile_summaries_rust(
                &profiles, &usage, &scores, backoffs, 100, config
            ))
            .unwrap(),
        );
    }
}

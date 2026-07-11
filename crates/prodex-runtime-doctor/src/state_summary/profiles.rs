use std::collections::BTreeMap;

use crate::{RuntimeDoctorProfileSummary, RuntimeDoctorRouteSummary};

use super::quota::{
    runtime_doctor_quota_freshness_label, runtime_doctor_quota_pressure_band_reason,
    runtime_doctor_quota_summary_from_usage_snapshot_at, runtime_doctor_quota_window_status_reason,
    runtime_doctor_unknown_quota_summary,
};
use super::routes::{
    runtime_doctor_effective_health_score_from_map, runtime_doctor_effective_score_from_map,
    runtime_doctor_route_bad_pairing_key, runtime_doctor_route_circuit_key,
    runtime_doctor_route_health_key, runtime_doctor_route_kind_label,
    runtime_doctor_route_performance_key, runtime_doctor_transport_backoff_key,
    runtime_doctor_transport_backoff_profile_name,
};
use super::{
    RuntimeDoctorBackoffMaps, RuntimeDoctorHealthScore, RuntimeDoctorRouteKind,
    RuntimeDoctorStateSummaryConfig, RuntimeDoctorUsageSnapshot,
};

pub fn runtime_doctor_route_circuit_state(until: Option<i64>, now: i64) -> &'static str {
    match until {
        Some(until) if until > now => "open",
        Some(_) => "half_open",
        None => "closed",
    }
}

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

pub fn runtime_doctor_profile_summaries(
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

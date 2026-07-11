use std::collections::BTreeMap;

use super::{
    RuntimeDoctorBackoffMaps, RuntimeDoctorHealthScore, RuntimeDoctorQuotaWindowStatus,
    RuntimeDoctorUsageSnapshot,
};

pub fn runtime_doctor_backoff_maps_from_runtime(
    backoffs: &prodex_runtime_state::RuntimeProfileBackoffs,
) -> RuntimeDoctorBackoffMaps<'_> {
    RuntimeDoctorBackoffMaps {
        retry_backoff_until: &backoffs.retry_backoff_until,
        transport_backoff_until: &backoffs.transport_backoff_until,
        route_circuit_open_until: &backoffs.route_circuit_open_until,
    }
}

pub fn runtime_doctor_health_scores_from_runtime(
    scores: &BTreeMap<String, prodex_runtime_state::RuntimeProfileHealth>,
) -> BTreeMap<String, RuntimeDoctorHealthScore> {
    scores
        .iter()
        .map(|(key, health)| {
            (
                key.clone(),
                RuntimeDoctorHealthScore {
                    score: health.score,
                    updated_at: health.updated_at,
                },
            )
        })
        .collect()
}

pub fn runtime_doctor_quota_window_status_from_runtime(
    status: prodex_quota::RuntimeQuotaWindowStatus,
) -> RuntimeDoctorQuotaWindowStatus {
    match status {
        prodex_quota::RuntimeQuotaWindowStatus::Ready => RuntimeDoctorQuotaWindowStatus::Ready,
        prodex_quota::RuntimeQuotaWindowStatus::Thin => RuntimeDoctorQuotaWindowStatus::Thin,
        prodex_quota::RuntimeQuotaWindowStatus::Critical => {
            RuntimeDoctorQuotaWindowStatus::Critical
        }
        prodex_quota::RuntimeQuotaWindowStatus::Exhausted => {
            RuntimeDoctorQuotaWindowStatus::Exhausted
        }
        prodex_quota::RuntimeQuotaWindowStatus::Unknown => RuntimeDoctorQuotaWindowStatus::Unknown,
    }
}

pub fn runtime_doctor_usage_snapshot_from_runtime(
    snapshot: &prodex_runtime_state::RuntimeProfileUsageSnapshot<
        prodex_quota::RuntimeQuotaWindowStatus,
    >,
) -> RuntimeDoctorUsageSnapshot {
    RuntimeDoctorUsageSnapshot {
        checked_at: snapshot.checked_at,
        five_hour_status: runtime_doctor_quota_window_status_from_runtime(
            snapshot.five_hour_status,
        ),
        five_hour_remaining_percent: snapshot.five_hour_remaining_percent,
        five_hour_reset_at: snapshot.five_hour_reset_at,
        weekly_status: runtime_doctor_quota_window_status_from_runtime(snapshot.weekly_status),
        weekly_remaining_percent: snapshot.weekly_remaining_percent,
        weekly_reset_at: snapshot.weekly_reset_at,
    }
}

pub fn runtime_doctor_usage_snapshots_from_runtime(
    usage_snapshots: &BTreeMap<
        String,
        prodex_runtime_state::RuntimeProfileUsageSnapshot<prodex_quota::RuntimeQuotaWindowStatus>,
    >,
) -> BTreeMap<String, RuntimeDoctorUsageSnapshot> {
    usage_snapshots
        .iter()
        .map(|(profile_name, snapshot)| {
            (
                profile_name.clone(),
                runtime_doctor_usage_snapshot_from_runtime(snapshot),
            )
        })
        .collect()
}

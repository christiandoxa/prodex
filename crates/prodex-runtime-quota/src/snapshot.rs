use crate::route::runtime_route_kind_to_proxy;
use crate::summary::{runtime_quota_precommit_guard_reason, runtime_quota_summary_from_proxy};
use crate::window::{
    runtime_quota_window_observation, runtime_quota_window_status_from_proxy,
    runtime_quota_window_status_to_proxy,
};
use chrono::Local;
use prodex_quota::{RuntimeQuotaSummary, RuntimeQuotaWindowStatus, UsageResponse};
use prodex_runtime_state::{
    RuntimeProfileUsageSnapshot as RuntimeProfileUsageSnapshotGeneric, RuntimeRouteKind,
};
use runtime_proxy_crate as runtime_proxy;

pub type RuntimeProfileUsageSnapshot = RuntimeProfileUsageSnapshotGeneric<RuntimeQuotaWindowStatus>;

pub fn runtime_usage_snapshot_to_proxy(
    snapshot: &RuntimeProfileUsageSnapshot,
) -> runtime_proxy::RuntimeProxyUsageSnapshot {
    runtime_proxy::RuntimeProxyUsageSnapshot {
        checked_at: snapshot.checked_at,
        five_hour_status: runtime_quota_window_status_to_proxy(snapshot.five_hour_status),
        five_hour_remaining_percent: snapshot.five_hour_remaining_percent,
        five_hour_reset_at: snapshot.five_hour_reset_at,
        weekly_status: runtime_quota_window_status_to_proxy(snapshot.weekly_status),
        weekly_remaining_percent: snapshot.weekly_remaining_percent,
        weekly_reset_at: snapshot.weekly_reset_at,
    }
}

pub fn runtime_usage_snapshot_from_proxy(
    snapshot: runtime_proxy::RuntimeProxyUsageSnapshot,
) -> RuntimeProfileUsageSnapshot {
    RuntimeProfileUsageSnapshot {
        checked_at: snapshot.checked_at,
        five_hour_status: runtime_quota_window_status_from_proxy(snapshot.five_hour_status),
        five_hour_remaining_percent: snapshot.five_hour_remaining_percent,
        five_hour_reset_at: snapshot.five_hour_reset_at,
        weekly_status: runtime_quota_window_status_from_proxy(snapshot.weekly_status),
        weekly_remaining_percent: snapshot.weekly_remaining_percent,
        weekly_reset_at: snapshot.weekly_reset_at,
    }
}

pub fn runtime_profile_usage_snapshot_from_usage(
    usage: &UsageResponse,
) -> RuntimeProfileUsageSnapshot {
    runtime_usage_snapshot_from_proxy(
        runtime_proxy::runtime_proxy_usage_snapshot_from_observations_at(
            runtime_quota_window_observation(usage, "5h"),
            runtime_quota_window_observation(usage, "weekly"),
            Local::now().timestamp(),
        ),
    )
}

pub fn runtime_quota_summary_from_usage_snapshot(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, Local::now().timestamp())
}

pub fn runtime_quota_summary_from_usage_snapshot_at(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> RuntimeQuotaSummary {
    runtime_quota_summary_from_proxy(
        runtime_proxy::runtime_proxy_quota_summary_from_usage_snapshot_at(
            runtime_usage_snapshot_to_proxy(snapshot),
            runtime_route_kind_to_proxy(route_kind),
            now,
        ),
    )
}

pub fn runtime_profile_usage_snapshot_hold_active(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    runtime_proxy::runtime_proxy_usage_snapshot_hold_active(
        runtime_usage_snapshot_to_proxy(snapshot),
        now,
    )
}

pub fn runtime_profile_usage_snapshot_hold_expired(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    runtime_proxy::runtime_proxy_usage_snapshot_hold_expired(
        runtime_usage_snapshot_to_proxy(snapshot),
        now,
    )
}

pub fn runtime_usage_snapshot_is_usable(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
    stale_grace_seconds: i64,
) -> bool {
    runtime_proxy::runtime_proxy_usage_snapshot_is_usable(
        runtime_usage_snapshot_to_proxy(snapshot),
        now,
        stale_grace_seconds,
    )
}

pub fn runtime_snapshot_blocks_same_request_cold_start_probe(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
    stale_grace_seconds: i64,
    responses_critical_floor_percent: i64,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
        && runtime_quota_precommit_guard_reason(
            runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now),
            route_kind,
            responses_critical_floor_percent,
        )
        .is_some()
}

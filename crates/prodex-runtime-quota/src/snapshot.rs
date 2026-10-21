use crate::summary::{
    runtime_quota_summary_for_route_with_model_at, runtime_quota_summary_from_proxy,
};
use crate::window::{runtime_quota_window_status_from_proxy, runtime_quota_window_status_to_proxy};
use chrono::Local;
use prodex_quota::{
    RuntimeQuotaSummary, RuntimeQuotaWindowStatus, UsageResponse, UsageWindow, WindowPair,
};
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
        plan_type: None,
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
    let now = Local::now().timestamp();
    let summary = runtime_quota_summary_for_route_with_model_at(
        usage,
        RuntimeRouteKind::Responses,
        Some("gpt-5.6-sol"),
        now,
    );
    RuntimeProfileUsageSnapshot {
        checked_at: now,
        plan_type: usage.plan_type.clone(),
        five_hour_status: summary.five_hour.status,
        five_hour_remaining_percent: summary.five_hour.remaining_percent,
        five_hour_reset_at: summary.five_hour.reset_at,
        weekly_status: summary.weekly.status,
        weekly_remaining_percent: summary.weekly.remaining_percent,
        weekly_reset_at: summary.weekly.reset_at,
    }
}

fn runtime_quota_snapshot_status_tag(status: RuntimeQuotaWindowStatus) -> i64 {
    match status {
        RuntimeQuotaWindowStatus::Ready => 0,
        RuntimeQuotaWindowStatus::Thin => 1,
        RuntimeQuotaWindowStatus::Critical => 2,
        RuntimeQuotaWindowStatus::Exhausted => 3,
        RuntimeQuotaWindowStatus::Unknown => 4,
    }
}

pub fn usage_from_runtime_usage_snapshot(snapshot: &RuntimeProfileUsageSnapshot) -> UsageResponse {
    let plan = prodex_mojo_core::runtime_state_quota::quota_snapshot_rebuild_plan([
        runtime_quota_snapshot_status_tag(snapshot.five_hour_status),
        snapshot.five_hour_remaining_percent,
        snapshot.five_hour_reset_at,
        runtime_quota_snapshot_status_tag(snapshot.weekly_status),
        snapshot.weekly_remaining_percent,
        snapshot.weekly_reset_at,
    ])
    .expect("compiled Mojo snapshot reconstruction returned invalid output");
    // Rust only constructs the serde/chrono domain model from the checked plan.
    let window = |chosen: prodex_mojo_core::runtime_state_quota::QuotaSnapshotRebuildWindow,
                  reset_at: i64,
                  limit_window_seconds: i64| {
        chosen.include.then(|| UsageWindow {
            used_percent: Some(chosen.used_percent),
            reset_at: chosen.include_reset.then_some(reset_at),
            limit_window_seconds: Some(limit_window_seconds),
        })
    };
    UsageResponse {
        email: None,
        plan_type: snapshot.plan_type.clone(),
        rate_limit: Some(WindowPair {
            allowed: None,
            limit_reached: None,
            extra: std::collections::BTreeMap::new(),
            primary_window: window(plan.five_hour, snapshot.five_hour_reset_at, 18_000),
            secondary_window: window(plan.weekly, snapshot.weekly_reset_at, 604_800),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    }
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
            route_kind,
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
    _responses_critical_floor_percent: i64,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
        && {
            let summary = runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now);
            matches!(
                summary.five_hour.status,
                RuntimeQuotaWindowStatus::Exhausted
            )
        }
}

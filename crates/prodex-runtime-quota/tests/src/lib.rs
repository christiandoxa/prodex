use super::*;
use prodex_quota::{UsageWindow, WindowPair};

fn usage_response(
    five_hour_used_percent: i64,
    weekly_used_percent: i64,
    now: i64,
) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(five_hour_used_percent),
                reset_at: Some(now + 3_600),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(weekly_used_percent),
                reset_at: Some(now + 86_400),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

#[test]
fn quota_summary_for_route_matches_usage_windows() {
    let now = Local::now().timestamp();
    let usage = usage_response(96, 20, now);

    let summary = runtime_quota_summary_for_route(&usage, RuntimeRouteKind::Responses);

    assert_eq!(summary.five_hour.status, RuntimeQuotaWindowStatus::Critical);
    assert_eq!(summary.five_hour.remaining_percent, 4);
    assert_eq!(summary.weekly.status, RuntimeQuotaWindowStatus::Ready);
    assert_eq!(summary.weekly.remaining_percent, 80);
    assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Critical);
}

#[test]
fn cold_start_probe_block_respects_snapshot_guard() {
    let now = Local::now().timestamp();
    let snapshot = RuntimeProfileUsageSnapshot {
        checked_at: now,
        five_hour_status: RuntimeQuotaWindowStatus::Critical,
        five_hour_remaining_percent: 1,
        five_hour_reset_at: now + 3_600,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 80,
        weekly_reset_at: now + 86_400,
    };

    assert!(runtime_snapshot_blocks_same_request_cold_start_probe(
        &snapshot,
        RuntimeRouteKind::Responses,
        now,
        900,
        2,
    ));
    assert!(!runtime_snapshot_blocks_same_request_cold_start_probe(
        &snapshot,
        RuntimeRouteKind::Compact,
        now,
        900,
        2,
    ));
}

#[test]
fn cached_source_summary_prefers_live_probe_over_snapshot() {
    let now = 10_000;
    let usage = usage_response(96, 20, now);
    let snapshot = RuntimeProfileUsageSnapshot {
        checked_at: now,
        five_hour_status: RuntimeQuotaWindowStatus::Ready,
        five_hour_remaining_percent: 90,
        five_hour_reset_at: now + 3_600,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 90,
        weekly_reset_at: now + 86_400,
    };

    let (summary, source) = runtime_quota_summary_from_cached_sources(
        Some(&usage),
        Some(&snapshot),
        RuntimeRouteKind::Responses,
        now,
        900,
    );

    assert_eq!(source, Some(RuntimeQuotaSource::LiveProbe));
    assert_eq!(summary.five_hour.status, RuntimeQuotaWindowStatus::Critical);
    assert_eq!(summary.five_hour.remaining_percent, 4);
}

#[test]
fn cached_source_summary_keeps_active_exhausted_snapshot() {
    let now = 10_000;
    let snapshot = RuntimeProfileUsageSnapshot {
        checked_at: now - 10_000,
        five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
        five_hour_remaining_percent: 0,
        five_hour_reset_at: now + 3_600,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 90,
        weekly_reset_at: now + 86_400,
    };

    let (summary, source) = runtime_quota_summary_from_cached_sources(
        None,
        Some(&snapshot),
        RuntimeRouteKind::Responses,
        now,
        900,
    );

    assert_eq!(source, Some(RuntimeQuotaSource::PersistedSnapshot));
    assert_eq!(
        summary.five_hour.status,
        RuntimeQuotaWindowStatus::Exhausted
    );
    assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Exhausted);
}

#[test]
fn cached_source_summary_ignores_stale_non_hold_snapshot() {
    let now = 10_000;
    let snapshot = RuntimeProfileUsageSnapshot {
        checked_at: now - 10_000,
        five_hour_status: RuntimeQuotaWindowStatus::Ready,
        five_hour_remaining_percent: 90,
        five_hour_reset_at: now + 3_600,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 90,
        weekly_reset_at: now + 86_400,
    };

    let (summary, source) = runtime_quota_summary_from_cached_sources(
        None,
        Some(&snapshot),
        RuntimeRouteKind::Responses,
        now,
        900,
    );

    assert_eq!(source, None);
    assert_eq!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown);
    assert_eq!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown);
    assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Unknown);
}

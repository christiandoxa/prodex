use super::*;
use prodex_quota::{UsageWindow, WindowPair};

#[test]
fn quota_summary_for_route_matches_usage_windows() {
    let now = Local::now().timestamp();
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(96),
                reset_at: Some(now + 3_600),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: Some(now + 86_400),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

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

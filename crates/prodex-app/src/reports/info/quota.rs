//! Quota aggregation for info and status reports.

use super::{
    BTreeMap, InfoQuotaAggregate, RunProfileProbeReport, RuntimeProfileUsageSnapshot,
    required_main_window_snapshot_at, runtime_usage_snapshot_is_usable,
    usage_from_runtime_usage_snapshot,
};
use prodex_mojo_core::quota_pool::{
    QuotaPoolWindowInput, StatusQuotaProfileInput, StatusQuotaSummary, status_quota_summary_batch,
};

pub fn build_info_quota_aggregate(
    reports: &[RunProfileProbeReport],
    persisted_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
    stale_grace_seconds: i64,
) -> InfoQuotaAggregate {
    let summary =
        quota_report_summary(reports, persisted_usage_snapshots, now, stale_grace_seconds)
            .expect("Mojo Info quota summary failed");
    InfoQuotaAggregate {
        quota_compatible_profiles: summary.compatible_profiles,
        live_profiles: summary.live_profiles,
        snapshot_profiles: summary.snapshot_profiles,
        unavailable_profiles: summary.unavailable_profiles,
        five_hour_profiles_with_data: summary.five_hour.profiles,
        weekly_profiles_with_data: summary.weekly.profiles,
        five_hour_pool_remaining: summary.five_hour.total_remaining,
        weekly_pool_remaining: summary.weekly.total_remaining,
        earliest_five_hour_reset_at: summary.five_hour.earliest_reset_at,
        earliest_weekly_reset_at: summary.weekly.earliest_reset_at,
    }
}

// Rust projects dependency-owned usage types; Mojo chooses sources and reduces windows.
pub(crate) fn quota_report_summary(
    reports: &[RunProfileProbeReport],
    snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
    stale_grace_seconds: i64,
) -> Result<StatusQuotaSummary, prodex_mojo_core::MojoError> {
    let mut inputs = Vec::with_capacity(reports.len());
    for report in reports {
        let report_succeeded = report.result.is_ok();
        let report_windows = report
            .result
            .as_ref()
            .ok()
            .map(|usage| quota_report_windows(usage, now))
            .unwrap_or_default();
        let cached_usage = snapshots.get(&report.name).filter(|snapshot| {
            runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
        });
        let cached_snapshot_usable = cached_usage.is_some();
        let cached_windows = cached_usage
            .map(|snapshot| {
                let usage = usage_from_runtime_usage_snapshot(snapshot);
                quota_report_windows(&usage, now)
            })
            .unwrap_or_default();
        inputs.push(StatusQuotaProfileInput {
            quota_compatible: report.auth.quota_compatible,
            report_succeeded,
            cached_snapshot_usable,
            report_five_hour: report_windows.0,
            report_weekly: report_windows.1,
            cached_five_hour: cached_windows.0,
            cached_weekly: cached_windows.1,
        });
    }

    status_quota_summary_batch(&inputs)
}

fn quota_report_windows(
    usage: &prodex_quota::UsageResponse,
    now: i64,
) -> (Option<QuotaPoolWindowInput>, Option<QuotaPoolWindowInput>) {
    let window = |label| {
        required_main_window_snapshot_at(usage, label, now).map(|snapshot| QuotaPoolWindowInput {
            remaining_percent: snapshot.remaining_percent,
            reset_at: snapshot.reset_at,
        })
    };
    (window("5h"), window("weekly"))
}

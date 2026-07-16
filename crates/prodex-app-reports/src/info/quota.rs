use super::*;

pub fn build_info_quota_aggregate(
    reports: &[RunProfileProbeReport],
    persisted_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
    stale_grace_seconds: i64,
) -> InfoQuotaAggregate {
    let mut aggregate = InfoQuotaAggregate {
        quota_compatible_profiles: reports
            .iter()
            .filter(|report| report.auth.quota_compatible)
            .count(),
        live_profiles: 0,
        snapshot_profiles: 0,
        unavailable_profiles: 0,
        five_hour_pool_remaining: 0,
        weekly_pool_remaining: 0,
        earliest_five_hour_reset_at: None,
        earliest_weekly_reset_at: None,
    };

    for report in reports {
        if !report.auth.quota_compatible {
            continue;
        }

        let usage = match &report.result {
            Ok(usage) => Some((usage.clone(), InfoQuotaSource::LiveProbe)),
            Err(_) => persisted_usage_snapshots
                .get(&report.name)
                .filter(|snapshot| {
                    runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
                })
                .map(|snapshot| {
                    (
                        usage_from_runtime_usage_snapshot(snapshot),
                        InfoQuotaSource::PersistedSnapshot,
                    )
                }),
        };

        let Some((usage, source)) = usage else {
            aggregate.unavailable_profiles += 1;
            continue;
        };

        let Some((five_hour, weekly)) = info_main_window_snapshots_at(&usage, now) else {
            aggregate.unavailable_profiles += 1;
            continue;
        };

        match source {
            InfoQuotaSource::LiveProbe => aggregate.live_profiles += 1,
            InfoQuotaSource::PersistedSnapshot => aggregate.snapshot_profiles += 1,
        }
        aggregate.five_hour_pool_remaining += five_hour.remaining_percent;
        aggregate.weekly_pool_remaining += weekly.remaining_percent;
        if five_hour.reset_at != i64::MAX {
            aggregate.earliest_five_hour_reset_at = Some(
                aggregate
                    .earliest_five_hour_reset_at
                    .map_or(five_hour.reset_at, |current| {
                        current.min(five_hour.reset_at)
                    }),
            );
        }
        if weekly.reset_at != i64::MAX {
            aggregate.earliest_weekly_reset_at = Some(
                aggregate
                    .earliest_weekly_reset_at
                    .map_or(weekly.reset_at, |current| current.min(weekly.reset_at)),
            );
        }
    }

    aggregate
}

pub fn info_main_window_snapshots_at(
    usage: &UsageResponse,
    now: i64,
) -> Option<(MainWindowSnapshot, MainWindowSnapshot)> {
    Some((
        required_main_window_snapshot_at(usage, "5h", now)?,
        required_main_window_snapshot_at(usage, "weekly", now)?,
    ))
}

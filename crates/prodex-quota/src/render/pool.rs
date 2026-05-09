use super::*;

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct QuotaPoolAggregate {
    total_profiles: usize,
    available_profiles: usize,
    profiles_with_data: usize,
    five_hour_pool_remaining: i64,
    weekly_pool_remaining: i64,
    earliest_five_hour_reset_at: Option<i64>,
    earliest_weekly_reset_at: Option<i64>,
    last_updated_at: Option<i64>,
}

pub(super) fn collect_quota_pool_aggregate(reports: &[QuotaReport]) -> QuotaPoolAggregate {
    let mut aggregate = QuotaPoolAggregate {
        total_profiles: reports.len(),
        ..QuotaPoolAggregate::default()
    };

    for report in reports {
        aggregate.last_updated_at = Some(
            aggregate
                .last_updated_at
                .map_or(report.fetched_at, |current| current.max(report.fetched_at)),
        );
        let Ok(snapshot) = &report.result else {
            continue;
        };
        aggregate.available_profiles += 1;
        let ProviderQuotaSnapshot::OpenAi(usage) = snapshot else {
            continue;
        };
        let Some((five_hour, weekly)) = main_window_snapshots(usage) else {
            continue;
        };

        aggregate.profiles_with_data += 1;
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

pub(super) fn render_quota_pool_summary_lines(
    aggregate: &QuotaPoolAggregate,
    total_width: usize,
) -> Vec<String> {
    let fields = vec![
        (
            "Available".to_string(),
            format!(
                "{}/{} profile",
                aggregate.available_profiles, aggregate.total_profiles
            ),
        ),
        (
            "Last Updated".to_string(),
            format_quota_snapshot_time(aggregate.last_updated_at),
        ),
        (
            "5h remaining pool".to_string(),
            format_info_pool_remaining(
                aggregate.five_hour_pool_remaining,
                aggregate.profiles_with_data,
                aggregate.earliest_five_hour_reset_at,
            ),
        ),
        (
            "Weekly remaining pool".to_string(),
            format_info_pool_remaining(
                aggregate.weekly_pool_remaining,
                aggregate.profiles_with_data,
                aggregate.earliest_weekly_reset_at,
            ),
        ),
    ];
    let label_width = fields
        .iter()
        .map(|(label, _)| text_width(label) + 1)
        .max()
        .unwrap_or(CLI_LABEL_WIDTH)
        .min(total_width.saturating_sub(2).max(1));
    let mut lines = Vec::new();

    for (label, value) in fields {
        lines.extend(format_field_lines_with_layout(
            &label,
            &value,
            total_width,
            label_width,
        ));
    }

    lines
}

pub fn format_info_pool_remaining(
    total_remaining: i64,
    profiles_with_data: usize,
    earliest_reset_at: Option<i64>,
) -> String {
    if profiles_with_data == 0 {
        return "Unavailable".to_string();
    }

    let mut value = format!("{total_remaining}% across {profiles_with_data} profile(s)");
    if let Some(reset_at) = earliest_reset_at {
        value.push_str(&format!(
            "; earliest reset {}",
            format_precise_reset_time(Some(reset_at))
        ));
    }
    value
}

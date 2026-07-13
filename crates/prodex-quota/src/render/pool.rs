use super::*;

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct QuotaPoolAggregate {
    total_profiles: usize,
    available_profiles: usize,
    profiles_with_data: usize,
    ready_profiles_with_data: usize,
    five_hour_profiles_with_data: usize,
    weekly_profiles_with_data: usize,
    ready_five_hour_profiles_with_data: usize,
    ready_weekly_profiles_with_data: usize,
    main_profiles_with_data: usize,
    five_hour_pool_remaining: i64,
    weekly_pool_remaining: i64,
    ready_five_hour_pool_remaining: i64,
    ready_weekly_pool_remaining: i64,
    main_pool_remaining: i64,
    spark_profiles_with_data: usize,
    spark_five_hour_profiles_with_data: usize,
    spark_weekly_profiles_with_data: usize,
    spark_five_hour_pool_remaining: i64,
    spark_weekly_pool_remaining: i64,
    earliest_five_hour_reset_at: Option<i64>,
    earliest_weekly_reset_at: Option<i64>,
    earliest_main_reset_at: Option<i64>,
    earliest_spark_reset_at: Option<i64>,
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
        if provider_quota_snapshot_is_available(snapshot) {
            aggregate.available_profiles += 1;
        }
        match snapshot {
            ProviderQuotaSnapshot::OpenAi(usage) => {
                let five_hour = required_main_window_snapshot(usage, "5h");
                let weekly = required_main_window_snapshot(usage, "weekly");
                if five_hour.is_none() && weekly.is_none() {
                    continue;
                }

                aggregate.profiles_with_data += 1;
                if let Some(five_hour) = five_hour {
                    aggregate.five_hour_profiles_with_data += 1;
                    aggregate.five_hour_pool_remaining += five_hour.remaining_percent;
                    if five_hour.reset_at != i64::MAX {
                        aggregate.earliest_five_hour_reset_at = Some(
                            aggregate
                                .earliest_five_hour_reset_at
                                .map_or(five_hour.reset_at, |current| {
                                    current.min(five_hour.reset_at)
                                }),
                        );
                    }
                }
                if let Some(weekly) = weekly {
                    aggregate.weekly_profiles_with_data += 1;
                    aggregate.weekly_pool_remaining += weekly.remaining_percent;
                    if weekly.reset_at != i64::MAX {
                        aggregate.earliest_weekly_reset_at = Some(
                            aggregate
                                .earliest_weekly_reset_at
                                .map_or(weekly.reset_at, |current| current.min(weekly.reset_at)),
                        );
                    }
                }
                if openai_quota_has_ready_limit(usage) {
                    aggregate.ready_profiles_with_data += 1;
                    if let Some(five_hour) = five_hour {
                        aggregate.ready_five_hour_profiles_with_data += 1;
                        aggregate.ready_five_hour_pool_remaining += five_hour.remaining_percent;
                    }
                    if let Some(weekly) = weekly {
                        aggregate.ready_weekly_profiles_with_data += 1;
                        aggregate.ready_weekly_pool_remaining += weekly.remaining_percent;
                    }
                }
                let spark_five_hour = spark_window_snapshot(usage, "5h");
                let spark_weekly = spark_window_snapshot(usage, "weekly");
                if spark_five_hour.is_some() || spark_weekly.is_some() {
                    aggregate.spark_profiles_with_data += 1;
                    if let Some(window) = spark_five_hour {
                        aggregate.spark_five_hour_profiles_with_data += 1;
                        aggregate.spark_five_hour_pool_remaining += window.remaining_percent;
                    }
                    if let Some(window) = spark_weekly {
                        aggregate.spark_weekly_profiles_with_data += 1;
                        aggregate.spark_weekly_pool_remaining += window.remaining_percent;
                    }
                    for reset_at in [spark_five_hour, spark_weekly]
                        .into_iter()
                        .flatten()
                        .map(|window| window.reset_at)
                    {
                        if reset_at != i64::MAX {
                            aggregate.earliest_spark_reset_at = Some(
                                aggregate
                                    .earliest_spark_reset_at
                                    .map_or(reset_at, |current| current.min(reset_at)),
                            );
                        }
                    }
                }
            }
            ProviderQuotaSnapshot::Gemini(info) => {
                let Some(remaining_percent) = gemini_main_remaining_percent(info) else {
                    continue;
                };
                aggregate.main_profiles_with_data += 1;
                aggregate.main_pool_remaining += remaining_percent;
                if let Some(reset_at) = gemini_reset_epoch(info) {
                    aggregate.earliest_main_reset_at = Some(
                        aggregate
                            .earliest_main_reset_at
                            .map_or(reset_at, |current| current.min(reset_at)),
                    );
                }
            }
            ProviderQuotaSnapshot::Copilot(info) => {
                let Some(remaining_percent) = copilot_main_remaining_percent(info) else {
                    continue;
                };
                aggregate.main_profiles_with_data += 1;
                aggregate.main_pool_remaining += remaining_percent;
                if let Some(reset_at) = copilot_reset_epoch(info) {
                    aggregate.earliest_main_reset_at = Some(
                        aggregate
                            .earliest_main_reset_at
                            .map_or(reset_at, |current| current.min(reset_at)),
                    );
                }
            }
            ProviderQuotaSnapshot::External(_) => {}
        }
    }

    aggregate
}

fn gemini_main_remaining_percent(info: &GeminiQuotaInfo) -> Option<i64> {
    info.buckets
        .iter()
        .filter_map(gemini_bucket_remaining_percent)
        .min()
}

fn copilot_main_remaining_percent(info: &CopilotQuotaInfo) -> Option<i64> {
    ["chat", "completions"]
        .into_iter()
        .filter_map(|feature| {
            let total = info.monthly_quotas.get(feature).copied()?;
            (total > 0).then(|| {
                let remaining = info
                    .limited_user_quotas
                    .get(feature)
                    .copied()
                    .or_else(|| info.monthly_quotas.get(feature).copied())
                    .unwrap_or(0);
                ((remaining as f64 / total as f64) * 100.0).round() as i64
            })
        })
        .min()
}

fn provider_quota_snapshot_is_available(snapshot: &ProviderQuotaSnapshot) -> bool {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => openai_quota_has_ready_limit(usage),
        ProviderQuotaSnapshot::Copilot(info) => copilot_quota_is_ready(info),
        ProviderQuotaSnapshot::Gemini(info) => gemini_quota_is_ready(info),
        ProviderQuotaSnapshot::External(info) => info.available.unwrap_or(false),
    }
}

pub(super) fn render_quota_pool_summary_lines(
    aggregate: &QuotaPoolAggregate,
    total_width: usize,
) -> Vec<String> {
    let fields = quota_pool_summary_fields_for_aggregate(aggregate);
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

pub fn quota_pool_summary_fields(reports: &[QuotaReport]) -> Vec<(String, String)> {
    quota_pool_summary_fields_for_aggregate(&collect_quota_pool_aggregate(reports))
}

fn quota_pool_summary_fields_for_aggregate(
    aggregate: &QuotaPoolAggregate,
) -> Vec<(String, String)> {
    let mut fields = vec![
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
    ];
    if aggregate.profiles_with_data > 0 {
        fields.push((
            "Usable now".to_string(),
            format_ready_pool_remaining(
                aggregate.ready_five_hour_pool_remaining,
                aggregate.ready_weekly_pool_remaining,
                aggregate.ready_profiles_with_data,
                aggregate.ready_five_hour_profiles_with_data,
                aggregate.ready_weekly_profiles_with_data,
            ),
        ));
        fields.extend([
            (
                "5h remaining pool".to_string(),
                format_info_pool_remaining(
                    aggregate.five_hour_pool_remaining,
                    aggregate.five_hour_profiles_with_data,
                    aggregate.earliest_five_hour_reset_at,
                ),
            ),
            (
                "Weekly remaining pool".to_string(),
                format_info_pool_remaining(
                    aggregate.weekly_pool_remaining,
                    aggregate.weekly_profiles_with_data,
                    aggregate.earliest_weekly_reset_at,
                ),
            ),
        ]);
    } else if aggregate.main_profiles_with_data > 0 {
        fields.push((
            "Remaining pool".to_string(),
            format_info_pool_remaining(
                aggregate.main_pool_remaining,
                aggregate.main_profiles_with_data,
                aggregate.earliest_main_reset_at,
            ),
        ));
    } else {
        fields.extend([
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
        ]);
    }
    if aggregate.spark_profiles_with_data > 0 {
        fields.push((
            "Spark remaining pool".to_string(),
            format_dual_pool_remaining(
                aggregate.spark_five_hour_pool_remaining,
                aggregate.spark_weekly_pool_remaining,
                aggregate.spark_profiles_with_data,
                aggregate.spark_five_hour_profiles_with_data,
                aggregate.spark_weekly_profiles_with_data,
                aggregate.earliest_spark_reset_at,
            ),
        ));
    }
    fields
}

fn format_ready_pool_remaining(
    five_hour_remaining: i64,
    weekly_remaining: i64,
    ready_profiles: usize,
    five_hour_profiles: usize,
    weekly_profiles: usize,
) -> String {
    if ready_profiles == 0 {
        return "Unavailable".to_string();
    }

    let mut windows = Vec::new();
    if five_hour_profiles > 0 {
        windows.push(format!("5h {five_hour_remaining}%"));
    }
    if weekly_profiles > 0 {
        windows.push(format!("weekly {weekly_remaining}%"));
    }
    format!(
        "{} across {ready_profiles} ready profile(s)",
        windows.join(" | ")
    )
}

fn format_dual_pool_remaining(
    five_hour_remaining: i64,
    weekly_remaining: i64,
    profiles_with_data: usize,
    five_hour_profiles: usize,
    weekly_profiles: usize,
    earliest_reset_at: Option<i64>,
) -> String {
    if profiles_with_data == 0 {
        return "Unavailable".to_string();
    }

    let mut value = if five_hour_profiles == profiles_with_data
        && weekly_profiles == profiles_with_data
    {
        format!(
            "5h {five_hour_remaining}% | weekly {weekly_remaining}% across {profiles_with_data} profile(s)"
        )
    } else {
        let mut windows = Vec::new();
        if five_hour_profiles > 0 {
            windows.push(format!(
                "5h {five_hour_remaining}% across {five_hour_profiles} profile(s)"
            ));
        }
        if weekly_profiles > 0 {
            windows.push(format!(
                "weekly {weekly_remaining}% across {weekly_profiles} profile(s)"
            ));
        }
        windows.join(" | ")
    };
    if let Some(reset_at) = earliest_reset_at {
        value.push_str(&format!(
            "; earliest reset {}",
            format_precise_reset_time(Some(reset_at))
        ));
    }
    value
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

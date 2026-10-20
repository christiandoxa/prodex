use super::*;

pub use prodex_mojo_core::quota_pool::{
    QuotaPoolWindowInput, StatusQuotaProfileInput, status_quota_summary_batch,
};

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
    earliest_five_hour_reset_at: Option<i64>,
    earliest_weekly_reset_at: Option<i64>,
    earliest_main_reset_at: Option<i64>,
    last_updated_at: Option<i64>,
}

pub(super) fn collect_quota_pool_aggregate(reports: &[QuotaReport]) -> QuotaPoolAggregate {
    let observations = reports
        .iter()
        .map(
            |report| prodex_mojo_core::quota_pool::QuotaReportOverviewInput {
                fetched_at: report.fetched_at,
                available: report
                    .result
                    .as_ref()
                    .is_ok_and(provider_quota_snapshot_is_available),
            },
        )
        .collect::<Vec<_>>();
    let overview = prodex_mojo_core::quota_pool::quota_pool_report_overview(&observations)
        .expect("compiled Mojo quota report overview returned an invalid plan");
    let mut aggregate = QuotaPoolAggregate {
        total_profiles: overview.total_profiles,
        available_profiles: overview.available_profiles,
        last_updated_at: overview.last_updated_at,
        ..QuotaPoolAggregate::default()
    };
    let mut main_quota_rows = Vec::<prodex_mojo_core::quota::MainQuotaAggregationInput>::new();
    let mut openai_quota_rows = Vec::new();

    for report in reports {
        let Ok(snapshot) = &report.result else {
            continue;
        };
        match snapshot {
            ProviderQuotaSnapshot::OpenAi(usage) => {
                let five_hour = required_main_window_snapshot(usage, "5h");
                let weekly = required_main_window_snapshot(usage, "weekly");
                if five_hour.is_none() && weekly.is_none() {
                    continue;
                }
                let ready = openai_quota_has_ready_limit(usage);
                openai_quota_rows.push(prodex_mojo_core::quota_pool::OpenAiQuotaPoolInput {
                    five_hour: five_hour.map(|window| {
                        prodex_mojo_core::quota_pool::QuotaPoolWindowInput {
                            remaining_percent: window.remaining_percent,
                            reset_at: window.reset_at,
                        }
                    }),
                    weekly: weekly.map(|window| {
                        prodex_mojo_core::quota_pool::QuotaPoolWindowInput {
                            remaining_percent: window.remaining_percent,
                            reset_at: window.reset_at,
                        }
                    }),
                    ready,
                });
            }
            ProviderQuotaSnapshot::Gemini(info) => {
                main_quota_rows.push(prodex_mojo_core::quota::MainQuotaAggregationInput {
                    remaining_percent: super::gemini_main_remaining_percent(info),
                    reset_at: gemini_reset_epoch(info),
                });
            }
            ProviderQuotaSnapshot::Copilot(info) => {
                main_quota_rows.push(prodex_mojo_core::quota::MainQuotaAggregationInput {
                    remaining_percent: copilot_main_remaining_percent(info),
                    reset_at: copilot_reset_epoch(info),
                });
            }
            ProviderQuotaSnapshot::External(_) => {}
        }
    }

    let main = prodex_mojo_core::quota::main_quota_aggregate_batch(&main_quota_rows)
        .expect("Mojo main quota aggregation rejected normalized rows");
    aggregate.main_profiles_with_data = main.profiles_with_data;
    aggregate.main_pool_remaining = main.pool_remaining;
    aggregate.earliest_main_reset_at = main.earliest_reset_at;

    let openai = prodex_mojo_core::quota_pool::openai_quota_pool_aggregate(&openai_quota_rows)
        .expect("Mojo OpenAI quota-pool aggregation rejected normalized rows");
    aggregate.profiles_with_data = openai.profiles_with_data;
    aggregate.ready_profiles_with_data = openai.ready_profiles_with_data;
    aggregate.five_hour_profiles_with_data = openai.five_hour_profiles_with_data;
    aggregate.weekly_profiles_with_data = openai.weekly_profiles_with_data;
    aggregate.ready_five_hour_profiles_with_data = openai.ready_five_hour_profiles_with_data;
    aggregate.ready_weekly_profiles_with_data = openai.ready_weekly_profiles_with_data;
    aggregate.five_hour_pool_remaining = openai.five_hour_pool_remaining;
    aggregate.weekly_pool_remaining = openai.weekly_pool_remaining;
    aggregate.ready_five_hour_pool_remaining = openai.ready_five_hour_pool_remaining;
    aggregate.ready_weekly_pool_remaining = openai.ready_weekly_pool_remaining;
    aggregate.earliest_five_hour_reset_at = openai.earliest_five_hour_reset_at;
    aggregate.earliest_weekly_reset_at = openai.earliest_weekly_reset_at;

    aggregate
}

fn provider_quota_snapshot_is_available(snapshot: &ProviderQuotaSnapshot) -> bool {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => openai_quota_has_ready_limit(usage),
        ProviderQuotaSnapshot::Copilot(info) => copilot_quota_is_ready(info),
        ProviderQuotaSnapshot::Gemini(info) => gemini_quota_is_ready(info),
        ProviderQuotaSnapshot::External(info) => info.available.unwrap_or(false),
    }
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
    for section in prodex_mojo_core::quota_pool::quota_pool_summary_sections(
        aggregate.profiles_with_data,
        aggregate.main_profiles_with_data,
    )
    .expect("compiled Mojo quota-pool summary section planner failed")
    {
        use prodex_mojo_core::quota_pool::QuotaPoolSummarySection;
        let (label, value) = match section {
            QuotaPoolSummarySection::UsableNow => (
                "Usable now",
                format_ready_pool_remaining(
                    aggregate.ready_five_hour_pool_remaining,
                    aggregate.ready_weekly_pool_remaining,
                    aggregate.ready_profiles_with_data,
                    aggregate.ready_five_hour_profiles_with_data,
                    aggregate.ready_weekly_profiles_with_data,
                ),
            ),
            QuotaPoolSummarySection::FiveHour => (
                "5h remaining pool",
                format_info_pool_remaining(
                    aggregate.five_hour_pool_remaining,
                    aggregate.five_hour_profiles_with_data,
                    aggregate.earliest_five_hour_reset_at,
                ),
            ),
            QuotaPoolSummarySection::Weekly => (
                "Weekly remaining pool",
                format_info_pool_remaining(
                    aggregate.weekly_pool_remaining,
                    aggregate.weekly_profiles_with_data,
                    aggregate.earliest_weekly_reset_at,
                ),
            ),
            QuotaPoolSummarySection::Main => (
                "Remaining pool",
                format_info_pool_remaining(
                    aggregate.main_pool_remaining,
                    aggregate.main_profiles_with_data,
                    aggregate.earliest_main_reset_at,
                ),
            ),
            QuotaPoolSummarySection::NoDataFiveHour => (
                "5h remaining pool",
                format_info_pool_remaining(
                    aggregate.five_hour_pool_remaining,
                    aggregate.profiles_with_data,
                    aggregate.earliest_five_hour_reset_at,
                ),
            ),
            QuotaPoolSummarySection::NoDataWeekly => (
                "Weekly remaining pool",
                format_info_pool_remaining(
                    aggregate.weekly_pool_remaining,
                    aggregate.profiles_with_data,
                    aggregate.earliest_weekly_reset_at,
                ),
            ),
        };
        fields.push((label.to_owned(), value));
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
    prodex_mojo_core::quota::quota_ready_pool_remaining(
        five_hour_remaining,
        weekly_remaining,
        ready_profiles,
        five_hour_profiles,
        weekly_profiles,
    )
    .expect("Mojo ready quota-pool rendering returned invalid output")
}

pub fn format_info_pool_remaining(
    total_remaining: i64,
    profiles_with_data: usize,
    earliest_reset_at: Option<i64>,
) -> String {
    let reset = earliest_reset_at.map(|reset_at| format_precise_reset_time(Some(reset_at)));
    prodex_mojo_core::quota::quota_info_pool_remaining(
        total_remaining,
        profiles_with_data,
        reset.as_deref(),
    )
    .expect("Mojo quota-pool remaining rendering returned invalid output")
}

#[cfg(test)]
mod overview_tests {
    use super::*;
    use crate::AuthSummary;

    fn report(fetched_at: i64, result: Result<ProviderQuotaSnapshot, String>) -> QuotaReport {
        QuotaReport {
            name: format!("quota-overview-{fetched_at}"),
            active: false,
            auth: AuthSummary {
                label: "external".to_string(),
                quota_compatible: true,
            },
            workspace_id: None,
            workspace_name: None,
            result,
            fetched_at,
        }
    }

    fn external(available: bool) -> ProviderQuotaSnapshot {
        ProviderQuotaSnapshot::External(ExternalQuotaInfo {
            provider: "test".into(),
            account: None,
            plan: None,
            status: "ok".into(),
            main: "unknown".into(),
            reset: None,
            available: Some(available),
            details: Vec::new(),
        })
    }

    #[test]
    fn quota_pool_report_overview_uses_mojo_for_failed_and_available_reports() {
        let reports = [
            report(5, Ok(external(true))),
            report(20, Ok(external(false))),
            report(100, Err("unavailable".into())),
            report(-20, Ok(external(true))),
        ];
        let aggregate = collect_quota_pool_aggregate(&reports);
        assert_eq!(aggregate.total_profiles, 4);
        assert_eq!(aggregate.available_profiles, 2);
        assert_eq!(aggregate.last_updated_at, Some(100));
        assert_eq!(aggregate.profiles_with_data, 0);
        assert_eq!(aggregate.main_profiles_with_data, 0);

        let no_reports = collect_quota_pool_aggregate(&[]);
        assert_eq!(no_reports.total_profiles, 0);
        assert_eq!(no_reports.available_profiles, 0);
        assert_eq!(no_reports.last_updated_at, None);

        let only_failed = collect_quota_pool_aggregate(&[report(i64::MIN, Err("offline".into()))]);
        assert_eq!(only_failed.total_profiles, 1);
        assert_eq!(only_failed.available_profiles, 0);
        assert_eq!(only_failed.last_updated_at, Some(i64::MIN));
    }
}

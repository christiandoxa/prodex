use super::*;

#[derive(Debug)]
struct QuotaReportViewData {
    email: String,
    plan: String,
    main: String,
    status: String,
    resets: Option<String>,
}

#[derive(Clone, Copy)]
struct QuotaReportColumnWidths {
    profile: usize,
    current: usize,
    auth: usize,
    account: usize,
    plan: usize,
    remaining: usize,
}

#[derive(Debug, Default, Clone, Copy)]
struct QuotaPoolAggregate {
    total_profiles: usize,
    available_profiles: usize,
    profiles_with_data: usize,
    five_hour_pool_remaining: i64,
    weekly_pool_remaining: i64,
    earliest_five_hour_reset_at: Option<i64>,
    earliest_weekly_reset_at: Option<i64>,
    last_updated_at: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct RenderedQuotaReportWindow {
    pub(crate) output: String,
    pub(crate) shown_profiles: usize,
    pub(crate) total_profiles: usize,
    pub(crate) start_profile: usize,
    pub(crate) hidden_before: usize,
    pub(crate) hidden_after: usize,
}

pub(crate) fn render_quota_reports(reports: &[QuotaReport], detail: bool) -> String {
    render_quota_reports_with_layout(reports, detail, None, current_cli_width())
}

#[allow(dead_code)]
pub(crate) fn render_quota_reports_with_line_limit(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
) -> String {
    render_quota_reports_with_layout(reports, detail, max_lines, current_cli_width())
}

pub(crate) fn render_quota_reports_with_layout(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
    total_width: usize,
) -> String {
    render_quota_reports_window_with_layout(reports, detail, max_lines, total_width, 0, false)
        .output
}

pub(crate) fn render_quota_reports_window_with_layout(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
    total_width: usize,
    start_profile: usize,
    interactive_scroll_hint: bool,
) -> RenderedQuotaReportWindow {
    let column_widths = quota_report_column_widths(total_width);
    let pool_summary =
        render_quota_pool_summary_lines(&collect_quota_pool_aggregate(reports), total_width);
    let sections = build_quota_report_sections(reports, column_widths, detail, total_width);
    let header = render_quota_report_header(column_widths);
    let has_pool_summary = !pool_summary.is_empty();
    let mut output =
        render_quota_report_window_header(total_width, &pool_summary, &header, has_pool_summary);
    let total_profiles = sections.len();
    let start_profile = if total_profiles == 0 {
        0
    } else {
        start_profile.min(total_profiles.saturating_sub(1))
    };
    let shown_profiles = append_quota_report_sections(
        &mut output,
        &sections,
        max_lines,
        start_profile,
        interactive_scroll_hint,
    );
    let hidden_before = start_profile;
    let hidden_after = total_profiles.saturating_sub(start_profile.saturating_add(shown_profiles));
    RenderedQuotaReportWindow {
        output: output.join("\n"),
        shown_profiles,
        total_profiles,
        start_profile,
        hidden_before,
        hidden_after,
    }
}

fn quota_report_column_widths(total_width: usize) -> QuotaReportColumnWidths {
    const MIN_WIDTHS: [usize; 6] = [12, 3, 4, 14, 4, 13];
    const EXTRA_WEIGHTS: [usize; 6] = [12, 1, 3, 13, 4, 18];
    const DISTRIBUTION_ORDER: [usize; 6] = [5, 3, 0, 4, 2, 1];

    let gap_width = text_width(CLI_TABLE_GAP) * 5;
    let min_total = MIN_WIDTHS.iter().sum::<usize>();
    let available = total_width.saturating_sub(gap_width).max(min_total);

    let mut widths = MIN_WIDTHS;
    let mut remaining_extra = available.saturating_sub(min_total);
    let total_weight = EXTRA_WEIGHTS.iter().sum::<usize>().max(1);

    for (width, weight) in widths.iter_mut().zip(EXTRA_WEIGHTS) {
        let extra = remaining_extra * weight / total_weight;
        *width += extra;
    }

    let assigned = widths.iter().sum::<usize>().saturating_sub(min_total);
    remaining_extra = remaining_extra.saturating_sub(assigned);
    for index in DISTRIBUTION_ORDER.into_iter().cycle().take(remaining_extra) {
        widths[index] += 1;
    }

    QuotaReportColumnWidths {
        profile: widths[0],
        current: widths[1],
        auth: widths[2],
        account: widths[3],
        plan: widths[4],
        remaining: widths[5],
    }
}

fn quota_report_view_data(report: &QuotaReport) -> QuotaReportViewData {
    match &report.result {
        Ok(ProviderQuotaSnapshot::OpenAi(usage)) => {
            let blocked = collect_blocked_limits(usage, false);
            let status = if blocked.is_empty() {
                "Ready".to_string()
            } else {
                format!("Blocked: {}", format_blocked_limits(&blocked))
            };
            QuotaReportViewData {
                email: display_optional(usage.email.as_deref()).to_string(),
                plan: display_optional(usage.plan_type.as_deref()).to_string(),
                main: format_main_windows_compact(usage),
                status,
                resets: Some(format!("resets: {}", format_main_reset_summary(usage))),
            }
        }
        Ok(ProviderQuotaSnapshot::Copilot(info)) => QuotaReportViewData {
            email: display_optional(info.login.as_deref()).to_string(),
            plan: display_optional(
                info.copilot_plan
                    .as_deref()
                    .or(info.access_type_sku.as_deref()),
            )
            .to_string(),
            main: format_copilot_main_quota(info),
            status: format_copilot_quota_status(info),
            resets: Some(format_copilot_reset_summary(info).map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Err(err) => QuotaReportViewData {
            email: "-".to_string(),
            plan: "-".to_string(),
            main: "-".to_string(),
            status: format!("Error: {}", first_line_of_error(err)),
            resets: Some("resets: unavailable".to_string()),
        },
    }
}

fn render_quota_report_header(column_widths: QuotaReportColumnWidths) -> String {
    format!(
        "{:<name_w$}  {:<act_w$}  {:<auth_w$}  {:<email_w$}  {:<plan_w$}  {:<main_w$}",
        "PROFILE",
        "CUR",
        "AUTH",
        "ACCOUNT",
        "PLAN",
        "REMAINING",
        name_w = column_widths.profile,
        act_w = column_widths.current,
        auth_w = column_widths.auth,
        email_w = column_widths.account,
        plan_w = column_widths.plan,
        main_w = column_widths.remaining,
    )
}

fn render_quota_report_window_header(
    total_width: usize,
    pool_summary: &[String],
    header: &str,
    has_pool_summary: bool,
) -> Vec<String> {
    let mut output = vec![section_header_with_width("Quota Overview", total_width)];
    output.extend(pool_summary.iter().cloned());
    if has_pool_summary {
        output.push(String::new());
    }
    output.push(header.to_string());
    output.push("-".repeat(text_width(header)));
    output
}

fn build_quota_report_sections(
    reports: &[QuotaReport],
    column_widths: QuotaReportColumnWidths,
    detail: bool,
    total_width: usize,
) -> Vec<Vec<String>> {
    sort_quota_reports_for_display(reports)
        .into_iter()
        .map(|report| render_quota_report_section(report, column_widths, detail, total_width))
        .collect()
}

fn render_quota_report_section(
    report: &QuotaReport,
    column_widths: QuotaReportColumnWidths,
    detail: bool,
    total_width: usize,
) -> Vec<String> {
    let view = quota_report_view_data(report);
    let mut section = vec![render_quota_report_row(report, column_widths, &view)];
    push_wrapped_quota_report_line(
        &mut section,
        &format!("status: {}", view.status),
        total_width,
    );
    if detail && let Some(resets) = view.resets.as_deref() {
        push_wrapped_quota_report_line(&mut section, resets, total_width);
    }
    section.push(String::new());
    section
}

fn render_quota_report_row(
    report: &QuotaReport,
    column_widths: QuotaReportColumnWidths,
    view: &QuotaReportViewData,
) -> String {
    let active = if report.active { "*" } else { "" };
    format!(
        "{:<name_w$}{}{:<act_w$}{}{:<auth_w$}{}{:<email_w$}{}{:<plan_w$}{}{:<main_w$}",
        fit_cell(&report.name, column_widths.profile),
        CLI_TABLE_GAP,
        fit_cell(active, column_widths.current),
        CLI_TABLE_GAP,
        fit_cell(&report.auth.label, column_widths.auth),
        CLI_TABLE_GAP,
        fit_cell(&view.email, column_widths.account),
        CLI_TABLE_GAP,
        fit_cell(&view.plan, column_widths.plan),
        CLI_TABLE_GAP,
        fit_cell(&view.main, column_widths.remaining),
        name_w = column_widths.profile,
        act_w = column_widths.current,
        auth_w = column_widths.auth,
        email_w = column_widths.account,
        plan_w = column_widths.plan,
        main_w = column_widths.remaining,
    )
}

fn push_wrapped_quota_report_line(section: &mut Vec<String>, line: &str, total_width: usize) {
    section.extend(
        wrap_text(line, total_width.saturating_sub(2).max(1))
            .into_iter()
            .map(|line| format!("  {line}")),
    );
}

fn append_quota_report_sections(
    output: &mut Vec<String>,
    sections: &[Vec<String>],
    max_lines: Option<usize>,
    start_profile: usize,
    interactive_scroll_hint: bool,
) -> usize {
    match max_lines {
        Some(max_lines) => append_limited_quota_report_sections(
            output,
            sections,
            max_lines,
            start_profile,
            interactive_scroll_hint,
        ),
        None => {
            output.extend(sections.iter().flatten().cloned());
            sections.len()
        }
    }
}

fn append_limited_quota_report_sections(
    output: &mut Vec<String>,
    sections: &[Vec<String>],
    max_lines: usize,
    start_profile: usize,
    interactive_scroll_hint: bool,
) -> usize {
    let total_profiles = sections.len();
    let mut shown_profiles = 0_usize;
    let mut remaining = max_lines.saturating_sub(output.len());

    for section in sections.iter().skip(start_profile) {
        if section.len() > remaining {
            break;
        }
        remaining = remaining.saturating_sub(section.len());
        output.extend(section.iter().cloned());
        shown_profiles += 1;
    }

    if let Some(notice) = quota_report_window_notice(
        start_profile,
        shown_profiles,
        total_profiles,
        interactive_scroll_hint,
    ) {
        if remaining == 0 && !output.is_empty() {
            output.pop();
        }
        output.push(String::new());
        output.push(notice);
    }

    shown_profiles
}

fn quota_report_window_notice(
    start_profile: usize,
    shown_profiles: usize,
    total_profiles: usize,
    interactive_scroll_hint: bool,
) -> Option<String> {
    let hidden_before = start_profile;
    let hidden_profiles = total_profiles.saturating_sub(shown_profiles);
    let hidden_after = hidden_profiles.saturating_sub(hidden_before);
    if hidden_before == 0 && hidden_after == 0 {
        return None;
    }

    Some(if interactive_scroll_hint {
        let first_visible = start_profile.saturating_add(1);
        let last_visible = start_profile.saturating_add(shown_profiles);
        format!(
            "press Up/Down to scroll profiles ({first_visible}-{last_visible} of {total_profiles}; {hidden_before} above, {hidden_after} below)"
        )
    } else {
        format!("showing top {shown_profiles} of {total_profiles} profiles due to terminal height")
    })
}

fn collect_quota_pool_aggregate(reports: &[QuotaReport]) -> QuotaPoolAggregate {
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
        let Some((five_hour, weekly)) = info_main_window_snapshots(usage) else {
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

fn render_quota_pool_summary_lines(
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

pub(crate) fn sort_quota_reports_for_display(reports: &[QuotaReport]) -> Vec<&QuotaReport> {
    let mut sorted = reports.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        quota_report_sort_key(left)
            .cmp(&quota_report_sort_key(right))
            .then_with(|| left.name.cmp(&right.name))
    });
    sorted
}

fn quota_report_sort_key(report: &QuotaReport) -> (usize, i64) {
    (
        quota_report_status_rank(report),
        quota_report_earliest_main_reset_epoch(report).unwrap_or(i64::MAX),
    )
}

fn quota_report_status_rank(report: &QuotaReport) -> usize {
    match &report.result {
        Ok(ProviderQuotaSnapshot::OpenAi(usage))
            if collect_blocked_limits(usage, false).is_empty() =>
        {
            0
        }
        Ok(ProviderQuotaSnapshot::OpenAi(_)) => 1,
        Ok(ProviderQuotaSnapshot::Copilot(info)) if copilot_quota_is_ready(info) => 0,
        Ok(ProviderQuotaSnapshot::Copilot(_)) => 1,
        Err(_) => 2,
    }
}

fn quota_report_earliest_main_reset_epoch(report: &QuotaReport) -> Option<i64> {
    match report.result.as_ref().ok()? {
        ProviderQuotaSnapshot::OpenAi(usage) => earliest_required_main_reset_epoch(usage),
        ProviderQuotaSnapshot::Copilot(info) => copilot_reset_epoch(info),
    }
}

fn earliest_required_main_reset_epoch(usage: &UsageResponse) -> Option<i64> {
    ["5h", "weekly"]
        .into_iter()
        .filter_map(|label| {
            find_main_window(usage.rate_limit.as_ref()?, label).and_then(|window| window.reset_at)
        })
        .min()
}

pub(crate) fn format_main_windows(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair)
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn format_main_windows_compact(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair_compact)
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn format_main_reset_summary(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_main_reset_pair)
        .unwrap_or_else(|| "5h unavailable | weekly unavailable".to_string())
}

fn format_main_reset_pair(rate_limit: &WindowPair) -> String {
    [
        format_main_reset_window(rate_limit, "5h"),
        format_main_reset_window(rate_limit, "weekly"),
    ]
    .join(" | ")
}

fn format_main_reset_window(rate_limit: &WindowPair, label: &str) -> String {
    match find_main_window(rate_limit, label) {
        Some(window) => {
            let reset = window
                .reset_at
                .map(|epoch| format_precise_reset_time(Some(epoch)))
                .unwrap_or_else(|| "unknown".to_string());
            format!("{label} {reset}")
        }
        None => format!("{label} unavailable"),
    }
}

fn format_window_pair(rate_limit: &WindowPair) -> String {
    let mut parts = Vec::new();
    if let Some(primary) = rate_limit.primary_window.as_ref() {
        parts.push(format_window_status(primary));
    }
    if let Some(secondary) = rate_limit.secondary_window.as_ref() {
        parts.push(format_window_status(secondary));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

fn format_window_pair_compact(rate_limit: &WindowPair) -> String {
    let mut parts = Vec::new();
    if let Some(primary) = rate_limit.primary_window.as_ref() {
        parts.push(format_window_status_compact(primary));
    }
    if let Some(secondary) = rate_limit.secondary_window.as_ref() {
        parts.push(format_window_status_compact(secondary));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

fn format_named_window_status(label: &str, window: &UsageWindow) -> String {
    format!("{label}: {}", format_window_details(window))
}

pub(crate) fn format_window_status(window: &UsageWindow) -> String {
    format_named_window_status(&window_label(window.limit_window_seconds), window)
}

pub(crate) fn format_window_status_compact(window: &UsageWindow) -> String {
    let label = window_label(window.limit_window_seconds);
    match window.used_percent {
        Some(used) => {
            let remaining = remaining_percent(Some(used));
            format!("{label} {remaining}% left")
        }
        None => format!("{label} ?"),
    }
}

fn format_window_details(window: &UsageWindow) -> String {
    let reset = format_reset_time(window.reset_at);
    match window.used_percent {
        Some(used) => {
            let remaining = remaining_percent(window.used_percent);
            format!("{remaining}% left ({used}% used), resets {reset}")
        }
        None => format!("usage unknown, resets {reset}"),
    }
}

pub(crate) fn collect_blocked_limits(
    usage: &UsageResponse,
    include_code_review: bool,
) -> Vec<BlockedLimit> {
    let mut blocked = Vec::new();

    if let Some(main) = usage.rate_limit.as_ref() {
        push_required_main_window(&mut blocked, main, "5h");
        push_required_main_window(&mut blocked, main, "weekly");
    } else {
        blocked.push(BlockedLimit {
            message: "5h quota unavailable".to_string(),
        });
        blocked.push(BlockedLimit {
            message: "weekly quota unavailable".to_string(),
        });
    }

    for additional in &usage.additional_rate_limits {
        let label = additional
            .limit_name
            .as_deref()
            .or(additional.metered_feature.as_deref());
        push_blocked_window(
            &mut blocked,
            label,
            additional.rate_limit.primary_window.as_ref(),
        );
        push_blocked_window(
            &mut blocked,
            label,
            additional.rate_limit.secondary_window.as_ref(),
        );
    }

    if include_code_review && let Some(code_review) = usage.code_review_rate_limit.as_ref() {
        push_blocked_window(
            &mut blocked,
            Some("code-review"),
            code_review.primary_window.as_ref(),
        );
        push_blocked_window(
            &mut blocked,
            Some("code-review"),
            code_review.secondary_window.as_ref(),
        );
    }

    blocked
}

fn push_required_main_window(
    blocked: &mut Vec<BlockedLimit>,
    main: &WindowPair,
    required_label: &str,
) {
    let Some(window) = find_main_window(main, required_label) else {
        blocked.push(BlockedLimit {
            message: format!("{required_label} quota unavailable"),
        });
        return;
    };

    match window.used_percent {
        Some(used) if used < 100 => {}
        Some(_) => blocked.push(BlockedLimit {
            message: format!(
                "{required_label} exhausted until {}",
                format_reset_time(window.reset_at)
            ),
        }),
        None => blocked.push(BlockedLimit {
            message: format!("{required_label} quota unknown"),
        }),
    }
}

pub(crate) fn find_main_window<'a>(
    main: &'a WindowPair,
    expected_label: &str,
) -> Option<&'a UsageWindow> {
    [main.primary_window.as_ref(), main.secondary_window.as_ref()]
        .into_iter()
        .flatten()
        .find(|window| window_label(window.limit_window_seconds) == expected_label)
}

fn push_blocked_window(
    blocked: &mut Vec<BlockedLimit>,
    name: Option<&str>,
    window: Option<&UsageWindow>,
) {
    let Some(window) = window else {
        return;
    };
    let Some(used) = window.used_percent else {
        return;
    };
    if used < 100 {
        return;
    }

    let label = match name {
        Some(base) if !base.is_empty() => {
            format!("{base} {}", window_label(window.limit_window_seconds))
        }
        _ => window_label(window.limit_window_seconds),
    };

    blocked.push(BlockedLimit {
        message: format!(
            "{label} exhausted until {}",
            format_reset_time(window.reset_at)
        ),
    });
}

pub(crate) fn format_blocked_limits(blocked: &[BlockedLimit]) -> String {
    blocked
        .iter()
        .map(|limit| limit.message.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn remaining_percent(used_percent: Option<i64>) -> i64 {
    let Some(used) = used_percent else {
        return 0;
    };
    (100 - used).clamp(0, 100)
}

pub(crate) fn window_label(seconds: Option<i64>) -> String {
    let Some(seconds) = seconds else {
        return "usage".to_string();
    };

    if (17_700..=18_300).contains(&seconds) {
        return "5h".to_string();
    }
    if (601_200..=608_400).contains(&seconds) {
        return "weekly".to_string();
    }
    if (2_505_600..=2_678_400).contains(&seconds) {
        return "monthly".to_string();
    }

    format!("{seconds}s")
}

pub(crate) fn format_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M %:z")
}

pub(crate) fn format_precise_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M:%S %:z")
}

fn format_quota_snapshot_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M:%S %:z")
}

fn format_local_epoch(epoch: Option<i64>, pattern: &str) -> String {
    let Some(epoch) = epoch else {
        return "-".to_string();
    };

    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|dt| dt.format(pattern).to_string())
        .unwrap_or_else(|| epoch.to_string())
}

fn display_optional(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn copilot_quota_feature_labels() -> [(&'static str, &'static str); 2] {
    [("chat", "chat"), ("completions", "comp")]
}

fn copilot_remaining_quota(info: &CopilotUserInfo, feature: &str) -> Option<i64> {
    info.limited_user_quotas
        .get(feature)
        .copied()
        .or_else(|| info.monthly_quotas.get(feature).copied())
}

fn copilot_total_quota(info: &CopilotUserInfo, feature: &str) -> Option<i64> {
    info.monthly_quotas.get(feature).copied()
}

fn copilot_blocked_features(info: &CopilotUserInfo) -> Vec<String> {
    [("chat", "chat"), ("completions", "completions")]
        .into_iter()
        .filter_map(|(feature, label)| {
            (copilot_remaining_quota(info, feature).unwrap_or(1) <= 0).then_some(label.to_string())
        })
        .collect()
}

fn copilot_quota_is_ready(info: &CopilotUserInfo) -> bool {
    copilot_blocked_features(info).is_empty()
}

pub(crate) fn format_copilot_quota_status(info: &CopilotUserInfo) -> String {
    let blocked = copilot_blocked_features(info);
    if blocked.is_empty() {
        "Ready".to_string()
    } else {
        format!(
            "Blocked ({})",
            blocked
                .into_iter()
                .map(|feature| format!("{feature} exhausted"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

pub(crate) fn format_copilot_main_quota(info: &CopilotUserInfo) -> String {
    let parts = copilot_quota_feature_labels()
        .into_iter()
        .filter_map(|(feature, label)| {
            let remaining = copilot_remaining_quota(info, feature)?;
            Some(match copilot_total_quota(info, feature) {
                Some(total) => format!("{label} {remaining}/{total} left"),
                None => format!("{label} {remaining} left"),
            })
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

fn copilot_reset_epoch(info: &CopilotUserInfo) -> Option<i64> {
    let date =
        chrono::NaiveDate::parse_from_str(info.limited_user_reset_date.as_deref()?, "%Y-%m-%d")
            .ok()?;
    let datetime = date.and_hms_opt(0, 0, 0)?;
    Local
        .from_local_datetime(&datetime)
        .earliest()
        .map(|value| value.timestamp())
}

pub(crate) fn format_copilot_reset_summary(info: &CopilotUserInfo) -> Option<String> {
    Some(format!(
        "monthly {}",
        info.limited_user_reset_date.as_deref()?.trim()
    ))
}

pub(crate) fn render_profile_quota(profile_name: &str, usage: &UsageResponse) -> String {
    let blocked = collect_blocked_limits(usage, false);
    let status = if blocked.is_empty() {
        "Ready".to_string()
    } else {
        format!("Blocked ({})", format_blocked_limits(&blocked))
    };
    let mut panel = PanelBuilder::new(format!("Quota {profile_name}"));
    panel.push("Profile", profile_name);
    panel.push("Account", display_optional(usage.email.as_deref()));
    panel.push("Plan", display_optional(usage.plan_type.as_deref()));
    panel.push("Status", status);
    panel.push("Main", format_main_windows(usage));

    if let Some(code_review) = usage.code_review_rate_limit.as_ref() {
        panel.push("Code review", format_window_pair(code_review));
    }

    panel.extend(format_additional_limits(usage));
    panel.render()
}

pub(crate) fn render_profile_quota_snapshot(
    profile_name: &str,
    snapshot: &ProviderQuotaSnapshot,
) -> String {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => render_profile_quota(profile_name, usage),
        ProviderQuotaSnapshot::Copilot(info) => render_profile_copilot_quota(profile_name, info),
    }
}

fn render_profile_copilot_quota(profile_name: &str, info: &CopilotUserInfo) -> String {
    let mut panel = PanelBuilder::new(format!("Quota {profile_name}"));
    panel.push("Profile", profile_name);
    panel.push("Account", display_optional(info.login.as_deref()));
    panel.push(
        "Plan",
        display_optional(
            info.copilot_plan
                .as_deref()
                .or(info.access_type_sku.as_deref()),
        ),
    );
    if let Some(access_type) = info.access_type_sku.as_deref()
        && info.copilot_plan.as_deref() != Some(access_type)
    {
        panel.push("Access", access_type);
    }
    panel.push("Status", format_copilot_quota_status(info));
    panel.push("Main", format_copilot_main_quota(info));
    if let Some(reset) = format_copilot_reset_summary(info) {
        panel.push("Reset", reset);
    }
    panel.render()
}

fn format_additional_limits(usage: &UsageResponse) -> Vec<(String, String)> {
    let mut lines = Vec::new();

    for additional in &usage.additional_rate_limits {
        let name = additional
            .limit_name
            .as_deref()
            .or(additional.metered_feature.as_deref())
            .unwrap_or("Additional");

        if let Some(primary) = additional.rate_limit.primary_window.as_ref() {
            lines.push((
                additional_window_label(name, primary),
                format_window_details(primary),
            ));
        }
        if let Some(secondary) = additional.rate_limit.secondary_window.as_ref() {
            lines.push((
                additional_window_label(name, secondary),
                format_window_details(secondary),
            ));
        }
    }

    lines
}

fn additional_window_label(base: &str, window: &UsageWindow) -> String {
    format!("{base} {}", window_label(window.limit_window_seconds))
}

pub(crate) fn first_line_of_error(input: &str) -> String {
    input
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("-")
        .trim()
        .to_string()
}

pub(super) fn render_quota_watch_error_panel(title: &str, message: &str) -> String {
    let mut panel = PanelBuilder::new(title);
    panel.push("Error", first_line_of_error(message));
    panel.render()
}

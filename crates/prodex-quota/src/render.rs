use chrono::{Local, TimeZone};
use std::collections::{BTreeMap, BTreeSet};
use terminal_ui::{
    CLI_LABEL_WIDTH, CLI_TABLE_GAP, current_cli_width, fit_cell, format_field_lines_with_layout,
    panel_label_width, section_header_with_width, text_width, wrap_text,
};

use super::{
    BlockedLimit, CopilotQuotaInfo, MainWindowSnapshot, ProviderQuotaSnapshot, QuotaReport,
    RenderedQuotaReportWindow, RuntimeQuotaPressureBand, RuntimeQuotaSummary,
    RuntimeQuotaWindowStatus, RuntimeQuotaWindowSummary, UsageResponse, UsageWindow, WindowPair,
};

pub fn main_window_snapshots(
    usage: &UsageResponse,
) -> Option<(MainWindowSnapshot, MainWindowSnapshot)> {
    Some((
        required_main_window_snapshot(usage, "5h")?,
        required_main_window_snapshot(usage, "weekly")?,
    ))
}

pub fn required_main_window_snapshot(
    usage: &UsageResponse,
    label: &str,
) -> Option<MainWindowSnapshot> {
    required_main_window_snapshot_at(usage, label, Local::now().timestamp())
}

pub fn required_main_window_snapshot_at(
    usage: &UsageResponse,
    label: &str,
    now: i64,
) -> Option<MainWindowSnapshot> {
    let window = find_main_window(usage.rate_limit.as_ref()?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - now).max(0)
    };
    let pressure_score = seconds_until_reset
        .saturating_mul(1_000)
        .checked_div(remaining_percent.max(1))
        .unwrap_or(i64::MAX);

    Some(MainWindowSnapshot {
        remaining_percent,
        reset_at,
        pressure_score,
    })
}

pub fn quota_window_summary(usage: &UsageResponse, label: &str) -> RuntimeQuotaWindowSummary {
    let Some(window) = required_main_window_snapshot(usage, label) else {
        return RuntimeQuotaWindowSummary {
            status: RuntimeQuotaWindowStatus::Unknown,
            remaining_percent: 0,
            reset_at: i64::MAX,
        };
    };

    let status = if window.remaining_percent == 0 {
        RuntimeQuotaWindowStatus::Exhausted
    } else if window.remaining_percent <= 5 {
        RuntimeQuotaWindowStatus::Critical
    } else if window.remaining_percent <= 15 {
        RuntimeQuotaWindowStatus::Thin
    } else {
        RuntimeQuotaWindowStatus::Ready
    };

    RuntimeQuotaWindowSummary {
        status,
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

pub fn quota_summary(usage: &UsageResponse) -> RuntimeQuotaSummary {
    let five_hour = quota_window_summary(usage, "5h");
    let weekly = quota_window_summary(usage, "weekly");
    RuntimeQuotaSummary {
        five_hour,
        weekly,
        route_band: quota_pressure_band_from_windows(five_hour, weekly),
    }
}

pub fn quota_pressure_band_from_windows(
    five_hour: RuntimeQuotaWindowSummary,
    weekly: RuntimeQuotaWindowSummary,
) -> RuntimeQuotaPressureBand {
    [five_hour.status, weekly.status]
        .into_iter()
        .map(quota_pressure_band_from_window_status)
        .max()
        .unwrap_or(RuntimeQuotaPressureBand::Unknown)
}

pub fn quota_pressure_band_from_window_status(
    status: RuntimeQuotaWindowStatus,
) -> RuntimeQuotaPressureBand {
    match status {
        RuntimeQuotaWindowStatus::Ready => RuntimeQuotaPressureBand::Healthy,
        RuntimeQuotaWindowStatus::Thin => RuntimeQuotaPressureBand::Thin,
        RuntimeQuotaWindowStatus::Critical => RuntimeQuotaPressureBand::Critical,
        RuntimeQuotaWindowStatus::Exhausted => RuntimeQuotaPressureBand::Exhausted,
        RuntimeQuotaWindowStatus::Unknown => RuntimeQuotaPressureBand::Unknown,
    }
}

pub fn quota_reset_at_from_message(message: &str) -> Option<i64> {
    let marker = message.to_ascii_lowercase().find("try again at ")?;
    let candidate = message
        .get(marker + "try again at ".len()..)?
        .trim()
        .trim_end_matches('.');
    let now = Local::now();
    if let Some((time_text, meridiem)) = candidate
        .split_whitespace()
        .collect::<Vec<_>>()
        .get(..2)
        .and_then(|parts| (parts.len() == 2).then_some((parts[0], parts[1])))
        && let Ok(time) =
            chrono::NaiveTime::parse_from_str(&format!("{time_text} {meridiem}"), "%I:%M %p")
    {
        let mut naive = now.date_naive().and_time(time);
        let mut parsed = Local
            .from_local_datetime(&naive)
            .single()
            .or_else(|| Local.from_local_datetime(&naive).earliest())?;
        if parsed.timestamp() <= now.timestamp() {
            naive = naive.checked_add_signed(chrono::Duration::days(1))?;
            parsed = Local
                .from_local_datetime(&naive)
                .single()
                .or_else(|| Local.from_local_datetime(&naive).earliest())?;
        }
        return Some(parsed.timestamp());
    }

    let mut parts = candidate
        .split_whitespace()
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    if parts.len() < 5 {
        return None;
    }
    let day_digits = parts[1]
        .trim_end_matches(',')
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if day_digits.is_empty() {
        return None;
    }
    parts[1] = format!("{day_digits},");
    let normalized = parts[..5].join(" ");
    let naive = chrono::NaiveDateTime::parse_from_str(&normalized, "%b %d, %Y %I:%M %p").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .map(|datetime| datetime.timestamp())
}

pub fn earliest_required_main_reset_epoch(usage: &UsageResponse) -> Option<i64> {
    ["5h", "weekly"]
        .into_iter()
        .filter_map(|label| {
            find_main_window(usage.rate_limit.as_ref()?, label).and_then(|window| window.reset_at)
        })
        .min()
}

pub fn format_main_windows(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair)
        .unwrap_or_else(|| "-".to_string())
}

pub fn format_main_windows_compact(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair_compact)
        .unwrap_or_else(|| "-".to_string())
}

pub fn format_main_reset_summary(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_main_reset_pair)
        .unwrap_or_else(|| "5h unavailable | weekly unavailable".to_string())
}

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

pub fn render_quota_reports(reports: &[QuotaReport], detail: bool) -> String {
    render_quota_reports_with_layout(reports, detail, None, current_cli_width())
}

pub fn render_quota_reports_with_line_limit(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
) -> String {
    render_quota_reports_with_layout(reports, detail, max_lines, current_cli_width())
}

pub fn render_quota_reports_with_layout(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
    total_width: usize,
) -> String {
    render_quota_reports_window_with_layout(reports, detail, max_lines, total_width, 0, false)
        .output
}

pub fn render_quota_reports_window_with_layout(
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
    let duplicate_workspace_emails = quota_report_duplicate_workspace_emails(reports);
    sort_quota_reports_for_display(reports)
        .into_iter()
        .map(|report| {
            render_quota_report_section(
                report,
                column_widths,
                detail,
                total_width,
                &duplicate_workspace_emails,
            )
        })
        .collect()
}

fn render_quota_report_section(
    report: &QuotaReport,
    column_widths: QuotaReportColumnWidths,
    detail: bool,
    total_width: usize,
    duplicate_workspace_emails: &BTreeSet<String>,
) -> Vec<String> {
    let view = quota_report_view_data(report);
    let mut section = vec![render_quota_report_row(report, column_widths, &view)];
    if detail && quota_report_should_show_workspace(report, duplicate_workspace_emails) {
        push_wrapped_quota_report_line(
            &mut section,
            &format!("workspace: {}", quota_report_workspace_label(report)),
            total_width,
        );
    }
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

fn quota_report_duplicate_workspace_emails(reports: &[QuotaReport]) -> BTreeSet<String> {
    let mut workspace_ids_by_email = BTreeMap::<String, BTreeSet<String>>::new();
    for report in reports {
        let Some(email) = quota_report_openai_email(report) else {
            continue;
        };
        let Some(workspace_id) = report
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|workspace_id| !workspace_id.is_empty())
        else {
            continue;
        };
        workspace_ids_by_email
            .entry(normalize_email(email))
            .or_default()
            .insert(workspace_id.to_string());
    }
    workspace_ids_by_email
        .into_iter()
        .filter_map(|(email, workspace_ids)| (workspace_ids.len() > 1).then_some(email))
        .collect()
}

fn quota_report_openai_email(report: &QuotaReport) -> Option<&str> {
    match report.result.as_ref().ok()? {
        ProviderQuotaSnapshot::OpenAi(usage) => usage
            .email
            .as_deref()
            .map(str::trim)
            .filter(|email| !email.is_empty()),
        ProviderQuotaSnapshot::Copilot(_) => None,
    }
}

fn quota_report_should_show_workspace(
    report: &QuotaReport,
    duplicate_workspace_emails: &BTreeSet<String>,
) -> bool {
    quota_report_openai_email(report)
        .map(normalize_email)
        .is_some_and(|email| duplicate_workspace_emails.contains(&email))
}

fn quota_report_workspace_label(report: &QuotaReport) -> String {
    report
        .workspace_id
        .as_deref()
        .map(str::trim)
        .filter(|workspace_id| !workspace_id.is_empty())
        .map(short_workspace_id)
        .unwrap_or_else(|| "unknown".to_string())
}

fn short_workspace_id(workspace_id: &str) -> String {
    let chars = workspace_id.chars().collect::<Vec<_>>();
    if chars.len() <= 24 {
        return workspace_id.to_string();
    }
    let prefix = chars.iter().take(12).collect::<String>();
    let suffix = chars
        .iter()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{prefix}...{suffix}")
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
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

pub fn sort_quota_reports_for_display(reports: &[QuotaReport]) -> Vec<&QuotaReport> {
    sorted_quota_report_indexes(reports)
        .into_iter()
        .map(|index| &reports[index])
        .collect()
}

pub fn sorted_quota_report_indexes(reports: &[QuotaReport]) -> Vec<usize> {
    let mut sorted = (0..reports.len()).collect::<Vec<_>>();
    sorted.sort_by(|&left, &right| {
        quota_report_sort_key(&reports[left])
            .cmp(&quota_report_sort_key(&reports[right]))
            .then_with(|| reports[left].name.cmp(&reports[right].name))
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

fn copilot_quota_feature_labels() -> [(&'static str, &'static str); 2] {
    [("chat", "chat"), ("completions", "comp")]
}

fn copilot_remaining_quota(info: &CopilotQuotaInfo, feature: &str) -> Option<i64> {
    info.limited_user_quotas
        .get(feature)
        .copied()
        .or_else(|| info.monthly_quotas.get(feature).copied())
}

fn copilot_total_quota(info: &CopilotQuotaInfo, feature: &str) -> Option<i64> {
    info.monthly_quotas.get(feature).copied()
}

fn copilot_blocked_features(info: &CopilotQuotaInfo) -> Vec<String> {
    [("chat", "chat"), ("completions", "completions")]
        .into_iter()
        .filter_map(|(feature, label)| {
            (copilot_remaining_quota(info, feature).unwrap_or(1) <= 0).then_some(label.to_string())
        })
        .collect()
}

pub fn copilot_quota_is_ready(info: &CopilotQuotaInfo) -> bool {
    copilot_blocked_features(info).is_empty()
}

pub fn format_copilot_quota_status(info: &CopilotQuotaInfo) -> String {
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

pub fn format_copilot_main_quota(info: &CopilotQuotaInfo) -> String {
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

fn copilot_reset_epoch(info: &CopilotQuotaInfo) -> Option<i64> {
    let date =
        chrono::NaiveDate::parse_from_str(info.limited_user_reset_date.as_deref()?, "%Y-%m-%d")
            .ok()?;
    let datetime = date.and_hms_opt(0, 0, 0)?;
    Local
        .from_local_datetime(&datetime)
        .earliest()
        .map(|value| value.timestamp())
}

pub fn format_copilot_reset_summary(info: &CopilotQuotaInfo) -> Option<String> {
    Some(format!(
        "monthly {}",
        info.limited_user_reset_date.as_deref()?.trim()
    ))
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

pub fn format_window_status(window: &UsageWindow) -> String {
    format_named_window_status(&window_label(window.limit_window_seconds), window)
}

pub fn format_window_status_compact(window: &UsageWindow) -> String {
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

pub fn collect_blocked_limits(
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

pub fn find_main_window<'a>(main: &'a WindowPair, expected_label: &str) -> Option<&'a UsageWindow> {
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

pub fn format_blocked_limits(blocked: &[BlockedLimit]) -> String {
    blocked
        .iter()
        .map(|limit| limit.message.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn remaining_percent(used_percent: Option<i64>) -> i64 {
    let Some(used) = used_percent else {
        return 0;
    };
    (100 - used).clamp(0, 100)
}

pub fn window_label(seconds: Option<i64>) -> String {
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

pub fn format_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M %:z")
}

pub fn format_precise_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M:%S %:z")
}

pub fn format_quota_snapshot_time(epoch: Option<i64>) -> String {
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

pub fn render_profile_quota(profile_name: &str, usage: &UsageResponse) -> String {
    render_profile_quota_with_width(profile_name, usage, current_cli_width())
}

pub fn render_profile_quota_with_width(
    profile_name: &str,
    usage: &UsageResponse,
    total_width: usize,
) -> String {
    let blocked = collect_blocked_limits(usage, false);
    let status = if blocked.is_empty() {
        "Ready".to_string()
    } else {
        format!("Blocked ({})", format_blocked_limits(&blocked))
    };
    let mut fields = vec![
        ("Profile".to_string(), profile_name.to_string()),
        (
            "Account".to_string(),
            display_optional(usage.email.as_deref()).to_string(),
        ),
        (
            "Plan".to_string(),
            display_optional(usage.plan_type.as_deref()).to_string(),
        ),
        ("Status".to_string(), status),
        ("Main".to_string(), format_main_windows(usage)),
    ];

    if let Some(code_review) = usage.code_review_rate_limit.as_ref() {
        fields.push(("Code review".to_string(), format_window_pair(code_review)));
    }

    fields.extend(format_additional_limits(usage));
    render_panel_with_width(&format!("Quota {profile_name}"), &fields, total_width)
}

pub fn render_profile_quota_snapshot(
    profile_name: &str,
    snapshot: &ProviderQuotaSnapshot,
) -> String {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => render_profile_quota(profile_name, usage),
        ProviderQuotaSnapshot::Copilot(info) => render_profile_copilot_quota(profile_name, info),
    }
}

pub fn render_profile_copilot_quota(profile_name: &str, info: &CopilotQuotaInfo) -> String {
    render_profile_copilot_quota_with_width(profile_name, info, current_cli_width())
}

pub fn render_profile_copilot_quota_with_width(
    profile_name: &str,
    info: &CopilotQuotaInfo,
    total_width: usize,
) -> String {
    let mut fields = vec![
        ("Profile".to_string(), profile_name.to_string()),
        (
            "Account".to_string(),
            display_optional(info.login.as_deref()).to_string(),
        ),
        (
            "Plan".to_string(),
            display_optional(
                info.copilot_plan
                    .as_deref()
                    .or(info.access_type_sku.as_deref()),
            )
            .to_string(),
        ),
    ];
    if let Some(access_type) = info.access_type_sku.as_deref()
        && info.copilot_plan.as_deref() != Some(access_type)
    {
        fields.push(("Access".to_string(), access_type.to_string()));
    }
    fields.push(("Status".to_string(), format_copilot_quota_status(info)));
    fields.push(("Main".to_string(), format_copilot_main_quota(info)));
    if let Some(reset) = format_copilot_reset_summary(info) {
        fields.push(("Reset".to_string(), reset));
    }
    render_panel_with_width(&format!("Quota {profile_name}"), &fields, total_width)
}

pub fn render_quota_error_panel(title: &str, message: &str) -> String {
    render_quota_error_panel_with_width(title, message, current_cli_width())
}

pub fn render_quota_error_panel_with_width(
    title: &str,
    message: &str,
    total_width: usize,
) -> String {
    render_panel_with_width(
        title,
        &[("Error".to_string(), first_line_of_error(message))],
        total_width,
    )
}

fn render_panel_with_width(title: &str, fields: &[(String, String)], total_width: usize) -> String {
    let label_width = panel_label_width(fields, total_width);
    let mut lines = vec![section_header_with_width(title, total_width)];
    for (label, value) in fields {
        lines.extend(format_field_lines_with_layout(
            label,
            value,
            total_width,
            label_width,
        ));
    }
    lines.join("\n")
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

pub fn first_line_of_error(input: &str) -> String {
    input
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("-")
        .trim()
        .to_string()
}

#[cfg(test)]
#[path = "../tests/src/render.rs"]
mod tests;

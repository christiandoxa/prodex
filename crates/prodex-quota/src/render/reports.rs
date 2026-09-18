use super::*;
use std::cmp::Ordering;

#[derive(Debug)]
struct QuotaReportViewData {
    account: String,
    plan: String,
    remaining: String,
    status: String,
    resets: Option<String>,
}

pub fn render_quota_reports(reports: &[QuotaReport], detail: bool) -> String {
    render_quota_reports_with_layout(reports, detail, None, current_cli_width())
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
    render_quota_reports_window_with_sort(
        reports,
        detail,
        max_lines,
        total_width,
        start_profile,
        interactive_scroll_hint,
        QuotaReportSort::Remaining,
    )
}

pub fn render_quota_reports_window_with_sort(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
    total_width: usize,
    start_profile: usize,
    interactive_scroll_hint: bool,
    sort: QuotaReportSort,
) -> RenderedQuotaReportWindow {
    let sorted = sort_quota_reports_for_display_with_sort(reports, sort);
    let total_profiles = sorted.len();
    let start_profile = start_profile.min(total_profiles.saturating_sub(1));
    let header = if total_width < 60 {
        "PROFILE · AUTH · STATUS"
    } else {
        "PROFILE · AUTH · ACCOUNT · PLAN · STATUS · REMAINING"
    };
    let mut lines = vec![
        section_header_with_width("Quota Overview", total_width),
        header.to_string(),
    ];
    for (label, value) in quota_pool_summary_fields(reports) {
        lines.extend(wrap_text(&format!("{label}: {value}"), total_width.max(1)));
    }

    let header_lines = lines.len();
    let mut shown_profiles = 0usize;
    for report in sorted.iter().skip(start_profile) {
        let section = render_quota_report_section(report, detail, total_width);
        let reserve_notice =
            usize::from(start_profile > 0 || start_profile + shown_profiles + 1 < total_profiles);
        if max_lines.is_some_and(|limit| {
            lines
                .len()
                .saturating_add(section.len())
                .saturating_add(reserve_notice)
                > limit
        }) {
            break;
        }
        lines.extend(section);
        shown_profiles += 1;
    }

    let hidden_before = start_profile;
    let hidden_after = total_profiles.saturating_sub(start_profile.saturating_add(shown_profiles));
    if (hidden_before > 0 || hidden_after > 0) && max_lines.is_none_or(|limit| lines.len() < limit)
    {
        lines.push(if interactive_scroll_hint {
            format!(
                "profiles {}-{} of {total_profiles}; {hidden_before} above, {hidden_after} below",
                start_profile.saturating_add(1),
                start_profile.saturating_add(shown_profiles)
            )
        } else {
            format!(
                "showing {shown_profiles} of {total_profiles} profile(s) due to terminal height"
            )
        });
    }
    if lines.len() == header_lines && total_profiles == 0 {
        lines.push("No quota profiles.".to_string());
    }
    if let Some(limit) = max_lines {
        lines.truncate(limit);
    }

    RenderedQuotaReportWindow {
        output: lines.join("\n"),
        shown_profiles,
        total_profiles,
        start_profile,
        hidden_before,
        hidden_after,
    }
}

fn render_quota_report_section(
    report: &QuotaReport,
    detail: bool,
    total_width: usize,
) -> Vec<String> {
    let view = quota_report_view_data(report);
    let active = if report.active { "*" } else { " " };
    let summary = format!(
        "{active} {} · {} · {} · {} · {} · {}",
        report.name, report.auth.label, view.account, view.plan, view.status, view.remaining
    );
    let mut lines = wrap_text(&summary, total_width.max(1));
    if detail {
        if let Some(workspace) = quota_report_workspace_label(report) {
            lines.extend(wrap_text(
                &format!("  workspace: {workspace}"),
                total_width.max(1),
            ));
        }
        if let Some(resets) = view.resets.as_deref() {
            lines.extend(wrap_text(resets, total_width.max(1)));
        }
        if let Ok(ProviderQuotaSnapshot::OpenAi(usage)) = &report.result {
            for line in format_openai_additional_limit_summaries(usage) {
                lines.extend(wrap_text(&format!("  {line}"), total_width.max(1)));
            }
        }
    }
    lines
}

fn quota_report_view_data(report: &QuotaReport) -> QuotaReportViewData {
    match &report.result {
        Ok(ProviderQuotaSnapshot::OpenAi(usage)) => QuotaReportViewData {
            account: display_optional(usage.email.as_deref()).to_string(),
            plan: display_optional(usage.plan_type.as_deref()).to_string(),
            remaining: format_main_windows_compact(usage),
            status: format_openai_quota_status(usage),
            resets: Some(match usage.rate_limit_reset_credits.as_ref() {
                Some(credits) => format!(
                    "resets: {}; reset credits: {} available",
                    format_main_reset_summary(usage),
                    credits.available_count
                ),
                None => format!("resets: {}", format_main_reset_summary(usage)),
            }),
        },
        Ok(ProviderQuotaSnapshot::Copilot(info)) => QuotaReportViewData {
            account: display_optional(info.login.as_deref()).to_string(),
            plan: display_optional(
                info.copilot_plan
                    .as_deref()
                    .or(info.access_type_sku.as_deref()),
            )
            .to_string(),
            remaining: format_copilot_main_quota(info),
            status: format_copilot_quota_status(info),
            resets: format_copilot_reset_summary(info).map(|reset| format!("resets: {reset}")),
        },
        Ok(ProviderQuotaSnapshot::Gemini(info)) => QuotaReportViewData {
            account: display_optional(info.email.as_deref()).to_string(),
            plan: display_optional(info.plan.as_deref()).to_string(),
            remaining: format_gemini_main_quota(info),
            status: format_gemini_quota_status(info),
            resets: format_gemini_reset_summary(info).map(|reset| format!("resets: {reset}")),
        },
        Ok(ProviderQuotaSnapshot::External(info)) => QuotaReportViewData {
            account: display_optional(info.account.as_deref()).to_string(),
            plan: display_optional(info.plan.as_deref()).to_string(),
            remaining: info.main.clone(),
            status: info.status.clone(),
            resets: info.reset.as_ref().map(|reset| format!("resets: {reset}")),
        },
        Err(error) => QuotaReportViewData {
            account: "-".to_string(),
            plan: "-".to_string(),
            remaining: "-".to_string(),
            status: format_quota_error_status(error),
            resets: Some(format_quota_error_detail(error)),
        },
    }
}

fn quota_report_workspace_label(report: &QuotaReport) -> Option<String> {
    report
        .workspace_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            report
                .workspace_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(short_workspace_id)
        })
}

fn short_workspace_id(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= 24 {
        return value.to_string();
    }
    format!(
        "{}...{}",
        chars.iter().take(12).collect::<String>(),
        chars.iter().skip(chars.len() - 6).collect::<String>()
    )
}

pub fn format_openai_additional_limit_summaries(usage: &UsageResponse) -> Vec<String> {
    usage
        .additional_rate_limits
        .iter()
        .filter_map(|additional| {
            let name = additional
                .limit_name
                .as_deref()
                .or(additional.metered_feature.as_deref())
                .or(additional.limit_id.as_deref())
                .unwrap_or("Additional");
            let remaining = format_window_pair_compact(&additional.rate_limit);
            (remaining != "-").then(|| {
                format!(
                    "{name}: {remaining}; resets: {}",
                    format_additional_reset_pair(&additional.rate_limit)
                )
            })
        })
        .collect()
}

fn format_additional_reset_pair(rate_limit: &WindowPair) -> String {
    [
        rate_limit.primary_window.as_ref(),
        rate_limit.secondary_window.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(|window| {
        format!(
            "{} {}",
            window_label(window.limit_window_seconds),
            format_precise_reset_time(window.reset_at)
        )
    })
    .collect::<Vec<_>>()
    .join(" | ")
}

pub fn sort_quota_reports_for_display_with_sort(
    reports: &[QuotaReport],
    sort: QuotaReportSort,
) -> Vec<&QuotaReport> {
    sorted_quota_report_indexes_by(reports, sort)
        .into_iter()
        .map(|index| &reports[index])
        .collect()
}

pub fn sorted_quota_report_indexes(reports: &[QuotaReport]) -> Vec<usize> {
    sorted_quota_report_indexes_by(reports, QuotaReportSort::Remaining)
}

pub fn sorted_quota_report_indexes_by(
    reports: &[QuotaReport],
    sort: QuotaReportSort,
) -> Vec<usize> {
    let mut indexes = (0..reports.len()).collect::<Vec<_>>();
    indexes.sort_by(|&left, &right| {
        compare_quota_reports(&reports[left], &reports[right], sort)
            .then_with(|| reports[left].name.cmp(&reports[right].name))
    });
    indexes
}

fn compare_quota_reports(
    left: &QuotaReport,
    right: &QuotaReport,
    sort: QuotaReportSort,
) -> Ordering {
    match sort {
        QuotaReportSort::Current => (!left.active)
            .cmp(&(!right.active))
            .then_with(|| quota_status_rank(left).cmp(&quota_status_rank(right))),
        QuotaReportSort::Remaining => quota_status_rank(left)
            .cmp(&quota_status_rank(right))
            .then_with(|| quota_reset_epoch(left).cmp(&quota_reset_epoch(right))),
        QuotaReportSort::Profile => compare_text(&left.name, &right.name),
        QuotaReportSort::Auth => compare_text(&left.auth.label, &right.auth.label),
        QuotaReportSort::Account => compare_text(
            &quota_report_view_data(left).account,
            &quota_report_view_data(right).account,
        ),
        QuotaReportSort::Plan => compare_text(
            &quota_report_view_data(left).plan,
            &quota_report_view_data(right).plan,
        ),
    }
}

fn compare_text(left: &str, right: &str) -> Ordering {
    left.trim()
        .to_ascii_lowercase()
        .cmp(&right.trim().to_ascii_lowercase())
}

fn quota_status_rank(report: &QuotaReport) -> usize {
    match report.result.as_ref() {
        Ok(ProviderQuotaSnapshot::OpenAi(usage)) => {
            usize::from(!openai_quota_has_ready_limit(usage))
        }
        Ok(ProviderQuotaSnapshot::Copilot(info)) => usize::from(!copilot_quota_is_ready(info)),
        Ok(ProviderQuotaSnapshot::Gemini(info)) => usize::from(!gemini_quota_is_ready(info)),
        Ok(ProviderQuotaSnapshot::External(info)) => usize::from(info.available != Some(true)),
        Err(_) => 2,
    }
}

fn quota_reset_epoch(report: &QuotaReport) -> i64 {
    match report.result.as_ref().ok() {
        Some(ProviderQuotaSnapshot::OpenAi(usage)) => {
            earliest_required_main_reset_epoch(usage).unwrap_or(i64::MAX)
        }
        Some(ProviderQuotaSnapshot::Copilot(info)) => copilot_reset_epoch(info).unwrap_or(i64::MAX),
        Some(ProviderQuotaSnapshot::Gemini(info)) => gemini_reset_epoch(info).unwrap_or(i64::MAX),
        _ => i64::MAX,
    }
}

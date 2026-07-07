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
    status: usize,
    remaining: usize,
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
    let column_widths = quota_report_column_widths(total_width);
    let pool_summary =
        render_quota_pool_summary_lines(&collect_quota_pool_aggregate(reports), total_width);
    let sections = build_quota_report_sections(reports, column_widths, detail, total_width, sort);
    let header = render_quota_report_header(column_widths);
    let mut output = render_quota_report_window_header(total_width, &pool_summary, &header);
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
    const MIN_WIDTHS: [usize; 7] = [12, 3, 4, 14, 4, 8, 13];
    const EXTRA_WEIGHTS: [usize; 7] = [12, 1, 3, 13, 4, 8, 18];
    const DISTRIBUTION_ORDER: [usize; 7] = [6, 3, 0, 5, 4, 2, 1];

    let min_total = MIN_WIDTHS.iter().sum::<usize>();
    let available = total_width
        .saturating_sub(text_width(CLI_TABLE_GAP) * 6)
        .max(MIN_WIDTHS.len());

    if available < min_total {
        let mut widths = [1; 7];
        for index in DISTRIBUTION_ORDER
            .into_iter()
            .cycle()
            .take(available.saturating_sub(widths.len()))
        {
            widths[index] += 1;
        }
        return QuotaReportColumnWidths {
            profile: widths[0],
            current: widths[1],
            auth: widths[2],
            account: widths[3],
            plan: widths[4],
            status: widths[5],
            remaining: widths[6],
        };
    }

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
        status: widths[5],
        remaining: widths[6],
    }
}

fn quota_report_view_data(report: &QuotaReport) -> QuotaReportViewData {
    match &report.result {
        Ok(ProviderQuotaSnapshot::OpenAi(usage)) => QuotaReportViewData {
            email: display_optional(usage.email.as_deref()).to_string(),
            plan: display_optional(usage.plan_type.as_deref()).to_string(),
            main: format_main_windows_compact(usage),
            status: format_openai_quota_status(usage),
            resets: Some(format_openai_reset_summary(usage)),
        },
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
        Ok(ProviderQuotaSnapshot::Gemini(info)) => QuotaReportViewData {
            email: display_optional(info.email.as_deref()).to_string(),
            plan: display_optional(info.plan.as_deref()).to_string(),
            main: format_gemini_main_quota(info),
            status: format_gemini_quota_status(info),
            resets: Some(format_gemini_reset_summary(info).map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Ok(ProviderQuotaSnapshot::External(info)) => QuotaReportViewData {
            email: display_optional(info.account.as_deref()).to_string(),
            plan: display_optional(info.plan.as_deref()).to_string(),
            main: info.main.clone(),
            status: info.status.clone(),
            resets: Some(info.reset.as_ref().map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Err(err) => QuotaReportViewData {
            email: "-".to_string(),
            plan: "-".to_string(),
            main: "-".to_string(),
            status: format_quota_error_status(err),
            resets: Some(format_quota_error_detail(err)),
        },
    }
}

fn format_openai_reset_summary(usage: &UsageResponse) -> String {
    let reset_summary = format_main_reset_summary(usage);
    match usage.rate_limit_reset_credits.as_ref() {
        Some(credits) => format!(
            "resets: {reset_summary}; reset credits: {} available",
            credits.available_count
        ),
        None => format!("resets: {reset_summary}"),
    }
}

fn render_quota_report_header(column_widths: QuotaReportColumnWidths) -> String {
    format!(
        "{:<name_w$}  {:<act_w$}  {:<auth_w$}  {:<email_w$}  {:<plan_w$}  {:<status_w$}  {:<main_w$}",
        "PROFILE",
        "CUR",
        "AUTH",
        "ACCOUNT",
        "PLAN",
        "STATUS",
        "REMAINING",
        name_w = column_widths.profile,
        act_w = column_widths.current,
        auth_w = column_widths.auth,
        email_w = column_widths.account,
        plan_w = column_widths.plan,
        status_w = column_widths.status,
        main_w = column_widths.remaining,
    )
}

fn render_quota_report_window_header(
    total_width: usize,
    pool_summary: &[String],
    header: &str,
) -> Vec<String> {
    let mut output = vec![section_header_with_width("Quota Overview", total_width)];
    output.extend(pool_summary.iter().cloned());
    output.push(header.to_string());
    output.push("-".repeat(text_width(header)));
    output
}

fn build_quota_report_sections(
    reports: &[QuotaReport],
    column_widths: QuotaReportColumnWidths,
    detail: bool,
    total_width: usize,
    sort: QuotaReportSort,
) -> Vec<Vec<String>> {
    sort_quota_reports_for_display_with_sort(reports, sort)
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
    if detail && quota_report_should_show_workspace(report) {
        push_wrapped_quota_report_line(
            &mut section,
            &format!("workspace: {}", quota_report_workspace_label(report)),
            total_width,
        );
    }
    if detail && let Some(resets) = view.resets.as_deref() {
        section.extend(wrap_text(resets, total_width.max(1)));
    }
    if detail && let Ok(ProviderQuotaSnapshot::OpenAi(usage)) = &report.result {
        for line in format_openai_additional_limit_summaries(usage) {
            section.extend(wrap_text(&line, total_width.max(1)));
        }
    }
    section.push(String::new());
    section
}

pub fn format_openai_additional_limit_summaries(usage: &UsageResponse) -> Vec<String> {
    usage
        .additional_rate_limits
        .iter()
        .filter_map(format_openai_additional_limit_summary)
        .collect()
}

fn format_openai_additional_limit_summary(additional: &AdditionalRateLimit) -> Option<String> {
    let name = additional
        .limit_name
        .as_deref()
        .or(additional.metered_feature.as_deref())
        .unwrap_or("Additional");
    let main = format_window_pair_compact(&additional.rate_limit);
    if main == "-" {
        return None;
    }
    Some(format!(
        "{name}: {main}; resets: {}",
        format_additional_reset_pair(&additional.rate_limit)
    ))
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

fn quota_report_openai_email(report: &QuotaReport) -> Option<&str> {
    match report.result.as_ref().ok()? {
        ProviderQuotaSnapshot::OpenAi(usage) => usage
            .email
            .as_deref()
            .map(str::trim)
            .filter(|email| !email.is_empty()),
        ProviderQuotaSnapshot::Copilot(_)
        | ProviderQuotaSnapshot::Gemini(_)
        | ProviderQuotaSnapshot::External(_) => None,
    }
}

fn quota_report_should_show_workspace(report: &QuotaReport) -> bool {
    quota_report_openai_email(report).is_some()
        && (report
            .workspace_name
            .as_deref()
            .map(str::trim)
            .is_some_and(|name| !name.is_empty())
            || report
                .workspace_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|id| !id.is_empty()))
}

fn quota_report_workspace_label(report: &QuotaReport) -> String {
    if let Some(name) = report
        .workspace_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return name.to_string();
    }
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

fn render_quota_report_row(
    report: &QuotaReport,
    column_widths: QuotaReportColumnWidths,
    view: &QuotaReportViewData,
) -> String {
    let active = if report.active { "*" } else { "" };
    format!(
        "{}{}{}{}{}{}{}{}{}{}{}{}{}",
        pad_cell(&report.name, column_widths.profile),
        CLI_TABLE_GAP,
        pad_cell(active, column_widths.current),
        CLI_TABLE_GAP,
        pad_cell(&report.auth.label, column_widths.auth),
        CLI_TABLE_GAP,
        pad_cell(&view.email, column_widths.account),
        CLI_TABLE_GAP,
        pad_cell(&view.plan, column_widths.plan),
        CLI_TABLE_GAP,
        pad_cell(&view.status, column_widths.status),
        CLI_TABLE_GAP,
        pad_cell(&view.main, column_widths.remaining),
    )
}

fn push_wrapped_quota_report_line(section: &mut Vec<String>, line: &str, total_width: usize) {
    section.extend(
        wrap_text(line, total_width.saturating_sub(4).max(1))
            .into_iter()
            .map(|line| format!("    {line}")),
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

pub fn sort_quota_reports_for_display(reports: &[QuotaReport]) -> Vec<&QuotaReport> {
    sort_quota_reports_for_display_with_sort(reports, QuotaReportSort::Remaining)
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
    let mut sorted = (0..reports.len()).collect::<Vec<_>>();
    sorted.sort_by(|&left, &right| {
        compare_quota_reports_for_sort(&reports[left], &reports[right], sort)
            .then_with(|| reports[left].name.cmp(&reports[right].name))
    });
    sorted
}

fn compare_quota_reports_for_sort(
    left: &QuotaReport,
    right: &QuotaReport,
    sort: QuotaReportSort,
) -> Ordering {
    match sort {
        QuotaReportSort::Current => quota_report_current_blocked_sort_key(left)
            .cmp(&quota_report_current_blocked_sort_key(right))
            .then_with(|| {
                quota_report_current_sort_key(left).cmp(&quota_report_current_sort_key(right))
            })
            .then_with(|| {
                quota_report_remaining_sort_key(left).cmp(&quota_report_remaining_sort_key(right))
            }),
        QuotaReportSort::Remaining => {
            quota_report_remaining_sort_key(left).cmp(&quota_report_remaining_sort_key(right))
        }
        QuotaReportSort::Profile => compare_sort_text(&left.name, &right.name),
        QuotaReportSort::Auth => compare_sort_text(&left.auth.label, &right.auth.label),
        QuotaReportSort::Account => compare_sort_text(
            &quota_report_account_sort_value(left),
            &quota_report_account_sort_value(right),
        ),
        QuotaReportSort::Plan => compare_sort_text(
            &quota_report_plan_sort_value(left),
            &quota_report_plan_sort_value(right),
        ),
    }
}

fn compare_sort_text(left: &str, right: &str) -> Ordering {
    normalize_sort_text(left).cmp(&normalize_sort_text(right))
}

fn normalize_sort_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn quota_report_remaining_sort_key(report: &QuotaReport) -> (usize, i64) {
    (
        quota_report_status_rank(report),
        quota_report_earliest_main_reset_epoch(report).unwrap_or(i64::MAX),
    )
}

fn quota_report_current_sort_key(report: &QuotaReport) -> usize {
    usize::from(!report.active)
}

fn quota_report_current_blocked_sort_key(report: &QuotaReport) -> usize {
    match report.result.as_ref().ok() {
        Some(ProviderQuotaSnapshot::OpenAi(usage)) => {
            if openai_quota_has_ready_limit(usage) {
                return 0;
            }
            let blocked = collect_blocked_limits(usage, false);
            if blocked
                .iter()
                .any(|limit| limit.message.to_ascii_lowercase().contains("5h"))
            {
                1
            } else if blocked
                .iter()
                .any(|limit| limit.message.to_ascii_lowercase().contains("weekly"))
            {
                2
            } else {
                3
            }
        }
        Some(_) if quota_report_status_rank(report) == 0 => 0,
        Some(_) => 3,
        None => 4,
    }
}

fn quota_report_account_sort_value(report: &QuotaReport) -> String {
    quota_report_view_data(report).email
}

fn quota_report_plan_sort_value(report: &QuotaReport) -> String {
    quota_report_view_data(report).plan
}

fn quota_report_status_rank(report: &QuotaReport) -> usize {
    match &report.result {
        Ok(ProviderQuotaSnapshot::OpenAi(usage)) if openai_quota_has_ready_limit(usage) => 0,
        Ok(ProviderQuotaSnapshot::OpenAi(_)) => 1,
        Ok(ProviderQuotaSnapshot::Copilot(info)) if copilot_quota_is_ready(info) => 0,
        Ok(ProviderQuotaSnapshot::Copilot(_)) => 1,
        Ok(ProviderQuotaSnapshot::Gemini(info)) if gemini_quota_is_ready(info) => 0,
        Ok(ProviderQuotaSnapshot::Gemini(_)) => 1,
        Ok(ProviderQuotaSnapshot::External(info)) => match info.available {
            Some(true) => 0,
            Some(false) => 1,
            None => 1,
        },
        Err(_) => 2,
    }
}

fn quota_report_earliest_main_reset_epoch(report: &QuotaReport) -> Option<i64> {
    match report.result.as_ref().ok()? {
        ProviderQuotaSnapshot::OpenAi(usage) => earliest_required_main_reset_epoch(usage),
        ProviderQuotaSnapshot::Copilot(info) => copilot_reset_epoch(info),
        ProviderQuotaSnapshot::Gemini(info) => gemini_reset_epoch(info),
        ProviderQuotaSnapshot::External(_) => None,
    }
}

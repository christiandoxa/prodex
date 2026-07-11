use super::super::{
    ProviderQuotaSnapshot, QuotaProviderFilter, QuotaReport, QuotaReportSort,
    format_copilot_main_quota, format_copilot_quota_status, format_copilot_reset_summary,
    format_gemini_main_quota, format_gemini_quota_status, format_gemini_reset_summary,
    format_main_windows, format_main_windows_compact, format_openai_quota_status,
    format_quota_error_status, quota_pool_summary_fields_for_reports,
    sorted_quota_report_indexes_by_sort,
};
use super::{
    AllQuotaWatchLayout, AllQuotaWatchSnapshot, filter_quota_reports_by_provider,
    quota_watch_updated_at,
};
use prodex_quota::UsageResponse;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use terminal_ui::{
    pad_cell, tui_border_style, tui_connected_footer_block, tui_connected_separator_line,
    tui_detail_style, tui_error_style, tui_muted_style, tui_success_style, tui_title_style,
};

pub(super) struct AllQuotaWatchTuiFrame {
    pub(super) title: String,
    pub(super) body: String,
    pub(super) overview_fields: Vec<(String, String)>,
    pub(super) table: Option<AllQuotaWatchTuiTable>,
    pub(super) footer: String,
}

pub(super) struct AllQuotaWatchTuiTable {
    pub(super) rows: Vec<AllQuotaWatchTuiRow>,
}

pub(super) struct AllQuotaWatchTuiRow {
    pub(super) profile: Vec<String>,
    pub(super) current: Vec<String>,
    pub(super) auth: Vec<String>,
    pub(super) account: Vec<String>,
    pub(super) plan: Vec<String>,
    pub(super) status: Vec<String>,
    pub(super) remaining: Vec<String>,
    pub(super) detail: Vec<String>,
}

pub(super) fn build_all_quota_watch_tui_frame(
    snapshot: &AllQuotaWatchSnapshot,
    layout: AllQuotaWatchLayout,
) -> AllQuotaWatchTuiFrame {
    let (body, overview_fields, table) = match snapshot {
        AllQuotaWatchSnapshot::Reports { reports, .. } => {
            let filtered_reports =
                filter_quota_reports_by_provider(reports, layout.provider_filter);
            let sorted_indexes =
                sorted_quota_report_indexes_by_sort(&filtered_reports, layout.sort);
            let shown_profiles = quota_watch_visible_profile_count(
                &filtered_reports,
                &sorted_indexes,
                layout.detail,
                layout.max_lines,
                layout.scroll_offset,
            );
            (
                String::new(),
                quota_watch_tui_overview_fields(&filtered_reports),
                Some(build_all_quota_watch_tui_table(
                    &filtered_reports,
                    layout,
                    &sorted_indexes,
                    shown_profiles,
                )),
            )
        }
        AllQuotaWatchSnapshot::Loading { updated } => (
            "Quota".to_string(),
            quota_watch_status_fields("Loading quota data...", updated),
            None,
        ),
        AllQuotaWatchSnapshot::Empty { updated } => (
            "Quota".to_string(),
            quota_watch_status_fields("No profiles configured", updated),
            None,
        ),
        AllQuotaWatchSnapshot::Error { updated, message } => (
            "Quota".to_string(),
            quota_watch_status_fields(message, updated),
            None,
        ),
    };
    let provider_hint = if layout.provider_filter_locked {
        "provider fixed"
    } else {
        "f provider"
    };
    AllQuotaWatchTuiFrame {
        title: "Prodex Quota".to_string(),
        body,
        overview_fields,
        table,
        footer: format!(
            "u update | s sort {} | filter {} | {} | j/k scroll | q quit",
            layout.sort.label(),
            layout.provider_filter.label(),
            provider_hint
        ),
    }
}

fn quota_watch_tui_overview_fields(reports: &[QuotaReport]) -> Vec<(String, String)> {
    let mut fields = quota_pool_summary_fields_for_reports(reports);
    if fields
        .first()
        .is_some_and(|(label, _)| label == "Available")
        && fields
            .get(1)
            .is_some_and(|(label, _)| label == "Last Updated")
    {
        let updated = fields.remove(1).1;
        fields[0].1 = format!("{} | updated {updated}", fields[0].1);
    }
    fields
}

fn quota_watch_status_fields(message: &str, updated: &str) -> Vec<(String, String)> {
    vec![
        ("Status".to_string(), message.to_string()),
        ("Last Updated".to_string(), updated.to_string()),
    ]
}

fn build_all_quota_watch_tui_table(
    reports: &[QuotaReport],
    layout: AllQuotaWatchLayout,
    sorted_indexes: &[usize],
    shown_profiles: usize,
) -> AllQuotaWatchTuiTable {
    let rows = sorted_indexes
        .iter()
        .copied()
        .skip(layout.scroll_offset)
        .take(shown_profiles)
        .map(|index| build_all_quota_watch_tui_row(&reports[index], layout.detail))
        .collect::<Vec<_>>();
    AllQuotaWatchTuiTable { rows }
}

fn build_all_quota_watch_tui_row(report: &QuotaReport, detail: bool) -> AllQuotaWatchTuiRow {
    let view = quota_watch_report_view(report);
    let account = vec![view.account];
    let mut detail_line = Vec::new();
    if detail && quota_watch_should_show_workspace(report) {
        detail_line.push(format!(
            "workspace: {}",
            quota_watch_workspace_label(report)
        ));
    }
    if detail && let Some(resets) = view.resets {
        let reset = quota_watch_reset_detail(&resets);
        detail_line.insert(0, reset.windows);
        if let Some(credits) = reset.credits {
            detail_line.push(credits);
        }
    }
    if detail && let Ok(ProviderQuotaSnapshot::OpenAi(usage)) = &report.result {
        detail_line.extend(prodex_quota::format_openai_additional_limit_summaries(
            usage,
        ));
    }
    AllQuotaWatchTuiRow {
        profile: vec![report.name.clone()],
        current: vec![if report.active { "*" } else { "" }.to_string()],
        auth: vec![report.auth.label.clone()],
        account,
        plan: vec![view.plan],
        status: vec![view.status],
        remaining: vec![view.main],
        detail: (!detail_line.is_empty())
            .then(|| detail_line.join(" | "))
            .into_iter()
            .collect(),
    }
}

struct QuotaWatchReportView {
    account: String,
    plan: String,
    main: String,
    status: String,
    resets: Option<String>,
}

fn quota_watch_report_view(report: &QuotaReport) -> QuotaWatchReportView {
    match &report.result {
        Ok(ProviderQuotaSnapshot::OpenAi(usage)) => QuotaWatchReportView {
            account: quota_watch_optional(usage.email.as_deref()).to_string(),
            plan: quota_watch_optional(usage.plan_type.as_deref()).to_string(),
            main: format_main_windows_compact(usage),
            status: format_openai_quota_status(usage),
            resets: Some(quota_watch_openai_reset_summary(usage)),
        },
        Ok(ProviderQuotaSnapshot::Copilot(info)) => QuotaWatchReportView {
            account: quota_watch_optional(info.login.as_deref()).to_string(),
            plan: quota_watch_optional(
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
        Ok(ProviderQuotaSnapshot::Gemini(info)) => QuotaWatchReportView {
            account: quota_watch_optional(info.email.as_deref()).to_string(),
            plan: quota_watch_optional(info.plan.as_deref()).to_string(),
            main: format_gemini_main_quota(info),
            status: format_gemini_quota_status(info),
            resets: Some(format_gemini_reset_summary(info).map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Ok(ProviderQuotaSnapshot::External(info)) => QuotaWatchReportView {
            account: quota_watch_optional(info.account.as_deref()).to_string(),
            plan: quota_watch_optional(info.plan.as_deref()).to_string(),
            main: info.main.clone(),
            status: info.status.clone(),
            resets: Some(info.reset.as_ref().map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Err(err) => QuotaWatchReportView {
            account: "-".to_string(),
            plan: "-".to_string(),
            main: "-".to_string(),
            status: format_quota_error_status(err),
            resets: Some(prodex_quota::format_quota_error_detail(err)),
        },
    }
}

fn quota_watch_openai_reset_summary(usage: &UsageResponse) -> String {
    let reset_summary = prodex_quota::format_main_reset_summary(usage);
    match usage.rate_limit_reset_credits.as_ref() {
        Some(credits) => format!(
            "resets: {reset_summary}; reset credits: {} available",
            credits.available_count
        ),
        None => format!("resets: {reset_summary}"),
    }
}

struct QuotaWatchResetDetail {
    windows: String,
    credits: Option<String>,
}

fn quota_watch_reset_detail(resets: &str) -> QuotaWatchResetDetail {
    if resets.trim_start().starts_with("error:") {
        return QuotaWatchResetDetail {
            windows: resets.to_string(),
            credits: None,
        };
    }
    let (windows, credits) = resets
        .strip_prefix("resets: ")
        .unwrap_or(resets)
        .split_once("; reset credits:")
        .map_or(
            (resets.strip_prefix("resets: ").unwrap_or(resets), None),
            |(left, right)| (left, Some(right.trim())),
        );
    QuotaWatchResetDetail {
        windows: format!("resets: {windows}"),
        credits: credits.map(|credits| format!("reset credits: {credits}")),
    }
}

fn quota_watch_optional(value: Option<&str>) -> &str {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-")
}

fn quota_watch_openai_email(report: &QuotaReport) -> Option<&str> {
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

fn quota_watch_should_show_workspace(report: &QuotaReport) -> bool {
    quota_watch_openai_email(report).is_some()
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

fn quota_watch_workspace_label(report: &QuotaReport) -> String {
    report
        .workspace_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| report.name.trim())
        .to_string()
}

fn quota_human_tui_compact_label(line: &str) -> Option<&'static str> {
    const LABELS: [&str; 23] = [
        "Available",
        "Last Updated",
        "5h remaining pool",
        "Weekly remaining pool",
        "Remaining pool",
        "Account",
        "Plan",
        "Status",
        "Main",
        "5h",
        "Weekly",
        "Reset credits",
        "Auth",
        "Provider",
        "Reset",
        "Project",
        "Bucket",
        "Quota",
        "Rate groups",
        "Model groups",
        "Admin API",
        "Model provider",
        "Source",
    ];
    LABELS
        .into_iter()
        .find(|label| line.trim_start().starts_with(&format!("{label}:")))
}

pub(super) fn build_profile_quota_watch_tui_frame(
    profile_name: &str,
    updated: &str,
    quota_result: std::result::Result<ProviderQuotaSnapshot, String>,
) -> AllQuotaWatchTuiFrame {
    let (body, overview_fields) = match quota_result {
        Ok(snapshot) => (
            format!("Quota {profile_name}"),
            quota_watch_profile_fields(updated, &snapshot),
        ),
        Err(err) => (
            format!("Quota {profile_name}"),
            quota_watch_status_fields(&err, updated),
        ),
    };
    AllQuotaWatchTuiFrame {
        title: format!("Prodex Quota {profile_name}"),
        body,
        overview_fields,
        table: None,
        footer: "refresh 5s | q quit".to_string(),
    }
}

fn quota_watch_profile_fields(
    updated: &str,
    snapshot: &ProviderQuotaSnapshot,
) -> Vec<(String, String)> {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => {
            vec![
                (
                    "Account".to_string(),
                    quota_watch_optional(usage.email.as_deref()).to_string(),
                ),
                (
                    "Plan".to_string(),
                    quota_watch_optional(usage.plan_type.as_deref()).to_string(),
                ),
                ("Status".to_string(), format_openai_quota_status(usage)),
                ("Main".to_string(), format_main_windows(usage)),
                (
                    "Reset".to_string(),
                    quota_watch_openai_reset_summary(usage)
                        .trim_start_matches("resets: ")
                        .to_string(),
                ),
                ("Last Updated".to_string(), updated.to_string()),
            ]
        }
        ProviderQuotaSnapshot::Copilot(info) => vec![
            (
                "Account".to_string(),
                quota_watch_optional(info.login.as_deref()).to_string(),
            ),
            (
                "Plan".to_string(),
                quota_watch_optional(
                    info.copilot_plan
                        .as_deref()
                        .or(info.access_type_sku.as_deref()),
                )
                .to_string(),
            ),
            ("Status".to_string(), format_copilot_quota_status(info)),
            ("Main".to_string(), format_copilot_main_quota(info)),
            (
                "Reset".to_string(),
                format_copilot_reset_summary(info).unwrap_or_else(|| "unavailable".to_string()),
            ),
            ("Last Updated".to_string(), updated.to_string()),
        ],
        ProviderQuotaSnapshot::Gemini(info) => vec![
            (
                "Account".to_string(),
                quota_watch_optional(info.email.as_deref()).to_string(),
            ),
            (
                "Plan".to_string(),
                quota_watch_optional(info.plan.as_deref()).to_string(),
            ),
            ("Status".to_string(), format_gemini_quota_status(info)),
            ("Main".to_string(), format_gemini_main_quota(info)),
            (
                "Reset".to_string(),
                format_gemini_reset_summary(info).unwrap_or_else(|| "unavailable".to_string()),
            ),
            ("Last Updated".to_string(), updated.to_string()),
        ],
        ProviderQuotaSnapshot::External(info) => vec![
            (
                "Account".to_string(),
                quota_watch_optional(info.account.as_deref()).to_string(),
            ),
            (
                "Plan".to_string(),
                quota_watch_optional(info.plan.as_deref()).to_string(),
            ),
            ("Status".to_string(), info.status.clone()),
            ("Main".to_string(), info.main.clone()),
            (
                "Reset".to_string(),
                info.reset
                    .clone()
                    .unwrap_or_else(|| "unavailable".to_string()),
            ),
            ("Last Updated".to_string(), updated.to_string()),
        ],
    }
}

pub(super) fn render_all_quota_watch_tui(
    frame: &mut ratatui::Frame<'_>,
    data: &AllQuotaWatchTuiFrame,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(frame.area());

    let body_block = Block::default()
        .title(Line::styled(data.title.as_str(), quota_watch_title_style()))
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .border_style(quota_watch_border_style());
    let body_area = body_block.inner(chunks[0]);
    frame.render_widget(body_block, chunks[0]);

    if let Some(table) = &data.table {
        let overview_height =
            quota_watch_overview_height(data.overview_fields.len(), body_area.height);
        let body_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(overview_height),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(body_area);
        let overview = Paragraph::new(quota_watch_fields_text(&data.body, &data.overview_fields))
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(overview, body_chunks[0]);
        render_quota_watch_connected_separator(frame, chunks[0], body_chunks[1].y);
        render_all_quota_watch_tui_table(frame, body_chunks[2], table);
    } else if !data.overview_fields.is_empty() {
        let body = Paragraph::new(quota_watch_fields_text(&data.body, &data.overview_fields))
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(body, body_area);
    } else {
        let body = Paragraph::new(quota_human_tui_text(&data.body))
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(body, body_area);
    }

    let footer = Paragraph::new(Line::styled(
        data.footer.as_str(),
        quota_watch_footer_style(),
    ))
    .block(tui_connected_footer_block(quota_watch_border_style()));
    frame.render_widget(footer, chunks[1]);
}

fn render_quota_watch_connected_separator(frame: &mut ratatui::Frame<'_>, outer: Rect, y: u16) {
    frame.render_widget(
        Paragraph::new(Line::styled(
            quota_watch_separator_line(outer.width),
            quota_watch_border_style(),
        )),
        Rect {
            x: outer.x,
            y,
            width: outer.width,
            height: 1,
        },
    );
}

pub(super) fn quota_watch_separator_line(width: u16) -> String {
    tui_connected_separator_line(width)
}

pub(crate) fn render_all_quota_reports_once_tui(
    frame: &mut ratatui::Frame<'_>,
    reports: &[QuotaReport],
    detail: bool,
) {
    let area = frame.area();
    let snapshot = AllQuotaWatchSnapshot::Reports {
        updated: quota_watch_updated_at(),
        profile_count: reports.len(),
        reports: reports.to_vec(),
    };
    let data = build_all_quota_watch_tui_frame(
        &snapshot,
        AllQuotaWatchLayout {
            detail,
            scroll_offset: 0,
            sort: QuotaReportSort::Remaining,
            provider_filter: QuotaProviderFilter::All,
            provider_filter_locked: false,
            total_width: usize::from(area.width).saturating_sub(4),
            max_lines: quota_watch_tui_table_lines(
                area.height,
                quota_watch_snapshot_overview_field_count(&snapshot, QuotaProviderFilter::All),
            ),
        },
    );
    render_all_quota_watch_tui(frame, &data);
}

pub(crate) fn render_profile_quota_once_tui(
    frame: &mut ratatui::Frame<'_>,
    profile_name: &str,
    quota: ProviderQuotaSnapshot,
) {
    let data =
        build_profile_quota_watch_tui_frame(profile_name, &quota_watch_updated_at(), Ok(quota));
    render_all_quota_watch_tui(frame, &data);
}

fn render_all_quota_watch_tui_table(
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
    table: &AllQuotaWatchTuiTable,
) {
    let text = quota_watch_table_text(table);
    let widget = Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(widget, area);
}

pub(super) fn quota_watch_overview_height(field_count: usize, max_height: u16) -> u16 {
    u16::try_from(field_count)
        .unwrap_or(max_height)
        .min(max_height)
}

pub(super) fn quota_watch_table_text(table: &AllQuotaWatchTuiTable) -> Text<'static> {
    let mut lines = vec![Line::styled(
        format!(
            "{:<24} {:<3} {:<7} {:<24} {:<8} {:<15} {}",
            "PROFILE", "CUR", "AUTH", "ACCOUNT", "PLAN", "STATUS", "REMAINING"
        ),
        quota_watch_title_style(),
    )];
    for (index, row) in table.rows.iter().enumerate() {
        if index > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(quota_watch_table_main_line(row));
        for detail in &row.detail {
            lines.push(Line::styled(detail.clone(), quota_watch_detail_style()));
        }
    }
    Text::from(lines)
}

fn quota_watch_table_main_line(row: &AllQuotaWatchTuiRow) -> Line<'static> {
    let status = quota_watch_first_cell(&row.status);
    let cell = quota_watch_table_cell_text;
    Line::from(vec![
        Span::raw(cell(quota_watch_first_cell(&row.profile), 24)),
        Span::raw(" "),
        Span::raw(cell(quota_watch_first_cell(&row.current), 3)),
        Span::raw(" "),
        Span::raw(cell(quota_watch_first_cell(&row.auth), 7)),
        Span::raw(" "),
        Span::raw(cell(quota_watch_first_cell(&row.account), 24)),
        Span::raw(" "),
        Span::raw(cell(quota_watch_first_cell(&row.plan), 8)),
        Span::raw(" "),
        Span::styled(cell(status, 15), quota_watch_status_style(status)),
        Span::raw(" "),
        Span::raw(quota_watch_first_cell(&row.remaining).to_string()),
    ])
}

fn quota_watch_table_cell_text(value: &str, width: usize) -> String {
    pad_cell(value.trim(), width)
}

fn quota_watch_status_style(status: &str) -> Style {
    if status.contains("Blocked") || status.contains("Error") {
        tui_error_style()
    } else if status.contains("Ready") || status.contains("healthy") {
        tui_success_style()
    } else {
        Style::default()
    }
}

fn quota_watch_first_cell(lines: &[String]) -> &str {
    lines.first().map(String::as_str).unwrap_or("")
}

fn quota_watch_fields_text(title: &str, fields: &[(String, String)]) -> Text<'static> {
    let mut lines = Vec::new();
    if !title.is_empty() {
        lines.push(Line::styled(title.to_string(), quota_watch_title_style()));
    }
    lines.extend(fields.iter().map(|(label, value)| {
        Line::from(vec![
            Span::styled(format!("{label}:"), tui_title_style()),
            Span::raw(" "),
            Span::raw(value.clone()),
        ])
    }));
    Text::from(lines)
}

pub(crate) fn quota_human_tui_text(output: &str) -> Text<'_> {
    Text::from(
        output
            .lines()
            .map(|line| Line::from(quota_human_tui_spans(line)))
            .collect::<Vec<_>>(),
    )
}

pub(super) fn quota_human_tui_spans(line: &str) -> Vec<Span<'_>> {
    if line.starts_with("== ")
        || line == "Quota Overview"
        || line.starts_with("Quota ")
        || line.ends_with("profiles")
    {
        return vec![Span::styled(line, quota_watch_title_style())];
    }
    if quota_human_tui_compact_label(line).is_some() {
        let Some((label, value)) = line.split_once(':') else {
            return vec![Span::raw(line)];
        };
        return vec![
            Span::styled(format!("{label}:"), tui_title_style()),
            Span::raw(" "),
            Span::raw(value.trim_start().to_string()),
        ];
    }
    if line.chars().all(|ch| ch == '-' || ch.is_whitespace()) {
        return vec![Span::styled(line, quota_watch_muted_style())];
    }
    if line.contains("Blocked") || line.contains("Error") {
        return vec![Span::styled(line, tui_error_style())];
    }
    if line.contains("Ready") || line.contains("healthy") {
        return vec![Span::styled(line, tui_success_style())];
    }
    if line.contains("thin") || line.contains("critical") || line.contains("exhausted") {
        return vec![Span::styled(line, tui_error_style())];
    }
    if line.starts_with("PROFILE") {
        return vec![Span::styled(
            line,
            Style::default().add_modifier(Modifier::BOLD),
        )];
    }
    if line.starts_with("workspace:")
        || line.starts_with("error:")
        || line.starts_with("resets:")
        || line.starts_with("status:")
        || line.starts_with("reset credits:")
        || line.trim_start().starts_with("workspace:")
        || line.trim_start().starts_with("error:")
        || line.trim_start().starts_with("resets:")
        || line.trim_start().starts_with("status:")
        || line.trim_start().starts_with("reset credits:")
    {
        return vec![Span::styled(line, quota_watch_detail_style())];
    }
    if line.starts_with("press ") || line.trim_start().starts_with("press ") {
        return vec![Span::styled(line, quota_watch_muted_style())];
    }
    vec![Span::raw(line)]
}

fn quota_watch_detail_style() -> Style {
    tui_detail_style()
}

fn quota_watch_title_style() -> Style {
    tui_title_style()
}

fn quota_watch_border_style() -> Style {
    tui_border_style()
}

fn quota_watch_muted_style() -> Style {
    tui_muted_style()
}

fn quota_watch_footer_style() -> Style {
    tui_title_style()
}

pub(super) fn quota_watch_snapshot_overview_field_count(
    snapshot: &AllQuotaWatchSnapshot,
    provider_filter: QuotaProviderFilter,
) -> usize {
    match snapshot {
        AllQuotaWatchSnapshot::Reports { reports, .. } => quota_watch_tui_overview_fields(
            &filter_quota_reports_by_provider(reports, provider_filter),
        )
        .len(),
        AllQuotaWatchSnapshot::Loading { .. }
        | AllQuotaWatchSnapshot::Empty { .. }
        | AllQuotaWatchSnapshot::Error { .. } => 2,
    }
}

pub(super) fn quota_watch_tui_table_lines(
    terminal_height: u16,
    overview_field_count: usize,
) -> Option<usize> {
    let body_inner = usize::from(terminal_height)
        .saturating_sub(3)
        .saturating_sub(2);
    let overview_height = overview_field_count.min(body_inner);
    Some(
        body_inner
            .saturating_sub(overview_height)
            .saturating_sub(1)
            .max(1),
    )
}

fn quota_watch_visible_profile_count(
    reports: &[QuotaReport],
    sorted_indexes: &[usize],
    detail: bool,
    max_lines: Option<usize>,
    start_profile: usize,
) -> usize {
    let Some(max_lines) = max_lines else {
        return sorted_indexes.len().saturating_sub(start_profile);
    };
    let mut shown_profiles = 0_usize;
    let mut remaining = max_lines.saturating_sub(1);
    for index in sorted_indexes.iter().copied().skip(start_profile) {
        let row_lines = usize::from(shown_profiles > 0)
            + quota_watch_tui_row_line_count(&reports[index], detail);
        if row_lines > remaining {
            break;
        }
        remaining = remaining.saturating_sub(row_lines);
        shown_profiles += 1;
    }
    shown_profiles
}

fn quota_watch_tui_row_line_count(report: &QuotaReport, detail: bool) -> usize {
    build_all_quota_watch_tui_row(report, detail)
        .detail
        .len()
        .saturating_add(1)
}

pub(super) fn quota_watch_tui_max_scroll_offset_for_snapshot(
    snapshot: &AllQuotaWatchSnapshot,
    detail: bool,
    provider_filter: QuotaProviderFilter,
    sort: QuotaReportSort,
    max_lines: Option<usize>,
) -> usize {
    let AllQuotaWatchSnapshot::Reports { reports, .. } = snapshot else {
        return 0;
    };
    let filtered_reports = filter_quota_reports_by_provider(reports, provider_filter);
    if filtered_reports.is_empty() {
        return 0;
    }
    let sorted_indexes = sorted_quota_report_indexes_by_sort(&filtered_reports, sort);
    for scroll_offset in 0..filtered_reports.len() {
        let shown_profiles = quota_watch_visible_profile_count(
            &filtered_reports,
            &sorted_indexes,
            detail,
            max_lines,
            scroll_offset,
        );
        if scroll_offset.saturating_add(shown_profiles) >= filtered_reports.len() {
            return scroll_offset;
        }
    }
    filtered_reports.len().saturating_sub(1)
}

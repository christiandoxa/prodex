use self::data::{
    build_all_quota_watch_tui_row, quota_human_tui_compact_label, quota_watch_profile_fields,
};
use super::*;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use terminal_ui::{
    pad_cell, tui_border_style, tui_detail_style, tui_error_style, tui_muted_style,
    tui_success_style, tui_title_style,
};

#[path = "frame_data.rs"]
mod data;

pub(crate) struct AllQuotaWatchTuiFrame {
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) overview_fields: Vec<(String, String)>,
    pub(crate) table: Option<AllQuotaWatchTuiTable>,
    pub(crate) footer: String,
}

pub(crate) struct AllQuotaWatchTuiTable {
    pub(crate) rows: Vec<AllQuotaWatchTuiRow>,
}

pub(crate) struct AllQuotaWatchTuiRow {
    pub(crate) profile: Vec<String>,
    pub(crate) current: Vec<String>,
    pub(crate) auth: Vec<String>,
    pub(crate) account: Vec<String>,
    pub(crate) plan: Vec<String>,
    pub(crate) status: Vec<String>,
    pub(crate) remaining: Vec<String>,
    pub(crate) detail: Vec<String>,
}

pub(crate) fn build_all_quota_watch_tui_frame(
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
                "Quota Overview".to_string(),
                quota_pool_summary_fields_for_reports(&filtered_reports),
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

pub(crate) fn build_profile_quota_watch_tui_frame(
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

pub(crate) fn render_all_quota_watch_tui(
    frame: &mut ratatui::Frame<'_>,
    data: &AllQuotaWatchTuiFrame,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(frame.area());

    let body_block = Block::default()
        .title(Line::styled(data.title.as_str(), quota_watch_title_style()))
        .borders(Borders::ALL)
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
        frame.render_widget(
            Block::default()
                .borders(Borders::TOP)
                .border_style(quota_watch_border_style()),
            body_chunks[1],
        );
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
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(quota_watch_border_style()),
    );
    frame.render_widget(footer, chunks[1]);
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

pub(crate) fn quota_watch_overview_height(field_count: usize, max_height: u16) -> u16 {
    u16::try_from(field_count.saturating_add(1))
        .unwrap_or(max_height)
        .min(max_height)
}

pub(crate) fn quota_watch_table_text(table: &AllQuotaWatchTuiTable) -> Text<'static> {
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
    let mut lines = vec![Line::styled(title.to_string(), quota_watch_title_style())];
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

pub(crate) fn quota_human_tui_spans(line: &str) -> Vec<Span<'_>> {
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

pub(crate) fn quota_watch_tui_table_lines(
    terminal_height: u16,
    overview_field_count: usize,
) -> Option<usize> {
    let body_inner = usize::from(terminal_height)
        .saturating_sub(3)
        .saturating_sub(2);
    let overview_height = overview_field_count.saturating_add(1).min(body_inner);
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

pub(crate) fn quota_watch_tui_max_scroll_offset_for_snapshot(
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

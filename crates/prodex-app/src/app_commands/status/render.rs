use super::{
    StatusOverview, StatusResourceHistory, StatusResourceSnapshot, format_info_load_summary,
    format_info_pool_remaining, format_info_runway, format_info_token_usage_summary,
};
use chrono::{Local, TimeZone};
use prodex_app_reports::InfoTokenUsageSummary;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline, Wrap};
use terminal_ui::{
    tui_accent_style, tui_border_style, tui_error_style, tui_hint_style, tui_metric_style,
    tui_muted_style, tui_secondary_style, tui_title_style,
};

pub(super) fn render_status_dashboard(
    frame: &mut Frame<'_>,
    overview: Option<&StatusOverview>,
    resources: &StatusResourceSnapshot,
    history: &StatusResourceHistory,
    error: Option<&str>,
    refreshing: bool,
) {
    let area = frame.area();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    render_status_header(frame, rows[0], overview, refreshing);

    if rows[1].height < 14 || rows[1].width < 70 {
        render_compact_status(frame, rows[1], overview, resources, error);
    } else {
        let body = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(8)])
            .split(rows[1]);
        let quota = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(body[0]);
        render_quota_gauge(frame, quota[0], "5 HOUR", overview, true);
        render_quota_gauge(frame, quota[1], "WEEKLY", overview, false);

        let detail_direction = if body[1].width >= 100 {
            Direction::Horizontal
        } else {
            Direction::Vertical
        };
        let detail = Layout::default()
            .direction(detail_direction)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(body[1]);
        render_token_panel(frame, detail[0], overview);
        render_resource_panel(frame, detail[1], resources, history);
    }

    let footer = if let Some(error) = error {
        Line::from(vec![
            Span::styled(" refresh error: ", tui_error_style()),
            Span::styled(error.to_string(), tui_muted_style()),
        ])
    } else {
        Line::from(vec![
            Span::styled(" q/esc ", tui_hint_style().add_modifier(Modifier::BOLD)),
            Span::raw("quit  "),
            Span::styled("r ", tui_hint_style().add_modifier(Modifier::BOLD)),
            Span::raw("refresh  "),
            Span::styled(
                "NET shows Prodex socket queues; disk rates come from /proc/<pid>/io",
                tui_muted_style(),
            ),
        ])
    };
    frame.render_widget(Paragraph::new(footer), rows[2]);
}

fn render_status_header(
    frame: &mut Frame<'_>,
    area: Rect,
    overview: Option<&StatusOverview>,
    refreshing: bool,
) {
    let profile = overview
        .map(|overview| overview.runtime_profile.as_str())
        .unwrap_or("loading");
    let updated = overview
        .map(|overview| overview.updated_at.as_str())
        .unwrap_or("waiting for first snapshot");
    let state = if refreshing { "refreshing" } else { "live" };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" PRODEX STATUS ", tui_title_style()),
        Span::styled(format!(" profile {profile} "), tui_accent_style()),
        Span::styled(format!(" {state} · {updated} "), tui_secondary_style()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(tui_border_style()),
    )
    .alignment(Alignment::Center);
    frame.render_widget(header, area);
}

fn render_quota_gauge(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    overview: Option<&StatusOverview>,
    five_hour: bool,
) {
    let Some(overview) = overview else {
        frame.render_widget(
            Paragraph::new("loading quota…").block(status_block(title)),
            area,
        );
        return;
    };
    let window = if five_hour {
        overview.quota.five_hour
    } else {
        overview.quota.weekly
    };
    if window.profiles == 0 {
        frame.render_widget(
            Paragraph::new("quota unavailable").block(status_block(title)),
            area,
        );
        return;
    }
    let average = (window.total_remaining as f64 / window.profiles as f64).clamp(0.0, 100.0);
    let reset = format_reset(window.earliest_reset_at, Local::now().timestamp());
    let label = format!(
        "{average:.0}% avg · pool {}% · {reset}",
        window.total_remaining
    );
    frame.render_widget(
        Gauge::default()
            .block(status_block(title))
            .gauge_style(Style::default().fg(quota_color(average)))
            .ratio(average / 100.0)
            .label(label),
        area,
    );
}

fn render_token_panel(frame: &mut Frame<'_>, area: Rect, overview: Option<&StatusOverview>) {
    let block = status_block("TOKEN USAGE · HISTORICAL");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(overview) = overview else {
        frame.render_widget(Paragraph::new("loading token history…"), inner);
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(2)])
        .split(inner);
    let total = overview
        .token_summary
        .total
        .input_tokens
        .saturating_add(overview.token_summary.total.output_tokens);
    let lines = vec![
        Line::from(vec![
            Span::styled("total ", tui_secondary_style()),
            Span::styled(human_count(total), tui_metric_style()),
            Span::styled(
                format!(
                    "  {} event(s) / {} log(s)",
                    overview.token_summary.event_count, overview.token_summary.log_count
                ),
                tui_muted_style(),
            ),
        ]),
        Line::from(format!(
            "in {}  cached {}  out {}  reasoning {}",
            human_count(overview.token_summary.total.input_tokens),
            human_count(overview.token_summary.total.cached_input_tokens),
            human_count(overview.token_summary.total.output_tokens),
            human_count(overview.token_summary.total.reasoning_tokens),
        )),
        Line::from(vec![
            Span::styled("token efficiency ", tui_secondary_style()),
            Span::styled(
                token_efficiency(&overview.token_summary),
                tui_accent_style(),
            ),
        ]),
        Line::from(vec![
            Span::styled("active/config ", tui_secondary_style()),
            Span::styled(
                format!("{} / {}", overview.runtime_profile, overview.active_profile),
                tui_accent_style(),
            ),
            Span::styled(
                format!("  {} profile(s)", overview.profile_count),
                tui_muted_style(),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), rows[0]);
    let history_title = match (&overview.token_first_at, &overview.token_last_at) {
        (Some(first), Some(last)) => format!(" recent events {first} → {last} "),
        _ => " no token events ".to_string(),
    };
    frame.render_widget(
        Sparkline::default()
            .block(Block::default().title(history_title))
            .data(&overview.token_history)
            .style(Style::default().fg(Color::LightMagenta)),
        rows[1],
    );
}

fn render_resource_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    resources: &StatusResourceSnapshot,
    history: &StatusResourceHistory,
) {
    let block = status_block("PRODEX RESOURCES");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if !resources.available {
        frame.render_widget(
            Paragraph::new("process resources unavailable (Linux /proc required)"),
            inner,
        );
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(2),
        ])
        .split(inner);
    let cpu = resources.cpu_percent.unwrap_or_default();
    frame.render_widget(
        Gauge::default()
            .block(Block::default().title(" CPU "))
            .gauge_style(Style::default().fg(quota_color(100.0 - cpu)))
            .ratio((cpu / 100.0).clamp(0.0, 1.0))
            .label(format!("{cpu:.1}% host capacity")),
        rows[0],
    );
    let memory_percent = resource_memory_percent(resources);
    let details = vec![
        Line::from(format!(
            "RAM {} ({memory_percent:.1}%) · {} proc / {} runtime",
            human_bytes(resources.resident_bytes),
            resources.process_count,
            resources.runtime_process_count,
        )),
        Line::from(format!(
            "DISK R {}/s W {}/s · NET {} sockets RXq {} TXq {}",
            human_bytes(resources.disk_read_bytes_per_second),
            human_bytes(resources.disk_write_bytes_per_second),
            resources.socket_count,
            human_bytes(resources.network_rx_queue_bytes),
            human_bytes(resources.network_tx_queue_bytes),
        )),
    ];
    frame.render_widget(Paragraph::new(details), rows[1]);
    let charts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(rows[2]);
    let cpu_history = history.cpu.iter().copied().collect::<Vec<_>>();
    let memory_history = history.memory.iter().copied().collect::<Vec<_>>();
    let disk_history = history.disk.iter().copied().collect::<Vec<_>>();
    let network_history = history.network.iter().copied().collect::<Vec<_>>();
    for (area, title, data, color) in [
        (charts[0], "CPU", cpu_history.as_slice(), Color::LightCyan),
        (
            charts[1],
            "RAM",
            memory_history.as_slice(),
            Color::LightGreen,
        ),
        (
            charts[2],
            "DISK",
            disk_history.as_slice(),
            Color::LightYellow,
        ),
        (
            charts[3],
            "NET",
            network_history.as_slice(),
            Color::LightBlue,
        ),
    ] {
        frame.render_widget(
            Sparkline::default()
                .block(Block::default().title(format!(" {title} ")))
                .data(data)
                .style(Style::default().fg(color)),
            area,
        );
    }
}

fn render_compact_status(
    frame: &mut Frame<'_>,
    area: Rect,
    overview: Option<&StatusOverview>,
    resources: &StatusResourceSnapshot,
    error: Option<&str>,
) {
    let lines = if let Some(overview) = overview {
        status_fields(overview, resources)
            .into_iter()
            .map(|(label, value)| {
                Line::from(vec![
                    Span::styled(format!("{label}: "), tui_secondary_style()),
                    Span::raw(value),
                ])
            })
            .collect::<Vec<_>>()
    } else {
        vec![Line::from(
            error
                .unwrap_or("collecting first status snapshot…")
                .to_string(),
        )]
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(status_block("SUMMARY"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn status_block(title: &'static str) -> Block<'static> {
    Block::default()
        .title(Line::from(Span::styled(
            format!(" {title} "),
            tui_title_style(),
        )))
        .borders(Borders::ALL)
        .border_style(tui_border_style())
}

pub(super) fn status_fields(
    overview: &StatusOverview,
    resources: &StatusResourceSnapshot,
) -> Vec<(String, String)> {
    let now = Local::now().timestamp();
    vec![
        (
            "Profile".to_string(),
            format!(
                "runtime={}, configured={}, pool={}, quota-compatible={}, unavailable={}",
                overview.runtime_profile,
                overview.active_profile,
                overview.profile_count,
                overview.quota.compatible_profiles,
                overview.quota.unavailable_profiles
            ),
        ),
        (
            "5h quota".to_string(),
            format_info_pool_remaining(
                overview.quota.five_hour.total_remaining,
                overview.quota.five_hour.profiles,
                overview.quota.five_hour.earliest_reset_at,
            ),
        ),
        (
            "5h runway".to_string(),
            format_info_runway(
                overview.quota.five_hour.profiles,
                overview.quota.five_hour.total_remaining,
                overview.quota.five_hour.earliest_reset_at,
                overview.five_hour_runway.as_ref(),
                now,
            ),
        ),
        (
            "Weekly quota".to_string(),
            format_info_pool_remaining(
                overview.quota.weekly.total_remaining,
                overview.quota.weekly.profiles,
                overview.quota.weekly.earliest_reset_at,
            ),
        ),
        (
            "Weekly runway".to_string(),
            format_info_runway(
                overview.quota.weekly.profiles,
                overview.quota.weekly.total_remaining,
                overview.quota.weekly.earliest_reset_at,
                overview.weekly_runway.as_ref(),
                now,
            ),
        ),
        (
            "Token usage".to_string(),
            format_info_token_usage_summary(&overview.token_summary),
        ),
        (
            "Token efficiency".to_string(),
            token_efficiency(&overview.token_summary),
        ),
        (
            "Token history".to_string(),
            format!(
                "{} {} → {}",
                text_sparkline(&overview.token_history),
                overview.token_first_at.as_deref().unwrap_or("-"),
                overview.token_last_at.as_deref().unwrap_or("-")
            ),
        ),
        (
            "Processes".to_string(),
            format!(
                "{} total, {} runtime; CPU {}",
                resources.process_count,
                resources.runtime_process_count,
                resources
                    .cpu_percent
                    .map(|value| format!("{value:.1}%"))
                    .unwrap_or_else(|| "warming up".to_string())
            ),
        ),
        (
            "Memory".to_string(),
            format!(
                "{} ({:.1}% host)",
                human_bytes(resources.resident_bytes),
                resource_memory_percent(resources)
            ),
        ),
        (
            "Network".to_string(),
            format!(
                "{} sockets; RX queue {}, TX queue {}",
                resources.socket_count,
                human_bytes(resources.network_rx_queue_bytes),
                human_bytes(resources.network_tx_queue_bytes)
            ),
        ),
        (
            "Disk I/O".to_string(),
            format!(
                "read {} total ({}/s), write {} total ({}/s)",
                human_bytes(resources.disk_read_bytes),
                human_bytes(resources.disk_read_bytes_per_second),
                human_bytes(resources.disk_write_bytes),
                human_bytes(resources.disk_write_bytes_per_second)
            ),
        ),
        (
            "Recent load".to_string(),
            format_info_load_summary(&overview.runtime_load, overview.runtime_process_count),
        ),
        ("Updated".to_string(), overview.updated_at.clone()),
    ]
}

fn token_efficiency(summary: &InfoTokenUsageSummary) -> String {
    let input = summary.total.input_tokens;
    let output = summary.total.output_tokens;
    let cache = if input == 0 {
        0.0
    } else {
        summary.total.cached_input_tokens as f64 / input as f64 * 100.0
    };
    let output_share = if input.saturating_add(output) == 0 {
        0.0
    } else {
        output as f64 / input.saturating_add(output) as f64 * 100.0
    };
    format!("cache hit {cache:.1}% · output share {output_share:.1}%")
}

fn resource_memory_percent(resources: &StatusResourceSnapshot) -> f64 {
    if resources.memory_total_bytes == 0 {
        0.0
    } else {
        resources.resident_bytes as f64 / resources.memory_total_bytes as f64 * 100.0
    }
}

fn quota_color(remaining: f64) -> Color {
    if remaining <= 10.0 {
        Color::LightRed
    } else if remaining <= 25.0 {
        Color::LightYellow
    } else {
        Color::LightGreen
    }
}

fn format_reset(reset_at: Option<i64>, now: i64) -> String {
    let Some(reset_at) = reset_at else {
        return "reset unknown".to_string();
    };
    let relative = terminal_ui::format_relative_duration(reset_at.saturating_sub(now));
    let absolute = Local
        .timestamp_opt(reset_at, 0)
        .single()
        .map(|value| value.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| reset_at.to_string());
    format!("reset in {relative} ({absolute})")
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn human_count(value: u64) -> String {
    if value >= 1_000_000_000 {
        format!("{:.1}B", value as f64 / 1_000_000_000.0)
    } else if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

pub(super) fn text_sparkline(values: &[u64]) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = values.iter().copied().max().unwrap_or_default();
    if max == 0 {
        return "-".to_string();
    }
    values
        .iter()
        .map(|value| {
            let index = ((*value as u128 * (BARS.len() - 1) as u128) / max as u128) as usize;
            BARS[index]
        })
        .collect()
}

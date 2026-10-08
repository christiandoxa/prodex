use super::{
    StatusOverview, StatusResourceHistory, StatusResourceSnapshot, format_info_load_summary,
    format_info_pool_remaining, format_info_token_usage_summary,
};
use chrono::{Local, TimeZone};
use prodex_mojo_core::info_render;
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
        let gauge = info_render::format_status_quota_gauge(
            window.total_remaining,
            window.profiles,
            window.earliest_reset_at,
            Local::now().timestamp(),
            None,
        )
        .expect("Mojo status quota-gauge formatter returned invalid output");
        frame.render_widget(Paragraph::new(gauge.label).block(status_block(title)), area);
        return;
    }
    let now = Local::now().timestamp();
    let absolute_reset = window.earliest_reset_at.and_then(|reset_at| {
        Local
            .timestamp_opt(reset_at, 0)
            .single()
            .map(|value| value.format("%m-%d %H:%M").to_string())
            .or_else(|| Some(reset_at.to_string()))
    });
    let gauge = info_render::format_status_quota_gauge(
        window.total_remaining,
        window.profiles,
        window.earliest_reset_at,
        now,
        absolute_reset.as_deref(),
    )
    .expect("Mojo status quota-gauge formatter returned invalid output");
    frame.render_widget(
        Gauge::default()
            .block(status_block(title))
            .gauge_style(Style::default().fg(quota_color(gauge.band)))
            .ratio(gauge.ratio)
            .label(gauge.label),
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
            Span::styled(
                info_render::format_human_count(total)
                    .expect("Mojo status human-count formatter returned invalid output"),
                tui_metric_style(),
            ),
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
            info_render::format_human_count(overview.token_summary.total.input_tokens)
                .expect("Mojo status human-count formatter returned invalid output"),
            info_render::format_human_count(overview.token_summary.total.cached_input_tokens)
                .expect("Mojo status human-count formatter returned invalid output"),
            info_render::format_human_count(overview.token_summary.total.output_tokens)
                .expect("Mojo status human-count formatter returned invalid output"),
            info_render::format_human_count(overview.token_summary.total.reasoning_tokens)
                .expect("Mojo status human-count formatter returned invalid output"),
        )),
        Line::from(vec![
            Span::styled("token efficiency ", tui_secondary_style()),
            Span::styled(
                info_render::format_token_efficiency(
                    overview.token_summary.total.input_tokens,
                    overview.token_summary.total.cached_input_tokens,
                    overview.token_summary.total.output_tokens,
                )
                .expect("Mojo status token-efficiency formatter returned invalid output"),
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
            .gauge_style(Style::default().fg(resource_color(100.0 - cpu)))
            .ratio((cpu / 100.0).clamp(0.0, 1.0))
            .label(format!("{cpu:.1}% host capacity")),
        rows[0],
    );
    let memory_percent =
        info_render::format_memory_percent(resources.resident_bytes, resources.memory_total_bytes)
            .expect("Mojo status memory-percent formatter returned invalid output");
    let details = vec![
        Line::from(format!(
            "RAM {} ({memory_percent}) · {} proc / {} runtime",
            info_render::format_human_bytes(resources.resident_bytes)
                .expect("Mojo status byte formatter returned invalid output"),
            resources.process_count,
            resources.runtime_process_count,
        )),
        Line::from(format!(
            "DISK R {}/s W {}/s · NET {} sockets RXq {} TXq {}",
            info_render::format_human_bytes(resources.disk_read_bytes_per_second)
                .expect("Mojo status byte formatter returned invalid output"),
            info_render::format_human_bytes(resources.disk_write_bytes_per_second)
                .expect("Mojo status byte formatter returned invalid output"),
            resources.socket_count,
            info_render::format_human_bytes(resources.network_rx_queue_bytes)
                .expect("Mojo status byte formatter returned invalid output"),
            info_render::format_human_bytes(resources.network_tx_queue_bytes)
                .expect("Mojo status byte formatter returned invalid output"),
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
    let five_hour_runway = status_runway(
        overview.quota.five_hour.profiles,
        overview.quota.five_hour.total_remaining,
        overview.quota.five_hour.earliest_reset_at,
        overview.five_hour_runway.as_ref(),
        now,
    );
    let weekly_runway = status_runway(
        overview.quota.weekly.profiles,
        overview.quota.weekly.total_remaining,
        overview.quota.weekly.earliest_reset_at,
        overview.weekly_runway.as_ref(),
        now,
    );
    let token_history_text = info_render::format_text_sparkline(&overview.token_history)
        .expect("Mojo status sparkline formatter returned invalid output");
    info_render::format_status_fields(info_render::InfoStatusFields {
        runtime_profile: &overview.runtime_profile,
        active_profile: &overview.active_profile,
        profile_count: overview.profile_count,
        quota_compatible_profiles: overview.quota.compatible_profiles,
        unavailable_profiles: overview.quota.unavailable_profiles,
        five_hour_quota: &format_info_pool_remaining(
            overview.quota.five_hour.total_remaining,
            overview.quota.five_hour.profiles,
            overview.quota.five_hour.earliest_reset_at,
        ),
        five_hour_runway: &five_hour_runway,
        weekly_quota: &format_info_pool_remaining(
            overview.quota.weekly.total_remaining,
            overview.quota.weekly.profiles,
            overview.quota.weekly.earliest_reset_at,
        ),
        weekly_runway: &weekly_runway,
        token_usage_summary: &format_info_token_usage_summary(&overview.token_summary),
        token_input: overview.token_summary.total.input_tokens,
        token_cached_input: overview.token_summary.total.cached_input_tokens,
        token_output: overview.token_summary.total.output_tokens,
        token_history_text: &token_history_text,
        token_first_at: overview.token_first_at.as_deref(),
        token_last_at: overview.token_last_at.as_deref(),
        resources: info_render::InfoStatusResources {
            available: resources.available,
            process_count: resources.process_count,
            runtime_process_count: resources.runtime_process_count,
            cpu_percent: resources.cpu_percent,
            resident_bytes: resources.resident_bytes,
            memory_total_bytes: resources.memory_total_bytes,
            socket_count: resources.socket_count,
            network_rx_queue_bytes: resources.network_rx_queue_bytes,
            network_tx_queue_bytes: resources.network_tx_queue_bytes,
            disk_read_bytes: resources.disk_read_bytes,
            disk_write_bytes: resources.disk_write_bytes,
            disk_read_bytes_per_second: resources.disk_read_bytes_per_second,
            disk_write_bytes_per_second: resources.disk_write_bytes_per_second,
        },
        recent_load: &format_info_load_summary(
            &overview.runtime_load,
            overview.runtime_process_count,
        ),
        updated_at: &overview.updated_at,
    })
    .expect("Mojo status field renderer returned invalid output")
}

fn status_runway(
    profiles_with_data: usize,
    current_remaining: i64,
    earliest_reset_at: Option<i64>,
    estimate: Option<&super::InfoRunwayEstimate>,
    now: i64,
) -> String {
    let reset_text = earliest_reset_at.map(|timestamp| {
        (
            timestamp,
            prodex_quota::format_precise_reset_time(Some(timestamp)),
        )
    });
    let exhaust_text = estimate.map(|estimate| {
        (
            estimate,
            prodex_quota::format_precise_reset_time(Some(estimate.exhaust_at)),
        )
    });
    info_render::format_status_runway(info_render::InfoStatusRunway {
        profiles_with_data,
        current_remaining,
        earliest_reset: reset_text
            .as_ref()
            .map(|(timestamp, text)| info_render::InfoStatusReset {
                timestamp: *timestamp,
                text,
            }),
        estimate: exhaust_text.as_ref().map(|(estimate, text)| {
            info_render::InfoStatusRunwayEstimate {
                burn_per_hour: estimate.burn_per_hour,
                observed_profiles: estimate.observed_profiles,
                observed_span_seconds: estimate.observed_span_seconds,
                exhaust_at: estimate.exhaust_at,
                exhaust_text: text,
            }
        }),
        now,
    })
    .expect("Mojo status runway renderer returned invalid output")
}

fn quota_color(band: u8) -> Color {
    match band {
        0 => Color::LightRed,
        1 => Color::LightYellow,
        _ => Color::LightGreen,
    }
}

fn resource_color(remaining: f64) -> Color {
    if remaining <= 10.0 {
        Color::LightRed
    } else if remaining <= 25.0 {
        Color::LightYellow
    } else {
        Color::LightGreen
    }
}

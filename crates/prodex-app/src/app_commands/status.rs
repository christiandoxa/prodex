use super::log_tui::LogTuiTerminal;
use anyhow::Result;
use chrono::{Local, TimeZone};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use prodex_app_reports::{
    InfoTokenUsageEvent, InfoTokenUsageSummary, collect_info_token_usage_summary_from_texts,
    info_token_usage_event_from_line, runtime_usage_snapshot_is_usable,
    usage_from_runtime_usage_snapshot,
};
use prodex_quota::required_main_window_snapshot_at;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline, Wrap};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};
use terminal_ui::{
    print_panel, tui_accent_style, tui_border_style, tui_error_style, tui_hint_style,
    tui_metric_style, tui_muted_style, tui_secondary_style, tui_title_style,
};

use crate::{
    AppPaths, AppState, AppStateIoExt, INFO_RUNTIME_LOG_TAIL_BYTES, InfoQuotaWindow,
    InfoRunwayEstimate, ProdexProcessInfo, RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    StatusArgs, collect_active_runtime_log_paths, collect_info_runtime_load_summary,
    collect_prodex_processes, collect_recent_runtime_log_paths, collect_run_profile_reports,
    estimate_info_runway, format_info_load_summary, format_info_pool_remaining, format_info_runway,
    format_info_token_usage_summary, load_runtime_usage_snapshots, read_runtime_log_tail,
};

const STATUS_OVERVIEW_REFRESH: Duration = Duration::from_secs(5);
const STATUS_INPUT_POLL: Duration = Duration::from_millis(100);
const STATUS_HISTORY_POINTS: usize = 60;
const STATUS_TOKEN_HISTORY_POINTS: usize = 64;

struct StatusOverview {
    updated_at: String,
    active_profile: String,
    runtime_profile: String,
    profile_count: usize,
    quota: StatusQuotaSummary,
    five_hour_runway: Option<InfoRunwayEstimate>,
    weekly_runway: Option<InfoRunwayEstimate>,
    token_summary: InfoTokenUsageSummary,
    token_history: Vec<u64>,
    token_first_at: Option<String>,
    token_last_at: Option<String>,
    runtime_load: crate::InfoRuntimeLoadSummary,
    runtime_process_count: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct StatusQuotaWindow {
    profiles: usize,
    total_remaining: i64,
    earliest_reset_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default)]
struct StatusQuotaSummary {
    compatible_profiles: usize,
    unavailable_profiles: usize,
    five_hour: StatusQuotaWindow,
    weekly: StatusQuotaWindow,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct StatusResourceCounters {
    available: bool,
    process_count: usize,
    runtime_process_count: usize,
    process_cpu_ticks: u64,
    system_cpu_ticks: u64,
    resident_bytes: u64,
    memory_total_bytes: u64,
    disk_read_bytes: u64,
    disk_write_bytes: u64,
    socket_count: usize,
    network_rx_queue_bytes: u64,
    network_tx_queue_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct StatusResourceSnapshot {
    available: bool,
    process_count: usize,
    runtime_process_count: usize,
    cpu_percent: Option<f64>,
    resident_bytes: u64,
    memory_total_bytes: u64,
    disk_read_bytes: u64,
    disk_write_bytes: u64,
    disk_read_bytes_per_second: u64,
    disk_write_bytes_per_second: u64,
    socket_count: usize,
    network_rx_queue_bytes: u64,
    network_tx_queue_bytes: u64,
}

#[derive(Default)]
struct StatusResourceTracker {
    previous: Option<(Instant, StatusResourceCounters)>,
}

impl StatusResourceTracker {
    fn sample(&mut self) -> StatusResourceSnapshot {
        let processes = collect_prodex_processes();
        let now = Instant::now();
        let counters = collect_status_resource_counters(&processes);
        let snapshot = status_resource_snapshot(
            self.previous
                .map(|(instant, previous)| (previous, now.saturating_duration_since(instant))),
            counters,
        );
        self.previous = Some((now, counters));
        snapshot
    }
}

#[derive(Default)]
struct StatusResourceHistory {
    cpu: VecDeque<u64>,
    memory: VecDeque<u64>,
    disk: VecDeque<u64>,
    network: VecDeque<u64>,
}

impl StatusResourceHistory {
    fn push(&mut self, snapshot: &StatusResourceSnapshot) {
        push_history(
            &mut self.cpu,
            snapshot.cpu_percent.unwrap_or_default().round() as u64,
        );
        push_history(&mut self.memory, snapshot.resident_bytes);
        push_history(
            &mut self.disk,
            snapshot
                .disk_read_bytes_per_second
                .saturating_add(snapshot.disk_write_bytes_per_second),
        );
        push_history(
            &mut self.network,
            snapshot
                .network_rx_queue_bytes
                .saturating_add(snapshot.network_tx_queue_bytes),
        );
    }
}

fn push_history(history: &mut VecDeque<u64>, value: u64) {
    if history.len() == STATUS_HISTORY_POINTS {
        history.pop_front();
    }
    history.push_back(value);
}

struct StatusRefresh {
    receiver: Receiver<std::result::Result<StatusOverview, String>>,
    sender: mpsc::Sender<std::result::Result<StatusOverview, String>>,
    in_flight: bool,
}

impl StatusRefresh {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            receiver,
            sender,
            in_flight: false,
        }
    }

    fn start(&mut self) {
        if self.in_flight {
            return;
        }
        self.in_flight = true;
        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = AppPaths::discover()
                .and_then(|paths| collect_status_overview(&paths))
                .map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
    }

    fn take(&mut self) -> Option<std::result::Result<StatusOverview, String>> {
        let mut latest = None;
        while let Ok(snapshot) = self.receiver.try_recv() {
            self.in_flight = false;
            latest = Some(snapshot);
        }
        latest
    }
}

pub(crate) fn handle_status(args: StatusArgs) -> Result<()> {
    if args.once || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return print_status_once();
    }
    watch_status(Duration::from_secs(args.interval))
}

fn print_status_once() -> Result<()> {
    let paths = AppPaths::discover()?;
    let overview = collect_status_overview(&paths)?;
    let mut resources = StatusResourceTracker::default();
    let _ = resources.sample();
    thread::sleep(Duration::from_millis(200));
    let resources = resources.sample();
    print_panel("Status", &status_fields(&overview, &resources));
    Ok(())
}

fn watch_status(resource_interval: Duration) -> Result<()> {
    let mut tui = LogTuiTerminal::new("status")?;
    let mut refresh = StatusRefresh::new();
    let mut overview = None;
    let mut overview_error = None;
    let mut resources = StatusResourceTracker::default();
    let mut resource_snapshot = resources.sample();
    let mut history = StatusResourceHistory::default();
    history.push(&resource_snapshot);
    let mut next_resource_sample = Instant::now() + resource_interval;
    let mut next_overview_refresh = Instant::now();
    let mut dirty = true;

    loop {
        let now = Instant::now();
        if now >= next_overview_refresh {
            refresh.start();
            next_overview_refresh = now + STATUS_OVERVIEW_REFRESH;
        }
        if let Some(result) = refresh.take() {
            match result {
                Ok(snapshot) => {
                    overview = Some(snapshot);
                    overview_error = None;
                }
                Err(error) => overview_error = Some(error),
            }
            dirty = true;
        }
        if now >= next_resource_sample {
            resource_snapshot = resources.sample();
            history.push(&resource_snapshot);
            next_resource_sample = now + resource_interval;
            dirty = true;
        }

        if dirty {
            tui.terminal.draw(|frame| {
                render_status_dashboard(
                    frame,
                    overview.as_ref(),
                    &resource_snapshot,
                    &history,
                    overview_error.as_deref(),
                    refresh.in_flight,
                )
            })?;
            dirty = false;
        }

        if !event::poll(STATUS_INPUT_POLL)? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                    || (key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('z')))
                {
                    return Ok(());
                }
                if matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')) {
                    next_overview_refresh = Instant::now();
                    next_resource_sample = Instant::now();
                }
            }
            Event::Resize(_, _) => dirty = true,
            _ => {}
        }
    }
}

fn collect_status_overview(paths: &AppPaths) -> Result<StatusOverview> {
    let state = AppState::load(paths)?;
    let now = Local::now().timestamp();
    let quota = collect_status_quota(paths, &state, now);
    let processes = collect_prodex_processes();
    let runtime_logs = collect_active_runtime_log_paths(&processes);
    let runtime_load = collect_info_runtime_load_summary(&runtime_logs, now);
    let runtime_process_count = processes.iter().filter(|process| process.runtime).count();
    let five_hour_runway = estimate_info_runway(
        &runtime_load.observations,
        InfoQuotaWindow::FiveHour,
        quota.five_hour.total_remaining,
        now,
    );
    let weekly_runway = estimate_info_runway(
        &runtime_load.observations,
        InfoQuotaWindow::Weekly,
        quota.weekly.total_remaining,
        now,
    );
    let token_data = collect_status_token_data(&collect_recent_runtime_log_paths(8));
    let active_profile = state.active_profile.unwrap_or_else(|| "-".to_string());
    let runtime_profile = if runtime_process_count == 0 {
        active_profile.clone()
    } else {
        runtime_load
            .observations
            .iter()
            .max_by_key(|observation| observation.timestamp)
            .map(|observation| observation.profile.clone())
            .or(token_data.latest_profile)
            .unwrap_or_else(|| active_profile.clone())
    };

    Ok(StatusOverview {
        updated_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        active_profile,
        runtime_profile,
        profile_count: state.profiles.len(),
        quota,
        five_hour_runway,
        weekly_runway,
        token_summary: token_data.summary,
        token_history: token_data.history,
        token_first_at: token_data.first_at,
        token_last_at: token_data.last_at,
        runtime_load,
        runtime_process_count,
    })
}

fn collect_status_quota(paths: &AppPaths, state: &AppState, now: i64) -> StatusQuotaSummary {
    let profile_names = state
        .profiles
        .iter()
        .filter(|(_, profile)| profile.provider.supports_codex_runtime())
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let snapshots = load_runtime_usage_snapshots(paths, &state.profiles).unwrap_or_default();
    let reports = collect_run_profile_reports(state, profile_names, None, false);
    status_quota_from_reports(&reports, &snapshots, now)
}

fn status_quota_from_reports(
    reports: &[crate::RunProfileProbeReport],
    snapshots: &BTreeMap<String, crate::RuntimeProfileUsageSnapshot>,
    now: i64,
) -> StatusQuotaSummary {
    let mut summary = StatusQuotaSummary {
        compatible_profiles: reports
            .iter()
            .filter(|report| report.auth.quota_compatible)
            .count(),
        ..StatusQuotaSummary::default()
    };

    for report in reports {
        if !report.auth.quota_compatible {
            continue;
        }
        let usage = match &report.result {
            Ok(usage) => Some(usage.clone()),
            Err(_) => snapshots
                .get(&report.name)
                .filter(|snapshot| {
                    runtime_usage_snapshot_is_usable(
                        snapshot,
                        now,
                        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
                    )
                })
                .map(usage_from_runtime_usage_snapshot),
        };
        let Some(usage) = usage else {
            summary.unavailable_profiles += 1;
            continue;
        };
        let five_hour = required_main_window_snapshot_at(&usage, "5h", now);
        let weekly = required_main_window_snapshot_at(&usage, "weekly", now);
        if five_hour.is_none() && weekly.is_none() {
            summary.unavailable_profiles += 1;
            continue;
        }
        if let Some(window) = five_hour {
            add_quota_window(&mut summary.five_hour, window);
        }
        if let Some(window) = weekly {
            add_quota_window(&mut summary.weekly, window);
        }
    }
    summary
}

fn add_quota_window(window: &mut StatusQuotaWindow, snapshot: crate::MainWindowSnapshot) {
    window.profiles += 1;
    window.total_remaining = window
        .total_remaining
        .saturating_add(snapshot.remaining_percent);
    if snapshot.reset_at != i64::MAX {
        window.earliest_reset_at = Some(
            window
                .earliest_reset_at
                .map_or(snapshot.reset_at, |current| current.min(snapshot.reset_at)),
        );
    }
}

struct StatusTokenData {
    summary: InfoTokenUsageSummary,
    history: Vec<u64>,
    first_at: Option<String>,
    last_at: Option<String>,
    latest_profile: Option<String>,
}

fn collect_status_token_data(log_paths: &[PathBuf]) -> StatusTokenData {
    let tails = log_paths
        .iter()
        .filter_map(|path| {
            read_runtime_log_tail(path, INFO_RUNTIME_LOG_TAIL_BYTES)
                .ok()
                .map(|tail| String::from_utf8_lossy(&tail).into_owned())
        })
        .collect::<Vec<_>>();
    let summary = collect_info_token_usage_summary_from_texts(log_paths.len(), &tails);
    let mut events = tails
        .iter()
        .flat_map(|tail| tail.lines().filter_map(info_token_usage_event_from_line))
        .collect::<Vec<_>>();
    events.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.request.cmp(&right.request))
            .then_with(|| left.profile.cmp(&right.profile))
    });
    let first_at = events.first().map(|event| event.timestamp.clone());
    let last_at = events.last().map(|event| event.timestamp.clone());
    let latest_profile = events.last().map(|event| event.profile.clone());
    let history = token_history(&events, STATUS_TOKEN_HISTORY_POINTS);
    StatusTokenData {
        summary,
        history,
        first_at,
        last_at,
        latest_profile,
    }
}

fn token_history(events: &[InfoTokenUsageEvent], limit: usize) -> Vec<u64> {
    events
        .iter()
        .rev()
        .take(limit)
        .map(|event| event.input_tokens.saturating_add(event.output_tokens))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn collect_status_resource_counters(processes: &[ProdexProcessInfo]) -> StatusResourceCounters {
    let mut pids = processes
        .iter()
        .map(|process| process.pid)
        .collect::<BTreeSet<_>>();
    pids.insert(std::process::id());
    let Some(system_cpu_ticks) = fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|text| parse_system_cpu_ticks(&text))
    else {
        return StatusResourceCounters {
            process_count: pids.len(),
            runtime_process_count: processes.iter().filter(|process| process.runtime).count(),
            ..StatusResourceCounters::default()
        };
    };

    let mut counters = StatusResourceCounters {
        available: true,
        process_count: pids.len(),
        runtime_process_count: processes.iter().filter(|process| process.runtime).count(),
        system_cpu_ticks,
        memory_total_bytes: fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|text| parse_kib_field(&text, "MemTotal"))
            .unwrap_or_default(),
        ..StatusResourceCounters::default()
    };
    let mut socket_inodes = HashSet::new();

    for pid in pids {
        let dir = PathBuf::from(format!("/proc/{pid}"));
        counters.process_cpu_ticks = counters.process_cpu_ticks.saturating_add(
            fs::read_to_string(dir.join("stat"))
                .ok()
                .and_then(|text| parse_process_cpu_ticks(&text))
                .unwrap_or_default(),
        );
        counters.resident_bytes = counters.resident_bytes.saturating_add(
            fs::read_to_string(dir.join("status"))
                .ok()
                .and_then(|text| parse_kib_field(&text, "VmRSS"))
                .unwrap_or_default(),
        );
        if let Ok(io) = fs::read_to_string(dir.join("io")) {
            counters.disk_read_bytes = counters
                .disk_read_bytes
                .saturating_add(parse_u64_field(&io, "read_bytes").unwrap_or_default());
            counters.disk_write_bytes = counters
                .disk_write_bytes
                .saturating_add(parse_u64_field(&io, "write_bytes").unwrap_or_default());
        }
        collect_socket_inodes(&dir.join("fd"), &mut socket_inodes);
    }

    counters.socket_count = socket_inodes.len();
    for path in [
        "/proc/net/tcp",
        "/proc/net/tcp6",
        "/proc/net/udp",
        "/proc/net/udp6",
    ] {
        let Ok(table) = fs::read_to_string(path) else {
            continue;
        };
        let (rx, tx) = parse_network_queues(&table, &socket_inodes);
        counters.network_rx_queue_bytes = counters.network_rx_queue_bytes.saturating_add(rx);
        counters.network_tx_queue_bytes = counters.network_tx_queue_bytes.saturating_add(tx);
    }
    counters
}

fn status_resource_snapshot(
    previous: Option<(StatusResourceCounters, Duration)>,
    current: StatusResourceCounters,
) -> StatusResourceSnapshot {
    let (cpu_percent, disk_read_bytes_per_second, disk_write_bytes_per_second) = previous
        .filter(|(previous, _)| previous.available && current.available)
        .map(|(previous, elapsed)| {
            let system_delta = current
                .system_cpu_ticks
                .saturating_sub(previous.system_cpu_ticks);
            let process_delta = current
                .process_cpu_ticks
                .saturating_sub(previous.process_cpu_ticks);
            let cpu = (system_delta > 0)
                .then_some((process_delta as f64 / system_delta as f64 * 100.0).clamp(0.0, 100.0));
            let seconds = elapsed.as_secs_f64().max(0.001);
            (
                cpu,
                (current
                    .disk_read_bytes
                    .saturating_sub(previous.disk_read_bytes) as f64
                    / seconds) as u64,
                (current
                    .disk_write_bytes
                    .saturating_sub(previous.disk_write_bytes) as f64
                    / seconds) as u64,
            )
        })
        .unwrap_or((None, 0, 0));

    StatusResourceSnapshot {
        available: current.available,
        process_count: current.process_count,
        runtime_process_count: current.runtime_process_count,
        cpu_percent,
        resident_bytes: current.resident_bytes,
        memory_total_bytes: current.memory_total_bytes,
        disk_read_bytes: current.disk_read_bytes,
        disk_write_bytes: current.disk_write_bytes,
        disk_read_bytes_per_second,
        disk_write_bytes_per_second,
        socket_count: current.socket_count,
        network_rx_queue_bytes: current.network_rx_queue_bytes,
        network_tx_queue_bytes: current.network_tx_queue_bytes,
    }
}

fn parse_process_cpu_ticks(text: &str) -> Option<u64> {
    let fields = text
        .rsplit_once(')')?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    Some(
        fields
            .get(11)?
            .parse::<u64>()
            .ok()?
            .saturating_add(fields.get(12)?.parse::<u64>().ok()?),
    )
}

fn parse_system_cpu_ticks(text: &str) -> Option<u64> {
    let mut fields = text.lines().next()?.split_whitespace();
    if fields.next()? != "cpu" {
        return None;
    }
    let mut total = 0_u64;
    for value in fields.take(8) {
        total = total.saturating_add(value.parse::<u64>().ok()?);
    }
    Some(total)
}

fn parse_kib_field(text: &str, key: &str) -> Option<u64> {
    let value = text.lines().find_map(|line| {
        let (candidate, value) = line.split_once(':')?;
        (candidate == key).then_some(value)
    })?;
    value
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()
        .map(|kib| kib.saturating_mul(1024))
}

fn parse_u64_field(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let (candidate, value) = line.split_once(':')?;
        (candidate == key).then(|| value.trim().parse::<u64>().ok())?
    })
}

fn collect_socket_inodes(fd_dir: &Path, inodes: &mut HashSet<u64>) {
    let Ok(entries) = fs::read_dir(fd_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(target) = fs::read_link(entry.path()) else {
            continue;
        };
        let Some(target) = target.to_str() else {
            continue;
        };
        if let Some(inode) = target
            .strip_prefix("socket:[")
            .and_then(|value| value.strip_suffix(']'))
            .and_then(|value| value.parse::<u64>().ok())
        {
            inodes.insert(inode);
        }
    }
}

fn parse_network_queues(text: &str, inodes: &HashSet<u64>) -> (u64, u64) {
    let mut rx = 0_u64;
    let mut tx = 0_u64;
    for line in text.lines().skip(1) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let Some(inode) = fields.get(9).and_then(|value| value.parse::<u64>().ok()) else {
            continue;
        };
        if !inodes.contains(&inode) {
            continue;
        }
        let Some((tx_hex, rx_hex)) = fields.get(4).and_then(|value| value.split_once(':')) else {
            continue;
        };
        tx = tx.saturating_add(u64::from_str_radix(tx_hex, 16).unwrap_or_default());
        rx = rx.saturating_add(u64::from_str_radix(rx_hex, 16).unwrap_or_default());
    }
    (rx, tx)
}

fn render_status_dashboard(
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

fn status_fields(
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

fn text_sparkline(values: &[u64]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_parsers_extract_cpu_memory_and_disk_counters() {
        assert_eq!(
            parse_process_cpu_ticks("123 (prodex worker) S 1 2 3 4 5 6 7 8 9 10 120 30 0 0 0"),
            Some(150)
        );
        assert_eq!(
            parse_system_cpu_ticks("cpu  10 20 30 40 50 60 70 80 90\ncpu0 1 2 3 4"),
            Some(360)
        );
        assert_eq!(
            parse_kib_field("VmRSS: 2048 kB\n", "VmRSS"),
            Some(2_097_152)
        );
        assert_eq!(
            parse_u64_field("read_bytes: 123\nwrite_bytes: 456\n", "write_bytes"),
            Some(456)
        );
    }

    #[test]
    fn resource_snapshot_derives_cpu_and_disk_rates() {
        let previous = StatusResourceCounters {
            available: true,
            process_cpu_ticks: 100,
            system_cpu_ticks: 1_000,
            disk_read_bytes: 1_000,
            disk_write_bytes: 2_000,
            ..StatusResourceCounters::default()
        };
        let current = StatusResourceCounters {
            available: true,
            process_cpu_ticks: 120,
            system_cpu_ticks: 1_200,
            disk_read_bytes: 3_000,
            disk_write_bytes: 5_000,
            ..StatusResourceCounters::default()
        };
        let snapshot = status_resource_snapshot(Some((previous, Duration::from_secs(2))), current);

        assert_eq!(snapshot.cpu_percent, Some(10.0));
        assert_eq!(snapshot.disk_read_bytes_per_second, 1_000);
        assert_eq!(snapshot.disk_write_bytes_per_second, 1_500);
    }

    #[test]
    fn quota_summary_keeps_weekly_window_when_five_hour_is_absent() {
        let now = 1_000;
        let reports = vec![crate::RunProfileProbeReport {
            name: "main".to_string(),
            order_index: 0,
            auth: crate::AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(crate::UsageResponse {
                email: None,
                plan_type: None,
                rate_limit: Some(crate::WindowPair {
                    primary_window: None,
                    secondary_window: Some(crate::UsageWindow {
                        used_percent: Some(40),
                        reset_at: Some(2_000),
                        limit_window_seconds: Some(604_800),
                    }),
                }),
                code_review_rate_limit: None,
                rate_limit_reset_credits: None,
                additional_rate_limits: Vec::new(),
            }),
        }];
        let summary = status_quota_from_reports(&reports, &BTreeMap::new(), now);

        assert_eq!(summary.five_hour.profiles, 0);
        assert_eq!(summary.weekly.profiles, 1);
        assert_eq!(summary.weekly.total_remaining, 60);
        assert_eq!(summary.weekly.earliest_reset_at, Some(2_000));
    }

    #[test]
    fn network_queue_parser_filters_prodex_socket_inodes() {
        let table = concat!(
            "sl local_address rem_address st tx_queue:rx_queue tr tm->when retrnsmt uid timeout inode\n",
            "0: 0100007F:1F90 00000000:0000 0A 00000010:00000020 00:00000000 00000000 1000 0 42\n",
            "1: 0100007F:1F91 00000000:0000 0A 00000100:00000200 00:00000000 00000000 1000 0 99\n",
        );
        assert_eq!(
            parse_network_queues(table, &HashSet::from([42])),
            (0x20, 0x10)
        );
    }

    #[test]
    fn token_history_is_chronological_and_bounded() {
        let event = |timestamp: &str, input_tokens| InfoTokenUsageEvent {
            timestamp: timestamp.to_string(),
            input_tokens,
            output_tokens: 5,
            ..InfoTokenUsageEvent::default()
        };
        let events = vec![event("1", 10), event("2", 20), event("3", 30)];
        assert_eq!(token_history(&events, 2), vec![25, 35]);
        assert_eq!(text_sparkline(&[1, 2, 3]).chars().count(), 3);
    }

    #[test]
    fn dashboard_renders_at_wide_standard_and_compact_sizes() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let overview = StatusOverview {
            updated_at: "2026-07-13 20:00:00".to_string(),
            active_profile: "main".to_string(),
            runtime_profile: "main".to_string(),
            profile_count: 2,
            quota: StatusQuotaSummary {
                compatible_profiles: 2,
                five_hour: StatusQuotaWindow {
                    profiles: 2,
                    total_remaining: 140,
                    earliest_reset_at: Some(2_000),
                },
                weekly: StatusQuotaWindow {
                    profiles: 2,
                    total_remaining: 120,
                    earliest_reset_at: Some(10_000),
                },
                ..StatusQuotaSummary::default()
            },
            five_hour_runway: None,
            weekly_runway: None,
            token_summary: InfoTokenUsageSummary::default(),
            token_history: vec![10, 20, 30],
            token_first_at: Some("first".to_string()),
            token_last_at: Some("last".to_string()),
            runtime_load: crate::InfoRuntimeLoadSummary::default(),
            runtime_process_count: 1,
        };
        let resources = StatusResourceSnapshot {
            available: true,
            process_count: 2,
            runtime_process_count: 1,
            cpu_percent: Some(12.5),
            resident_bytes: 64 * 1024 * 1024,
            memory_total_bytes: 1024 * 1024 * 1024,
            ..StatusResourceSnapshot::default()
        };

        for (width, height) in [(120, 40), (80, 24), (60, 12)] {
            let mut terminal =
                Terminal::new(TestBackend::new(width, height)).expect("test terminal");
            terminal
                .draw(|frame| {
                    render_status_dashboard(
                        frame,
                        Some(&overview),
                        &resources,
                        &StatusResourceHistory::default(),
                        None,
                        false,
                    )
                })
                .expect("status dashboard should render");
        }
    }
}

use super::log_tui::LogTuiTerminal;
use anyhow::Result;
use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use prodex_app_reports::{
    InfoTokenUsageEvent, InfoTokenUsageSummary, collect_info_token_usage_summary_from_texts,
    info_token_usage_event_from_line, runtime_usage_snapshot_is_usable,
    usage_from_runtime_usage_snapshot,
};
use prodex_quota::required_main_window_snapshot_at;
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};
use terminal_ui::print_panel;

use crate::{
    AppPaths, AppState, AppStateIoExt, INFO_RUNTIME_LOG_TAIL_BYTES, InfoQuotaWindow,
    InfoRunwayEstimate, RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS, StatusArgs,
    collect_active_runtime_log_paths, collect_info_runtime_load_summary, collect_prodex_processes,
    collect_recent_runtime_log_paths, collect_run_profile_reports, estimate_info_runway,
    format_info_load_summary, format_info_pool_remaining, format_info_runway,
    format_info_token_usage_summary, load_runtime_usage_snapshots, read_runtime_log_tail,
};

mod render;
mod resource;

#[cfg(test)]
use render::text_sparkline;
use render::{render_status_dashboard, status_fields};
use resource::{collect_status_resource_counters, status_resource_snapshot};
#[cfg(test)]
use resource::{
    parse_kib_field, parse_network_queues, parse_process_cpu_ticks, parse_system_cpu_ticks,
    parse_u64_field,
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
    print_panel("Status", &status_fields(&overview, &resources))?;
    Ok(())
}

fn watch_status(resource_interval: Duration) -> Result<()> {
    let mut tui = LogTuiTerminal::stdout("status TUI")?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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

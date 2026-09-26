use super::log_tui::LogTuiTerminal;
use crate::reports::{
    InfoTokenUsageEvent, InfoTokenUsageSummary, collect_info_token_usage_summary_from_texts,
    info_token_usage_event_from_line, runtime_usage_snapshot_is_usable,
    usage_from_runtime_usage_snapshot,
};
use anyhow::Result;
use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use prodex_quota::{
    QuotaPoolWindowInput, StatusQuotaProfileInput, required_main_window_snapshot_at,
    status_quota_summary_batch,
};
use prodex_runtime_doctor::read_runtime_log_tail;
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
    format_info_token_usage_summary, load_runtime_usage_snapshots,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct StatusQuotaWindow {
    profiles: usize,
    total_remaining: i64,
    earliest_reset_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
        update_status_state(StatusUpdateContext {
            now,
            resource_interval,
            refresh: &mut refresh,
            overview: &mut overview,
            overview_error: &mut overview_error,
            resources: &mut resources,
            resource_snapshot: &mut resource_snapshot,
            history: &mut history,
            next_overview_refresh: &mut next_overview_refresh,
            next_resource_sample: &mut next_resource_sample,
            dirty: &mut dirty,
        });

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
        if handle_status_input(
            &mut next_overview_refresh,
            &mut next_resource_sample,
            &mut dirty,
        )? {
            return Ok(());
        }
    }
}

struct StatusUpdateContext<'a> {
    now: Instant,
    resource_interval: Duration,
    refresh: &'a mut StatusRefresh,
    overview: &'a mut Option<StatusOverview>,
    overview_error: &'a mut Option<String>,
    resources: &'a mut StatusResourceTracker,
    resource_snapshot: &'a mut StatusResourceSnapshot,
    history: &'a mut StatusResourceHistory,
    next_overview_refresh: &'a mut Instant,
    next_resource_sample: &'a mut Instant,
    dirty: &'a mut bool,
}

fn update_status_state(context: StatusUpdateContext<'_>) {
    let StatusUpdateContext {
        now,
        resource_interval,
        refresh,
        overview,
        overview_error,
        resources,
        resource_snapshot,
        history,
        next_overview_refresh,
        next_resource_sample,
        dirty,
    } = context;
    if now >= *next_overview_refresh {
        refresh.start();
        *next_overview_refresh = now + STATUS_OVERVIEW_REFRESH;
    }
    if let Some(result) = refresh.take() {
        match result {
            Ok(snapshot) => {
                *overview = Some(snapshot);
                *overview_error = None;
            }
            Err(error) => *overview_error = Some(error),
        }
        *dirty = true;
    }
    if now >= *next_resource_sample {
        *resource_snapshot = resources.sample();
        history.push(resource_snapshot);
        *next_resource_sample = now + resource_interval;
        *dirty = true;
    }
}

fn handle_status_input(
    next_overview_refresh: &mut Instant,
    next_resource_sample: &mut Instant,
    dirty: &mut bool,
) -> Result<bool> {
    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            if status_quit_key(&key) {
                return Ok(true);
            }
            if matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')) {
                *next_overview_refresh = Instant::now();
                *next_resource_sample = Instant::now();
            }
        }
        Event::Resize(_, _) => *dirty = true,
        _ => {}
    }
    Ok(false)
}

fn status_quit_key(key: &crossterm::event::KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('z')))
}

fn collect_status_overview(paths: &AppPaths) -> Result<StatusOverview> {
    let state = AppState::load(paths)?;
    let now = Local::now().timestamp();
    let quota = collect_status_quota(paths, &state, now)?;
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

fn collect_status_quota(
    paths: &AppPaths,
    state: &AppState,
    now: i64,
) -> Result<StatusQuotaSummary> {
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
) -> Result<StatusQuotaSummary> {
    let mut inputs = Vec::with_capacity(reports.len());
    for report in reports {
        let report_succeeded = report.result.is_ok();
        let report_windows = report
            .auth
            .quota_compatible
            .then(|| {
                report
                    .result
                    .as_ref()
                    .ok()
                    .map(|usage| status_quota_windows(usage, now))
            })
            .flatten()
            .unwrap_or_default();
        let cached_usage = if report.auth.quota_compatible && !report_succeeded {
            snapshots.get(&report.name).filter(|snapshot| {
                runtime_usage_snapshot_is_usable(
                    snapshot,
                    now,
                    RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
                )
            })
        } else {
            None
        };
        let cached_snapshot_usable = cached_usage.is_some();
        let cached_windows = cached_usage
            .map(usage_from_runtime_usage_snapshot)
            .as_ref()
            .map(|usage| status_quota_windows(usage, now))
            .unwrap_or_default();
        inputs.push(StatusQuotaProfileInput {
            quota_compatible: report.auth.quota_compatible,
            report_succeeded,
            cached_snapshot_usable,
            report_five_hour: report_windows.0,
            report_weekly: report_windows.1,
            cached_five_hour: cached_windows.0,
            cached_weekly: cached_windows.1,
        });
    }

    let summary = status_quota_summary_batch(&inputs)
        .map_err(|error| anyhow::anyhow!("Mojo status quota summary failed: {error:?}"))?;
    Ok(StatusQuotaSummary {
        compatible_profiles: summary.compatible_profiles,
        unavailable_profiles: summary.unavailable_profiles,
        five_hour: StatusQuotaWindow {
            profiles: summary.five_hour.profiles,
            total_remaining: summary.five_hour.total_remaining,
            earliest_reset_at: summary.five_hour.earliest_reset_at,
        },
        weekly: StatusQuotaWindow {
            profiles: summary.weekly.profiles,
            total_remaining: summary.weekly.total_remaining,
            earliest_reset_at: summary.weekly.earliest_reset_at,
        },
    })
}

fn status_quota_windows(
    usage: &prodex_quota::UsageResponse,
    now: i64,
) -> (Option<QuotaPoolWindowInput>, Option<QuotaPoolWindowInput>) {
    let window = |label| {
        required_main_window_snapshot_at(usage, label, now).map(|snapshot| QuotaPoolWindowInput {
            remaining_percent: snapshot.remaining_percent,
            reset_at: snapshot.reset_at,
        })
    };
    (window("5h"), window("weekly"))
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
mod tests;

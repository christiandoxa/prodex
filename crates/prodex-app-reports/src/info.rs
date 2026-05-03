use prodex_quota::{
    MainWindowSnapshot, RuntimeQuotaWindowStatus, UsageResponse, UsageWindow, WindowPair,
    find_main_window, format_precise_reset_time, remaining_percent,
};
use prodex_runtime_state::RuntimeProfileUsageSnapshot as RuntimeProfileUsageSnapshotGeneric;
use prodex_runtime_tuning::RuntimeTuningSnapshot;
use prodex_shared_types::{
    InfoQuotaAggregate, InfoQuotaSource, InfoQuotaWindow, InfoRuntimeLoadSummary,
    InfoRuntimeQuotaObservation, InfoRunwayEstimate, ProcessRow, ProdexProcessInfo,
    RunProfileProbeReport,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub type RuntimeProfileUsageSnapshot = RuntimeProfileUsageSnapshotGeneric<RuntimeQuotaWindowStatus>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InfoTokenUsageCounts {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InfoTokenUsageSummary {
    pub log_count: usize,
    pub event_count: usize,
    pub total: InfoTokenUsageCounts,
    pub by_profile: BTreeMap<String, InfoTokenUsageProfile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InfoTokenUsageProfile {
    pub event_count: usize,
    pub total: InfoTokenUsageCounts,
}

pub fn build_info_quota_aggregate(
    reports: &[RunProfileProbeReport],
    persisted_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
    stale_grace_seconds: i64,
) -> InfoQuotaAggregate {
    let mut aggregate = InfoQuotaAggregate {
        quota_compatible_profiles: reports
            .iter()
            .filter(|report| report.auth.quota_compatible)
            .count(),
        live_profiles: 0,
        snapshot_profiles: 0,
        unavailable_profiles: 0,
        five_hour_pool_remaining: 0,
        weekly_pool_remaining: 0,
        earliest_five_hour_reset_at: None,
        earliest_weekly_reset_at: None,
    };

    for report in reports {
        if !report.auth.quota_compatible {
            continue;
        }

        let usage = match &report.result {
            Ok(usage) => Some((usage.clone(), InfoQuotaSource::LiveProbe)),
            Err(_) => persisted_usage_snapshots
                .get(&report.name)
                .filter(|snapshot| {
                    runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
                })
                .map(|snapshot| {
                    (
                        usage_from_runtime_usage_snapshot(snapshot),
                        InfoQuotaSource::PersistedSnapshot,
                    )
                }),
        };

        let Some((usage, source)) = usage else {
            aggregate.unavailable_profiles += 1;
            continue;
        };

        let Some((five_hour, weekly)) = info_main_window_snapshots_at(&usage, now) else {
            aggregate.unavailable_profiles += 1;
            continue;
        };

        match source {
            InfoQuotaSource::LiveProbe => aggregate.live_profiles += 1,
            InfoQuotaSource::PersistedSnapshot => aggregate.snapshot_profiles += 1,
        }
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

pub fn info_main_window_snapshots_at(
    usage: &UsageResponse,
    now: i64,
) -> Option<(MainWindowSnapshot, MainWindowSnapshot)> {
    Some((
        required_main_window_snapshot_at(usage, "5h", now)?,
        required_main_window_snapshot_at(usage, "weekly", now)?,
    ))
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

pub fn usage_from_runtime_usage_snapshot(snapshot: &RuntimeProfileUsageSnapshot) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.five_hour_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.five_hour_reset_at != i64::MAX)
                    .then_some(snapshot.five_hour_reset_at),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.weekly_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.weekly_reset_at != i64::MAX)
                    .then_some(snapshot.weekly_reset_at),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

pub fn runtime_usage_snapshot_is_usable(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
    stale_grace_seconds: i64,
) -> bool {
    if runtime_profile_usage_snapshot_hold_active(snapshot, now) {
        return true;
    }
    if runtime_profile_usage_snapshot_hold_expired(snapshot, now) {
        return false;
    }
    now.saturating_sub(snapshot.checked_at) <= stale_grace_seconds
}

pub fn runtime_profile_usage_snapshot_hold_active(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at > now
    })
}

pub fn runtime_profile_usage_snapshot_hold_expired(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at <= now
    })
}

pub fn collect_info_token_usage_summary_from_text(text: &str) -> InfoTokenUsageSummary {
    let mut summary = InfoTokenUsageSummary::default();
    for line in text.lines() {
        let Some((profile, usage)) = info_token_usage_from_line(line) else {
            continue;
        };
        summary.event_count += 1;
        summary.total.add(usage);
        let profile = summary.by_profile.entry(profile).or_default();
        profile.event_count += 1;
        profile.total.add(usage);
    }
    summary
}

pub fn collect_info_token_usage_summary_from_texts<I, S>(
    log_count: usize,
    texts: I,
) -> InfoTokenUsageSummary
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut summary = InfoTokenUsageSummary {
        log_count,
        ..InfoTokenUsageSummary::default()
    };
    for text in texts {
        summary.merge(collect_info_token_usage_summary_from_text(text.as_ref()));
    }
    summary
}

impl InfoTokenUsageSummary {
    pub fn merge(&mut self, other: InfoTokenUsageSummary) {
        self.event_count += other.event_count;
        self.total.add(other.total);
        for (name, other_profile) in other.by_profile {
            let profile = self.by_profile.entry(name).or_default();
            profile.event_count += other_profile.event_count;
            profile.total.add(other_profile.total);
        }
    }
}

impl InfoTokenUsageCounts {
    fn add(&mut self, other: InfoTokenUsageCounts) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(other.reasoning_tokens);
    }
}

pub fn info_token_usage_from_line(line: &str) -> Option<(String, InfoTokenUsageCounts)> {
    if !line.contains("token_usage") {
        return None;
    }
    let fields = info_runtime_parse_fields(line);
    Some((
        fields
            .get("profile")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        InfoTokenUsageCounts {
            input_tokens: fields.get("input_tokens")?.parse::<u64>().ok()?,
            cached_input_tokens: fields
                .get("cached_input_tokens")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or_default(),
            output_tokens: fields
                .get("output_tokens")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or_default(),
            reasoning_tokens: fields
                .get("reasoning_tokens")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or_default(),
        },
    ))
}

pub fn collect_info_runtime_load_summary_from_text(
    text: &str,
    now: i64,
    recent_window_seconds: i64,
    lookback_seconds: i64,
) -> InfoRuntimeLoadSummary {
    let recent_cutoff = now.saturating_sub(recent_window_seconds);
    let lookback_cutoff = now.saturating_sub(lookback_seconds);
    let mut summary = InfoRuntimeLoadSummary::default();
    let mut latest_inflight_counts = BTreeMap::new();

    for line in text.lines() {
        if let Some((profile, count)) = info_runtime_inflight_from_line(line) {
            latest_inflight_counts.insert(profile, count);
        }
        let Some(observation) = info_runtime_selection_observation_from_line(line) else {
            continue;
        };
        if observation.timestamp >= lookback_cutoff {
            summary.observations.push(observation.clone());
        }
        if observation.timestamp >= recent_cutoff {
            summary.recent_selection_events += 1;
            summary.recent_first_timestamp = Some(
                summary
                    .recent_first_timestamp
                    .map_or(observation.timestamp, |current| {
                        current.min(observation.timestamp)
                    }),
            );
            summary.recent_last_timestamp = Some(
                summary
                    .recent_last_timestamp
                    .map_or(observation.timestamp, |current| {
                        current.max(observation.timestamp)
                    }),
            );
        }
    }

    summary.active_inflight_units = latest_inflight_counts.values().sum();
    summary
}

pub fn collect_info_runtime_load_summary_from_texts<I, S>(
    log_count: usize,
    texts: I,
    now: i64,
    recent_window_seconds: i64,
    lookback_seconds: i64,
) -> InfoRuntimeLoadSummary
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut summary = InfoRuntimeLoadSummary {
        log_count,
        ..InfoRuntimeLoadSummary::default()
    };

    for text in texts {
        let log_summary = collect_info_runtime_load_summary_from_text(
            text.as_ref(),
            now,
            recent_window_seconds,
            lookback_seconds,
        );
        summary.observations.extend(log_summary.observations);
        summary.active_inflight_units += log_summary.active_inflight_units;
        summary.recent_selection_events += log_summary.recent_selection_events;
        summary.recent_first_timestamp = match (
            summary.recent_first_timestamp,
            log_summary.recent_first_timestamp,
        ) {
            (Some(current), Some(candidate)) => Some(current.min(candidate)),
            (current, candidate) => current.or(candidate),
        };
        summary.recent_last_timestamp = match (
            summary.recent_last_timestamp,
            log_summary.recent_last_timestamp,
        ) {
            (Some(current), Some(candidate)) => Some(current.max(candidate)),
            (current, candidate) => current.or(candidate),
        };
    }

    summary
        .observations
        .sort_by_key(|observation| (observation.timestamp, observation.profile.clone()));
    summary
}

pub fn info_runtime_selection_observation_from_line(
    line: &str,
) -> Option<InfoRuntimeQuotaObservation> {
    if !line.contains("selection_pick") && !line.contains("selection_keep_current") {
        return None;
    }
    let timestamp = runtime_log_timestamp_epoch(line)?;
    let fields = info_runtime_parse_fields(line);
    let profile = fields.get("profile")?;
    if profile == "none" {
        return None;
    }
    Some(InfoRuntimeQuotaObservation {
        timestamp,
        profile: profile.clone(),
        five_hour_remaining: fields.get("five_hour_remaining")?.parse::<i64>().ok()?,
        weekly_remaining: fields.get("weekly_remaining")?.parse::<i64>().ok()?,
    })
}

pub fn info_runtime_inflight_from_line(line: &str) -> Option<(String, usize)> {
    if !line.contains("profile_inflight ") {
        return None;
    }
    let fields = info_runtime_parse_fields(line);
    Some((
        fields.get("profile")?.clone(),
        fields.get("count")?.parse::<usize>().ok()?,
    ))
}

pub fn runtime_log_timestamp_epoch(line: &str) -> Option<i64> {
    let timestamp = info_runtime_line_timestamp(line)?;
    chrono::DateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S%.f %:z")
        .or_else(|_| chrono::DateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S %:z"))
        .ok()
        .map(|datetime| datetime.timestamp())
}

pub fn info_runtime_line_timestamp(line: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
        return value
            .get("timestamp")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
    }
    let end = line.find("] ")?;
    line.strip_prefix('[')
        .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
        .map(ToString::to_string)
}

pub fn info_runtime_parse_fields(line: &str) -> BTreeMap<String, String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
        && let Some(fields) = value.get("fields").and_then(serde_json::Value::as_object)
    {
        return fields
            .iter()
            .filter_map(|(key, value)| {
                info_runtime_json_field_value(value).map(|value| (key.clone(), value))
            })
            .collect();
    }
    let message = line
        .split_once("] ")
        .map(|(_, message)| message)
        .unwrap_or(line)
        .trim();
    let mut fields = BTreeMap::new();
    for token in message.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        if key.is_empty() || value.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), value.trim_matches('"').to_string());
    }
    fields
}

fn info_runtime_json_field_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub fn estimate_info_runway(
    observations: &[InfoRuntimeQuotaObservation],
    window: InfoQuotaWindow,
    current_remaining: i64,
    now: i64,
    min_span_seconds: i64,
) -> Option<InfoRunwayEstimate> {
    if current_remaining <= 0 {
        return Some(InfoRunwayEstimate {
            burn_per_hour: 0.0,
            observed_profiles: 0,
            observed_span_seconds: 0,
            exhaust_at: now,
        });
    }

    let mut by_profile = BTreeMap::<String, Vec<&InfoRuntimeQuotaObservation>>::new();
    for observation in observations {
        by_profile
            .entry(observation.profile.clone())
            .or_default()
            .push(observation);
    }

    let mut burn_per_hour = 0.0;
    let mut observed_profiles = 0;
    let mut earliest = i64::MAX;
    let mut latest = i64::MIN;

    for profile_observations in by_profile.values_mut() {
        profile_observations.sort_by_key(|observation| observation.timestamp);
        let Some((profile_burn_per_hour, start, end)) =
            info_profile_window_burn_rate(profile_observations, window, min_span_seconds)
        else {
            continue;
        };
        burn_per_hour += profile_burn_per_hour;
        observed_profiles += 1;
        earliest = earliest.min(start);
        latest = latest.max(end);
    }

    if burn_per_hour <= 0.0 || observed_profiles == 0 || earliest == i64::MAX || latest == i64::MIN
    {
        return None;
    }

    let seconds_until_exhaustion =
        ((current_remaining as f64 / burn_per_hour) * 3600.0).ceil() as i64;

    Some(InfoRunwayEstimate {
        burn_per_hour,
        observed_profiles,
        observed_span_seconds: latest.saturating_sub(earliest),
        exhaust_at: now.saturating_add(seconds_until_exhaustion.max(0)),
    })
}

pub fn info_profile_window_burn_rate(
    observations: &[&InfoRuntimeQuotaObservation],
    window: InfoQuotaWindow,
    min_span_seconds: i64,
) -> Option<(f64, i64, i64)> {
    if observations.len() < 2 {
        return None;
    }

    let latest = observations.last()?;
    let mut earliest = *latest;
    let mut current_remaining = info_observation_window_remaining(latest, window);

    for observation in observations.iter().rev().skip(1) {
        let remaining = info_observation_window_remaining(observation, window);
        if remaining < current_remaining {
            break;
        }
        earliest = *observation;
        current_remaining = remaining;
    }

    let earliest_remaining = info_observation_window_remaining(earliest, window);
    let latest_remaining = info_observation_window_remaining(latest, window);
    let burned = earliest_remaining.saturating_sub(latest_remaining);
    let span_seconds = latest.timestamp.saturating_sub(earliest.timestamp);
    if burned <= 0 || span_seconds < min_span_seconds {
        return None;
    }

    Some((
        burned as f64 * 3600.0 / span_seconds as f64,
        earliest.timestamp,
        latest.timestamp,
    ))
}

pub fn info_observation_window_remaining(
    observation: &InfoRuntimeQuotaObservation,
    window: InfoQuotaWindow,
) -> i64 {
    match window {
        InfoQuotaWindow::FiveHour => observation.five_hour_remaining,
        InfoQuotaWindow::Weekly => observation.weekly_remaining,
    }
}

pub fn parse_ps_process_rows(text: &str) -> Vec<ProcessRow> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 {
            continue;
        }
        let Ok(pid) = tokens[0].parse::<u32>() else {
            continue;
        };
        rows.push(ProcessRow {
            pid,
            command: tokens[1].to_string(),
            args: tokens
                .iter()
                .skip(2)
                .map(|token| (*token).to_string())
                .collect(),
        });
    }
    rows
}

pub fn classify_prodex_process_row(
    row: ProcessRow,
    current_pid: u32,
    current_basename: Option<&str>,
) -> Option<ProdexProcessInfo> {
    if row.pid == current_pid || !is_prodex_process_row(&row, current_basename) {
        return None;
    }

    Some(ProdexProcessInfo {
        pid: row.pid,
        runtime: prodex_process_row_is_runtime(&row, current_basename),
    })
}

pub fn is_prodex_process_row(row: &ProcessRow, current_basename: Option<&str>) -> bool {
    let command_base = process_basename(&row.command);
    process_basename_matches(command_base, current_basename)
        || prodex_process_row_argv_span(row, current_basename).is_some()
}

pub fn prodex_process_row_is_runtime(row: &ProcessRow, current_basename: Option<&str>) -> bool {
    prodex_process_row_argv_span(row, current_basename)
        .and_then(|args| args.get(1))
        .is_some_and(|arg| arg == "run" || arg == "__runtime-broker")
}

pub fn prodex_process_row_argv_span<'a>(
    row: &'a ProcessRow,
    current_basename: Option<&str>,
) -> Option<&'a [String]> {
    row.args.iter().enumerate().find_map(|(index, arg)| {
        process_basename_matches(process_basename(arg), current_basename)
            .then_some(&row.args[index..])
    })
}

pub fn process_basename_matches(candidate: &str, current_basename: Option<&str>) -> bool {
    candidate == "prodex" || current_basename.is_some_and(|name| candidate == name)
}

pub fn process_basename(input: &str) -> &str {
    Path::new(input)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(input)
}

pub fn runtime_log_pid_from_path(path: &Path) -> Option<u32> {
    runtime_log_pid_from_path_with_prefix(path, "prodex-runtime")
}

pub fn runtime_log_pid_from_path_with_prefix(path: &Path, prefix: &str) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix(&format!("{prefix}-"))?;
    let (pid, _) = rest.split_once('-')?;
    pid.parse::<u32>().ok()
}

pub fn select_active_runtime_log_paths<I>(
    processes: &[ProdexProcessInfo],
    log_paths: I,
) -> Vec<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    select_active_runtime_log_paths_with_prefix(processes, log_paths, "prodex-runtime")
}

pub fn select_active_runtime_log_paths_with_prefix<I>(
    processes: &[ProdexProcessInfo],
    log_paths: I,
    prefix: &str,
) -> Vec<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    let runtime_pids = processes
        .iter()
        .filter(|process| process.runtime)
        .map(|process| process.pid)
        .collect::<BTreeSet<_>>();
    if runtime_pids.is_empty() {
        return Vec::new();
    }

    let mut latest_logs = BTreeMap::new();
    for path in log_paths {
        let Some(pid) = runtime_log_pid_from_path_with_prefix(&path, prefix) else {
            continue;
        };
        if runtime_pids.contains(&pid) {
            latest_logs.insert(pid, path);
        }
    }
    latest_logs.into_values().collect()
}

pub fn select_recent_runtime_log_paths<I>(log_paths: I, limit: usize) -> Vec<PathBuf>
where
    I: IntoIterator<Item = (PathBuf, SystemTime)>,
{
    let mut paths = log_paths.into_iter().collect::<Vec<_>>();
    paths.sort_by(|(left_path, left_modified), (right_path, right_modified)| {
        left_modified
            .cmp(right_modified)
            .then_with(|| left_path.cmp(right_path))
    });
    paths.reverse();
    paths.truncate(limit);
    paths.into_iter().map(|(path, _)| path).collect()
}

pub fn format_info_process_summary(processes: &[ProdexProcessInfo]) -> String {
    let runtime_count = processes.iter().filter(|process| process.runtime).count();
    terminal_ui::format_info_process_summary_display(
        processes.len(),
        runtime_count,
        processes.iter().map(|process| process.pid),
        6,
    )
}

pub fn format_info_load_summary(
    summary: &InfoRuntimeLoadSummary,
    runtime_process_count: usize,
    recent_window_seconds: i64,
) -> String {
    terminal_ui::format_info_load_summary_display(
        terminal_ui::InfoLoadSummaryDisplay {
            log_count: summary.log_count,
            active_inflight_units: summary.active_inflight_units,
            recent_selection_events: summary.recent_selection_events,
            recent_first_timestamp: summary.recent_first_timestamp,
            recent_last_timestamp: summary.recent_last_timestamp,
        },
        runtime_process_count,
        recent_window_seconds,
    )
}

pub fn format_runtime_policy_summary(path: Option<&str>, version: Option<u32>) -> String {
    terminal_ui::format_runtime_policy_summary_display(path, version)
}

pub fn runtime_policy_json_value(path: Option<&str>, version: Option<u32>) -> serde_json::Value {
    path.zip(version)
        .map(|(path, version)| {
            serde_json::json!({
                "path": path,
                "version": version,
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

pub fn format_runtime_logs_summary(directory: &str, format: &str) -> String {
    terminal_ui::format_runtime_logs_summary_display(directory, format)
}

pub fn runtime_logs_json_value(directory: &str, format: &str) -> serde_json::Value {
    serde_json::json!({
        "directory": directory,
        "format": format,
    })
}

pub fn format_secret_backend_summary_parts(
    backend: Option<&str>,
    keyring_service: Option<&str>,
    error: Option<&str>,
) -> String {
    if let Some(err) = error {
        return format!("invalid ({err})");
    }

    match (backend, keyring_service) {
        (Some(backend), Some(service)) => format!("{backend} ({service})"),
        (Some(backend), None) => backend.to_string(),
        (None, _) => "invalid (missing backend)".to_string(),
    }
}

pub fn secret_backend_json_value_parts(
    backend: Option<&str>,
    keyring_service: Option<&str>,
    error: Option<&str>,
) -> serde_json::Value {
    if let Some(err) = error {
        return serde_json::json!({
            "invalid": true,
            "error": err,
        });
    }

    serde_json::json!({
        "backend": backend,
        "keyring_service": keyring_service,
    })
}

pub fn format_info_quota_data_summary(aggregate: &InfoQuotaAggregate) -> String {
    terminal_ui::format_info_quota_data_summary_display(
        aggregate.quota_compatible_profiles,
        aggregate.live_profiles,
        aggregate.snapshot_profiles,
        aggregate.unavailable_profiles,
    )
}

pub fn format_info_token_usage_summary(summary: &InfoTokenUsageSummary) -> String {
    let by_profile =
        summary
            .by_profile
            .iter()
            .map(|(profile, data)| terminal_ui::TokenUsageProfileDisplay {
                profile,
                total: token_usage_counts(data.total),
            });

    terminal_ui::format_info_token_usage_summary_display(
        summary.event_count,
        summary.log_count,
        token_usage_counts(summary.total),
        by_profile,
    )
}

fn token_usage_counts(usage: InfoTokenUsageCounts) -> terminal_ui::TokenUsageCounts {
    terminal_ui::TokenUsageCounts {
        input_tokens: usage.input_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        output_tokens: usage.output_tokens,
        reasoning_tokens: usage.reasoning_tokens,
    }
}

pub fn format_runtime_tuning_workers(snapshot: &RuntimeTuningSnapshot) -> String {
    terminal_ui::format_runtime_tuning_workers_display(terminal_ui::RuntimeTuningWorkersDisplay {
        worker_count: snapshot.worker_count,
        long_lived_worker_count: snapshot.long_lived_worker_count,
        async_worker_count: snapshot.async_worker_count,
        probe_refresh_worker_count: snapshot.probe_refresh_worker_count,
        active_request_limit: snapshot.active_request_limit,
        long_lived_queue_capacity: snapshot.long_lived_queue_capacity,
        lane_responses: snapshot.lane_limits.responses,
        lane_compact: snapshot.lane_limits.compact,
        lane_websocket: snapshot.lane_limits.websocket,
        lane_standard: snapshot.lane_limits.standard,
        websocket_connect_worker_count: snapshot.websocket_connect_worker_count,
        websocket_connect_queue_capacity: snapshot.websocket_connect_queue_capacity,
        websocket_connect_overflow_capacity: snapshot.websocket_connect_overflow_capacity,
        websocket_dns_worker_count: snapshot.websocket_dns_worker_count,
        websocket_dns_queue_capacity: snapshot.websocket_dns_queue_capacity,
        websocket_dns_overflow_capacity: snapshot.websocket_dns_overflow_capacity,
    })
}

pub fn format_runtime_tuning_budgets(snapshot: &RuntimeTuningSnapshot) -> String {
    terminal_ui::format_runtime_tuning_budgets_display(terminal_ui::RuntimeTuningBudgetsDisplay {
        precommit_attempt_limit: snapshot.precommit_attempt_limit,
        precommit_budget_ms: snapshot.precommit_budget_ms,
        pressure_precommit_attempt_limit: snapshot.pressure_precommit_attempt_limit,
        pressure_precommit_budget_ms: snapshot.pressure_precommit_budget_ms,
        continuation_precommit_attempt_limit: snapshot.continuation_precommit_attempt_limit,
        continuation_precommit_budget_ms: snapshot.continuation_precommit_budget_ms,
        admission_wait_budget_ms: snapshot.admission_wait_budget_ms,
        pressure_admission_wait_budget_ms: snapshot.pressure_admission_wait_budget_ms,
        long_lived_queue_wait_budget_ms: snapshot.long_lived_queue_wait_budget_ms,
        pressure_long_lived_queue_wait_budget_ms: snapshot.pressure_long_lived_queue_wait_budget_ms,
    })
}

pub fn format_runtime_tuning_transport(snapshot: &RuntimeTuningSnapshot) -> String {
    terminal_ui::format_runtime_tuning_transport_display(
        terminal_ui::RuntimeTuningTransportDisplay {
            http_connect_timeout_ms: snapshot.http_connect_timeout_ms,
            stream_idle_timeout_ms: snapshot.stream_idle_timeout_ms,
            sse_lookahead_timeout_ms: snapshot.sse_lookahead_timeout_ms,
            websocket_connect_timeout_ms: snapshot.websocket_connect_timeout_ms,
            websocket_precommit_progress_timeout_ms: snapshot
                .websocket_precommit_progress_timeout_ms,
            websocket_happy_eyeballs_delay_ms: snapshot.websocket_happy_eyeballs_delay_ms,
            websocket_previous_response_reuse_stale_ms: snapshot
                .websocket_previous_response_reuse_stale_ms,
            profile_inflight_soft_limit: snapshot.profile_inflight_soft_limit,
            profile_inflight_hard_limit: snapshot.profile_inflight_hard_limit,
        },
    )
}

pub fn format_info_pool_remaining(
    total_remaining: i64,
    profiles_with_data: usize,
    earliest_reset_at: Option<i64>,
) -> String {
    let earliest_reset =
        earliest_reset_at.map(|reset_at| format_precise_reset_time(Some(reset_at)));
    terminal_ui::format_info_pool_remaining_display(
        total_remaining,
        profiles_with_data,
        earliest_reset.as_deref(),
    )
}

pub fn format_info_runway(
    profiles_with_data: usize,
    current_remaining: i64,
    earliest_reset_at: Option<i64>,
    estimate: Option<&InfoRunwayEstimate>,
    now: i64,
) -> String {
    let reset_text =
        earliest_reset_at.map(|reset_at| (reset_at, format_precise_reset_time(Some(reset_at))));
    let earliest_reset =
        reset_text.as_ref().map(
            |(reset_at, reset_text)| terminal_ui::InfoRunwayResetDisplay {
                reset_at: *reset_at,
                reset_text,
            },
        );
    let exhaust_text =
        estimate.map(|estimate| format_precise_reset_time(Some(estimate.exhaust_at)));
    let estimate = estimate
        .zip(exhaust_text.as_deref())
        .map(
            |(estimate, exhaust_text)| terminal_ui::InfoRunwayEstimateDisplay {
                burn_per_hour: estimate.burn_per_hour,
                observed_profiles: estimate.observed_profiles,
                observed_span_seconds: estimate.observed_span_seconds,
                exhaust_at: estimate.exhaust_at,
                exhaust_text,
            },
        );

    terminal_ui::format_info_runway_display(
        profiles_with_data,
        current_remaining,
        earliest_reset,
        estimate,
        now,
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/crates/prodex-app-reports/src/info.rs"]
mod tests;

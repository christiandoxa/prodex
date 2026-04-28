use super::*;

pub(crate) fn collect_info_quota_aggregate(
    paths: &AppPaths,
    state: &AppState,
    now: i64,
) -> InfoQuotaAggregate {
    if state.profiles.is_empty() {
        return InfoQuotaAggregate {
            quota_compatible_profiles: 0,
            live_profiles: 0,
            snapshot_profiles: 0,
            unavailable_profiles: 0,
            five_hour_pool_remaining: 0,
            weekly_pool_remaining: 0,
            earliest_five_hour_reset_at: None,
            earliest_weekly_reset_at: None,
        };
    }

    let persisted_usage_snapshots =
        load_runtime_usage_snapshots(paths, &state.profiles).unwrap_or_default();
    let reports =
        collect_run_profile_reports(state, state.profiles.keys().cloned().collect(), None, false);
    build_info_quota_aggregate(&reports, &persisted_usage_snapshots, now)
}

pub(crate) fn build_info_quota_aggregate(
    reports: &[RunProfileProbeReport],
    persisted_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
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
                .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
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

        let Some((five_hour, weekly)) = info_main_window_snapshots(&usage) else {
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

pub(crate) fn info_main_window_snapshots(
    usage: &UsageResponse,
) -> Option<(MainWindowSnapshot, MainWindowSnapshot)> {
    Some((
        required_main_window_snapshot(usage, "5h")?,
        required_main_window_snapshot(usage, "weekly")?,
    ))
}

pub(crate) fn collect_prodex_processes() -> Vec<ProdexProcessInfo> {
    let current_pid = std::process::id();
    let current_basename = std::env::current_exe().ok().and_then(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
    });

    let mut processes = collect_process_rows()
        .into_iter()
        .filter_map(|row| {
            classify_prodex_process_row(row, current_pid, current_basename.as_deref())
        })
        .collect::<Vec<_>>();
    processes.sort_by_key(|process| process.pid);
    processes
}

pub(crate) fn collect_process_rows() -> Vec<ProcessRow> {
    collect_process_rows_from_proc()
        .or_else(|| collect_process_rows_from_ps().ok())
        .unwrap_or_default()
}

pub(crate) fn collect_process_rows_from_proc() -> Option<Vec<ProcessRow>> {
    let mut rows = Vec::new();
    let entries = fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let dir = entry.path();
        let Some(command) = fs::read_to_string(dir.join("comm"))
            .ok()
            .map(|value| value.trim().to_string())
        else {
            continue;
        };
        let Some(args_bytes) = fs::read(dir.join("cmdline")).ok() else {
            continue;
        };
        let args = args_bytes
            .split(|byte| *byte == 0)
            .filter_map(|chunk| {
                if chunk.is_empty() {
                    return None;
                }
                String::from_utf8(chunk.to_vec()).ok()
            })
            .collect::<Vec<_>>();
        rows.push(ProcessRow { pid, command, args });
    }
    Some(rows)
}

pub(crate) fn collect_process_rows_from_ps() -> Result<Vec<ProcessRow>> {
    let output = Command::new("ps")
        .args(["-Ao", "pid=,comm=,args="])
        .output()
        .context("failed to execute ps for prodex process listing")?;
    if !output.status.success() {
        bail!("ps returned exit status {}", output.status);
    }
    let text = String::from_utf8(output.stdout).context("ps output was not valid UTF-8")?;
    Ok(parse_ps_process_rows(&text))
}

pub(crate) fn parse_ps_process_rows(text: &str) -> Vec<ProcessRow> {
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

pub(crate) fn classify_prodex_process_row(
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

pub(crate) fn is_prodex_process_row(row: &ProcessRow, current_basename: Option<&str>) -> bool {
    let command_base = process_basename(&row.command);
    process_basename_matches(command_base, current_basename)
        || prodex_process_row_argv_span(row, current_basename).is_some()
}

pub(crate) fn prodex_process_row_is_runtime(
    row: &ProcessRow,
    current_basename: Option<&str>,
) -> bool {
    prodex_process_row_argv_span(row, current_basename)
        .and_then(|args| args.get(1))
        .is_some_and(|arg| arg == "run" || arg == "__runtime-broker")
}

pub(crate) fn prodex_process_row_argv_span<'a>(
    row: &'a ProcessRow,
    current_basename: Option<&str>,
) -> Option<&'a [String]> {
    row.args.iter().enumerate().find_map(|(index, arg)| {
        process_basename_matches(process_basename(arg), current_basename)
            .then_some(&row.args[index..])
    })
}

pub(crate) fn process_basename_matches(candidate: &str, current_basename: Option<&str>) -> bool {
    candidate == "prodex" || current_basename.is_some_and(|name| candidate == name)
}

pub(crate) fn process_basename(input: &str) -> &str {
    Path::new(input)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(input)
}

pub(crate) fn collect_active_runtime_log_paths(processes: &[ProdexProcessInfo]) -> Vec<PathBuf> {
    let runtime_pids = processes
        .iter()
        .filter(|process| process.runtime)
        .map(|process| process.pid)
        .collect::<BTreeSet<_>>();
    if runtime_pids.is_empty() {
        return Vec::new();
    }

    let mut latest_logs = BTreeMap::new();
    for path in prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()) {
        let Some(pid) = runtime_log_pid_from_path(&path) else {
            continue;
        };
        if runtime_pids.contains(&pid) {
            latest_logs.insert(pid, path);
        }
    }
    latest_logs.into_values().collect()
}

pub(crate) fn runtime_log_pid_from_path(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix(&format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-"))?;
    let (pid, _) = rest.split_once('-')?;
    pid.parse::<u32>().ok()
}

pub(crate) fn collect_info_runtime_load_summary(
    log_paths: &[PathBuf],
    now: i64,
) -> InfoRuntimeLoadSummary {
    let mut summary = InfoRuntimeLoadSummary {
        log_count: log_paths.len(),
        ..InfoRuntimeLoadSummary::default()
    };

    for path in log_paths {
        let Ok(tail) = read_runtime_log_tail(path, INFO_RUNTIME_LOG_TAIL_BYTES) else {
            continue;
        };
        let log_summary = collect_info_runtime_load_summary_from_text(
            &String::from_utf8_lossy(&tail),
            now,
            INFO_RECENT_LOAD_WINDOW_SECONDS,
            INFO_FORECAST_LOOKBACK_SECONDS,
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

pub(crate) fn collect_info_runtime_load_summary_from_text(
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

pub(crate) fn info_runtime_selection_observation_from_line(
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

pub(crate) fn info_runtime_inflight_from_line(line: &str) -> Option<(String, usize)> {
    if !line.contains("profile_inflight ") {
        return None;
    }
    let fields = info_runtime_parse_fields(line);
    Some((
        fields.get("profile")?.clone(),
        fields.get("count")?.parse::<usize>().ok()?,
    ))
}

pub(crate) fn runtime_log_timestamp_epoch(line: &str) -> Option<i64> {
    let timestamp = info_runtime_line_timestamp(line)?;
    chrono::DateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S%.f %:z")
        .or_else(|_| chrono::DateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S %:z"))
        .ok()
        .map(|datetime| datetime.timestamp())
}

pub(crate) fn info_runtime_line_timestamp(line: &str) -> Option<String> {
    let end = line.find("] ")?;
    line.strip_prefix('[')
        .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
        .map(ToString::to_string)
}

pub(crate) fn info_runtime_parse_fields(line: &str) -> BTreeMap<String, String> {
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

pub(crate) fn estimate_info_runway(
    observations: &[InfoRuntimeQuotaObservation],
    window: InfoQuotaWindow,
    current_remaining: i64,
    now: i64,
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
            info_profile_window_burn_rate(profile_observations, window)
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

pub(crate) fn info_profile_window_burn_rate(
    observations: &[&InfoRuntimeQuotaObservation],
    window: InfoQuotaWindow,
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
    if burned <= 0 || span_seconds < INFO_FORECAST_MIN_SPAN_SECONDS {
        return None;
    }

    Some((
        burned as f64 * 3600.0 / span_seconds as f64,
        earliest.timestamp,
        latest.timestamp,
    ))
}

pub(crate) fn info_observation_window_remaining(
    observation: &InfoRuntimeQuotaObservation,
    window: InfoQuotaWindow,
) -> i64 {
    match window {
        InfoQuotaWindow::FiveHour => observation.five_hour_remaining,
        InfoQuotaWindow::Weekly => observation.weekly_remaining,
    }
}

pub(crate) fn format_info_process_summary(processes: &[ProdexProcessInfo]) -> String {
    if processes.is_empty() {
        return "No".to_string();
    }

    let runtime_count = processes.iter().filter(|process| process.runtime).count();
    let pid_list = processes
        .iter()
        .take(6)
        .map(|process| process.pid.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = processes.len().saturating_sub(6);
    let extra = if remaining > 0 {
        format!(" (+{remaining} more)")
    } else {
        String::new()
    };

    format!(
        "Yes ({} total, {} runtime; pids: {}{})",
        processes.len(),
        runtime_count,
        pid_list,
        extra
    )
}

pub(crate) fn format_info_load_summary(
    summary: &InfoRuntimeLoadSummary,
    runtime_process_count: usize,
) -> String {
    if runtime_process_count == 0 {
        return "No active prodex runtime detected".to_string();
    }
    if summary.log_count == 0 {
        return "Runtime process detected, but no matching runtime log was found".to_string();
    }
    if summary.recent_selection_events == 0 {
        return format!(
            "{} active runtime log(s); no selection activity observed in the sampled window; inflight units {}",
            summary.log_count, summary.active_inflight_units
        );
    }
    if summary.recent_selection_events == 1 {
        return format!(
            "1 selection event observed in the sampled window; inflight units {}; {} active runtime log(s)",
            summary.active_inflight_units, summary.log_count
        );
    }

    let activity_span = summary
        .recent_first_timestamp
        .zip(summary.recent_last_timestamp)
        .map(|(start, end)| format_relative_duration(end.saturating_sub(start)))
        .unwrap_or_else(|| format!("{}m", INFO_RECENT_LOAD_WINDOW_SECONDS / 60));

    format!(
        "{} selection event(s) over {}; inflight units {}; {} active runtime log(s)",
        summary.recent_selection_events,
        activity_span,
        summary.active_inflight_units,
        summary.log_count
    )
}

pub(crate) fn format_info_quota_data_summary(aggregate: &InfoQuotaAggregate) -> String {
    if aggregate.quota_compatible_profiles == 0 {
        return "No quota-compatible profiles".to_string();
    }

    format!(
        "{} quota-compatible profile(s): live={}, snapshot={}, unavailable={}",
        aggregate.quota_compatible_profiles,
        aggregate.live_profiles,
        aggregate.snapshot_profiles,
        aggregate.unavailable_profiles
    )
}

pub(crate) fn format_runtime_policy_summary(summary: Option<&RuntimePolicySummary>) -> String {
    summary
        .map(|summary| format!("{} (v{})", summary.path.display(), summary.version))
        .unwrap_or_else(|| "disabled".to_string())
}

pub(crate) fn format_runtime_logs_summary() -> String {
    format!(
        "{} ({})",
        runtime_proxy_log_dir().display(),
        runtime_proxy_log_format().as_str()
    )
}

pub(crate) fn runtime_policy_json_value(
    summary: Option<&RuntimePolicySummary>,
) -> serde_json::Value {
    summary
        .map(|summary| {
            serde_json::json!({
                "path": summary.path.display().to_string(),
                "version": summary.version,
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

pub(crate) fn runtime_logs_json_value() -> serde_json::Value {
    serde_json::json!({
        "directory": runtime_proxy_log_dir().display().to_string(),
        "format": runtime_proxy_log_format().as_str(),
    })
}

pub(crate) fn format_runtime_tuning_workers(snapshot: &RuntimeTuningSnapshot) -> String {
    format!(
        "workers proxy={}, long-lived={}, async={}, probe-refresh={}; active={}, queue={}; lanes responses={}, compact={}, websocket={}, standard={}; ws-connect workers={}, queue={}, overflow={}; ws-dns workers={}, queue={}, overflow={}",
        snapshot.worker_count,
        snapshot.long_lived_worker_count,
        snapshot.async_worker_count,
        snapshot.probe_refresh_worker_count,
        snapshot.active_request_limit,
        snapshot.long_lived_queue_capacity,
        snapshot.lane_limits.responses,
        snapshot.lane_limits.compact,
        snapshot.lane_limits.websocket,
        snapshot.lane_limits.standard,
        snapshot.websocket_connect_worker_count,
        snapshot.websocket_connect_queue_capacity,
        snapshot.websocket_connect_overflow_capacity,
        snapshot.websocket_dns_worker_count,
        snapshot.websocket_dns_queue_capacity,
        snapshot.websocket_dns_overflow_capacity
    )
}

pub(crate) fn format_runtime_tuning_budgets(snapshot: &RuntimeTuningSnapshot) -> String {
    format!(
        "precommit={}x/{}ms, pressure-precommit={}x/{}ms, continuation={}x/{}ms; admission={}ms, pressure-admission={}ms, long-lived={}ms, pressure-long-lived={}ms",
        snapshot.precommit_attempt_limit,
        snapshot.precommit_budget_ms,
        snapshot.pressure_precommit_attempt_limit,
        snapshot.pressure_precommit_budget_ms,
        snapshot.continuation_precommit_attempt_limit,
        snapshot.continuation_precommit_budget_ms,
        snapshot.admission_wait_budget_ms,
        snapshot.pressure_admission_wait_budget_ms,
        snapshot.long_lived_queue_wait_budget_ms,
        snapshot.pressure_long_lived_queue_wait_budget_ms
    )
}

pub(crate) fn format_runtime_tuning_transport(snapshot: &RuntimeTuningSnapshot) -> String {
    format!(
        "http-connect={}ms, stream-idle={}ms, sse-lookahead={}ms; ws-connect={}ms, ws-progress={}ms, ws-happy={}ms, ws-stale-reuse={}ms; inflight soft/hard={}/{}",
        snapshot.http_connect_timeout_ms,
        snapshot.stream_idle_timeout_ms,
        snapshot.sse_lookahead_timeout_ms,
        snapshot.websocket_connect_timeout_ms,
        snapshot.websocket_precommit_progress_timeout_ms,
        snapshot.websocket_happy_eyeballs_delay_ms,
        snapshot.websocket_previous_response_reuse_stale_ms,
        snapshot.profile_inflight_soft_limit,
        snapshot.profile_inflight_hard_limit
    )
}

pub(crate) fn configured_secret_backend_selection() -> Result<secret_store::SecretBackendSelection>
{
    let policy = runtime_policy_secrets();
    let backend = env::var(PRODEX_SECRET_BACKEND_ENV)
        .ok()
        .map(|value| value.parse::<secret_store::SecretBackendKind>())
        .transpose()
        .map_err(anyhow::Error::new)?
        .or_else(|| policy.as_ref().and_then(|policy| policy.backend))
        .unwrap_or(secret_store::SecretBackendKind::File);
    let keyring_service = env::var(PRODEX_SECRET_KEYRING_SERVICE_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            policy
                .as_ref()
                .and_then(|policy| policy.keyring_service.clone())
        });
    secret_store::SecretBackendSelection::from_kind(backend, keyring_service)
        .map_err(anyhow::Error::new)
}

pub(crate) fn format_secret_backend_summary() -> String {
    match configured_secret_backend_selection() {
        Ok(selection) => match selection.keyring_service() {
            Some(service) => format!("{} ({service})", selection.kind()),
            None => selection.kind().to_string(),
        },
        Err(err) => format!("invalid ({err})"),
    }
}

pub(crate) fn secret_backend_json_value() -> serde_json::Value {
    match configured_secret_backend_selection() {
        Ok(selection) => serde_json::json!({
            "backend": selection.kind().as_str(),
            "keyring_service": selection.keyring_service(),
        }),
        Err(err) => serde_json::json!({
            "invalid": true,
            "error": err.to_string(),
        }),
    }
}

pub(crate) fn format_info_pool_remaining(
    total_remaining: i64,
    profiles_with_data: usize,
    earliest_reset_at: Option<i64>,
) -> String {
    if profiles_with_data == 0 {
        return "Unavailable".to_string();
    }

    let mut value = format!("{total_remaining}% across {profiles_with_data} profile(s)");
    if let Some(reset_at) = earliest_reset_at {
        value.push_str(&format!(
            "; earliest reset {}",
            format_precise_reset_time(Some(reset_at))
        ));
    }
    value
}

pub(crate) fn format_info_runway(
    profiles_with_data: usize,
    current_remaining: i64,
    earliest_reset_at: Option<i64>,
    estimate: Option<&InfoRunwayEstimate>,
    now: i64,
) -> String {
    if profiles_with_data == 0 {
        return "Unavailable".to_string();
    }
    if current_remaining <= 0 {
        return "Exhausted".to_string();
    }

    let Some(estimate) = estimate else {
        return "Unavailable (no recent quota decay observed in active runtime logs)".to_string();
    };

    let observed = format_relative_duration(estimate.observed_span_seconds);
    let burn = format!("{:.1}", estimate.burn_per_hour);
    if let Some(reset_at) = earliest_reset_at
        && reset_at <= estimate.exhaust_at
    {
        return format!(
            "Earliest reset {} arrives before the no-reset runway (~{} at {} aggregated-%/h, {} profile(s), observed over {})",
            format_precise_reset_time(Some(reset_at)),
            format_relative_duration(estimate.exhaust_at.saturating_sub(now)),
            burn,
            estimate.observed_profiles,
            observed
        );
    }

    format!(
        "{} (~{}) at {} aggregated-%/h from {} profile(s), observed over {}, no-reset estimate",
        format_precise_reset_time(Some(estimate.exhaust_at)),
        format_relative_duration(estimate.exhaust_at.saturating_sub(now)),
        burn,
        estimate.observed_profiles,
        observed
    )
}

pub(crate) fn format_relative_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds == 0 {
        return "now".to_string();
    }

    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        "<1m".to_string()
    }
}

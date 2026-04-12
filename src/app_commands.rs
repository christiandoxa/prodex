use super::*;

pub(super) fn handle_info(_args: InfoArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let policy_summary = runtime_policy_summary()?;
    let runtime_metrics_targets = collect_runtime_broker_metrics_targets(&paths);
    let now = Local::now().timestamp();
    let version_summary = format_info_prodex_version(&paths)?;
    let quota = collect_info_quota_aggregate(&paths, &state, now);
    let processes = collect_prodex_processes();
    let runtime_logs = collect_active_runtime_log_paths(&processes);
    let runtime_load = collect_info_runtime_load_summary(&runtime_logs, now);
    let runtime_process_count = processes.iter().filter(|process| process.runtime).count();
    let five_hour_runway = estimate_info_runway(
        &runtime_load.observations,
        InfoQuotaWindow::FiveHour,
        quota.five_hour_pool_remaining,
        now,
    );
    let weekly_runway = estimate_info_runway(
        &runtime_load.observations,
        InfoQuotaWindow::Weekly,
        quota.weekly_pool_remaining,
        now,
    );

    let fields = vec![
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
        (
            "Runtime policy".to_string(),
            format_runtime_policy_summary(policy_summary.as_ref()),
        ),
        (
            "Secret backend".to_string(),
            format_secret_backend_summary(),
        ),
        ("Runtime logs".to_string(), format_runtime_logs_summary()),
        ("Audit logs".to_string(), format_audit_logs_summary()),
        (
            "Runtime metrics".to_string(),
            format_runtime_broker_metrics_targets(&runtime_metrics_targets),
        ),
        ("Prodex version".to_string(), version_summary),
        (
            "Prodex processes".to_string(),
            format_info_process_summary(&processes),
        ),
        (
            "Recent load".to_string(),
            format_info_load_summary(&runtime_load, runtime_process_count),
        ),
        (
            "Quota data".to_string(),
            format_info_quota_data_summary(&quota),
        ),
        (
            "5h remaining pool".to_string(),
            format_info_pool_remaining(
                quota.five_hour_pool_remaining,
                quota.profiles_with_data(),
                quota.earliest_five_hour_reset_at,
            ),
        ),
        (
            "Weekly remaining pool".to_string(),
            format_info_pool_remaining(
                quota.weekly_pool_remaining,
                quota.profiles_with_data(),
                quota.earliest_weekly_reset_at,
            ),
        ),
        (
            "5h runway".to_string(),
            format_info_runway(
                quota.profiles_with_data(),
                quota.five_hour_pool_remaining,
                quota.earliest_five_hour_reset_at,
                five_hour_runway.as_ref(),
                now,
            ),
        ),
        (
            "Weekly runway".to_string(),
            format_info_runway(
                quota.profiles_with_data(),
                quota.weekly_pool_remaining,
                quota.earliest_weekly_reset_at,
                weekly_runway.as_ref(),
                now,
            ),
        ),
    ];
    print_panel("Info", &fields);
    Ok(())
}

pub(super) fn collect_info_quota_aggregate(
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
        collect_run_profile_reports(state, state.profiles.keys().cloned().collect(), None);
    build_info_quota_aggregate(&reports, &persisted_usage_snapshots, now)
}

pub(super) fn build_info_quota_aggregate(
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

pub(super) fn info_main_window_snapshots(
    usage: &UsageResponse,
) -> Option<(MainWindowSnapshot, MainWindowSnapshot)> {
    Some((
        required_main_window_snapshot(usage, "5h")?,
        required_main_window_snapshot(usage, "weekly")?,
    ))
}

pub(super) fn collect_prodex_processes() -> Vec<ProdexProcessInfo> {
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

pub(super) fn collect_process_rows() -> Vec<ProcessRow> {
    collect_process_rows_from_proc()
        .or_else(|| collect_process_rows_from_ps().ok())
        .unwrap_or_default()
}

pub(super) fn collect_process_rows_from_proc() -> Option<Vec<ProcessRow>> {
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

pub(super) fn collect_process_rows_from_ps() -> Result<Vec<ProcessRow>> {
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

pub(super) fn parse_ps_process_rows(text: &str) -> Vec<ProcessRow> {
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

pub(super) fn classify_prodex_process_row(
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

pub(super) fn is_prodex_process_row(row: &ProcessRow, current_basename: Option<&str>) -> bool {
    let command_base = process_basename(&row.command);
    process_basename_matches(command_base, current_basename)
        || prodex_process_row_argv_span(row, current_basename).is_some()
}

pub(super) fn prodex_process_row_is_runtime(
    row: &ProcessRow,
    current_basename: Option<&str>,
) -> bool {
    prodex_process_row_argv_span(row, current_basename)
        .and_then(|args| args.get(1))
        .is_some_and(|arg| arg == "run" || arg == "__runtime-broker")
}

pub(super) fn prodex_process_row_argv_span<'a>(
    row: &'a ProcessRow,
    current_basename: Option<&str>,
) -> Option<&'a [String]> {
    row.args.iter().enumerate().find_map(|(index, arg)| {
        process_basename_matches(process_basename(arg), current_basename)
            .then_some(&row.args[index..])
    })
}

pub(super) fn process_basename_matches(candidate: &str, current_basename: Option<&str>) -> bool {
    candidate == "prodex" || current_basename.is_some_and(|name| candidate == name)
}

pub(super) fn process_basename(input: &str) -> &str {
    Path::new(input)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(input)
}

pub(super) fn collect_active_runtime_log_paths(processes: &[ProdexProcessInfo]) -> Vec<PathBuf> {
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

pub(super) fn runtime_log_pid_from_path(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix(&format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-"))?;
    let (pid, _) = rest.split_once('-')?;
    pid.parse::<u32>().ok()
}

pub(super) fn collect_info_runtime_load_summary(
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

pub(super) fn collect_info_runtime_load_summary_from_text(
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

pub(super) fn info_runtime_selection_observation_from_line(
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

pub(super) fn info_runtime_inflight_from_line(line: &str) -> Option<(String, usize)> {
    if !line.contains("profile_inflight ") {
        return None;
    }
    let fields = info_runtime_parse_fields(line);
    Some((
        fields.get("profile")?.clone(),
        fields.get("count")?.parse::<usize>().ok()?,
    ))
}

pub(super) fn runtime_log_timestamp_epoch(line: &str) -> Option<i64> {
    let timestamp = info_runtime_line_timestamp(line)?;
    chrono::DateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S%.f %:z")
        .or_else(|_| chrono::DateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S %:z"))
        .ok()
        .map(|datetime| datetime.timestamp())
}

pub(super) fn info_runtime_line_timestamp(line: &str) -> Option<String> {
    let end = line.find("] ")?;
    line.strip_prefix('[')
        .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
        .map(ToString::to_string)
}

pub(super) fn info_runtime_parse_fields(line: &str) -> BTreeMap<String, String> {
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

pub(super) fn estimate_info_runway(
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

pub(super) fn info_profile_window_burn_rate(
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

pub(super) fn info_observation_window_remaining(
    observation: &InfoRuntimeQuotaObservation,
    window: InfoQuotaWindow,
) -> i64 {
    match window {
        InfoQuotaWindow::FiveHour => observation.five_hour_remaining,
        InfoQuotaWindow::Weekly => observation.weekly_remaining,
    }
}

pub(super) fn format_info_process_summary(processes: &[ProdexProcessInfo]) -> String {
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

pub(super) fn format_info_load_summary(
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

pub(super) fn format_info_quota_data_summary(aggregate: &InfoQuotaAggregate) -> String {
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

pub(super) fn format_runtime_policy_summary(summary: Option<&RuntimePolicySummary>) -> String {
    summary
        .map(|summary| format!("{} (v{})", summary.path.display(), summary.version))
        .unwrap_or_else(|| "disabled".to_string())
}

pub(super) fn format_runtime_logs_summary() -> String {
    format!(
        "{} ({})",
        runtime_proxy_log_dir().display(),
        runtime_proxy_log_format().as_str()
    )
}

pub(super) fn runtime_policy_json_value(
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

pub(super) fn runtime_logs_json_value() -> serde_json::Value {
    serde_json::json!({
        "directory": runtime_proxy_log_dir().display().to_string(),
        "format": runtime_proxy_log_format().as_str(),
    })
}

pub(super) fn configured_secret_backend_selection() -> Result<secret_store::SecretBackendSelection>
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

pub(super) fn format_secret_backend_summary() -> String {
    match configured_secret_backend_selection() {
        Ok(selection) => match selection.keyring_service() {
            Some(service) => format!("{} ({service})", selection.kind()),
            None => selection.kind().to_string(),
        },
        Err(err) => format!("invalid ({err})"),
    }
}

pub(super) fn secret_backend_json_value() -> serde_json::Value {
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

pub(super) fn audit_log_event_best_effort(
    component: &str,
    action: &str,
    outcome: &str,
    details: serde_json::Value,
) {
    let _ = append_audit_event(component, action, outcome, details);
}

pub(super) fn format_info_pool_remaining(
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

pub(super) fn format_info_runway(
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

pub(super) fn format_relative_duration(seconds: i64) -> String {
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
pub(super) fn handle_doctor(args: DoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let codex_home = default_codex_home(&paths)?;
    let policy_summary = runtime_policy_summary()?;
    let runtime_metrics_targets = collect_runtime_broker_metrics_targets(&paths);

    if args.runtime && args.json {
        let summary = collect_runtime_doctor_summary();
        let mut value = runtime_doctor_json_value(&summary);
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "runtime_policy".to_string(),
                runtime_policy_json_value(policy_summary.as_ref()),
            );
            object.insert("secret_backend".to_string(), secret_backend_json_value());
            object.insert("runtime_logs".to_string(), runtime_logs_json_value());
            object.insert("audit_logs".to_string(), audit_logs_json_value());
            object.insert(
                "live_brokers".to_string(),
                serde_json::to_value(collect_live_runtime_broker_observations(&paths))
                    .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
            );
            object.insert(
                "live_broker_metrics_targets".to_string(),
                serde_json::to_value(&runtime_metrics_targets)
                    .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
            );
        }
        let json = serde_json::to_string_pretty(&value)
            .context("failed to serialize runtime doctor summary")?;
        println!("{json}");
        return Ok(());
    }

    let summary_fields = vec![
        ("Prodex root".to_string(), paths.root.display().to_string()),
        (
            "State file".to_string(),
            format!(
                "{} ({})",
                paths.state_file.display(),
                if paths.state_file.exists() {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Profiles root".to_string(),
            paths.managed_profiles_root.display().to_string(),
        ),
        (
            "Default CODEX_HOME".to_string(),
            format!(
                "{} ({})",
                codex_home.display(),
                if codex_home.exists() {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Codex binary".to_string(),
            format_binary_resolution(&codex_bin()),
        ),
        (
            "Quota endpoint".to_string(),
            usage_url(&quota_base_url(None)),
        ),
        (
            "Runtime policy".to_string(),
            format_runtime_policy_summary(policy_summary.as_ref()),
        ),
        (
            "Secret backend".to_string(),
            format_secret_backend_summary(),
        ),
        ("Runtime logs".to_string(), format_runtime_logs_summary()),
        ("Audit logs".to_string(), format_audit_logs_summary()),
        (
            "Runtime metrics".to_string(),
            format_runtime_broker_metrics_targets(&runtime_metrics_targets),
        ),
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
    ];
    print_panel("Doctor", &summary_fields);

    if args.runtime {
        println!();
        print_panel("Runtime Proxy", &runtime_doctor_fields());
    }

    if state.profiles.is_empty() {
        return Ok(());
    }

    for report in collect_doctor_profile_reports(&state, args.quota) {
        let kind = if report.summary.managed {
            "managed"
        } else {
            "external"
        };

        println!();
        let mut fields = vec![
            (
                "Current".to_string(),
                if report.summary.active {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
            ("Kind".to_string(), kind.to_string()),
            ("Auth".to_string(), report.summary.auth.label),
            (
                "Email".to_string(),
                report.summary.email.as_deref().unwrap_or("-").to_string(),
            ),
            (
                "Path".to_string(),
                report.summary.codex_home.display().to_string(),
            ),
            (
                "Exists".to_string(),
                if report.summary.codex_home.exists() {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
        ];

        if let Some(quota) = report.quota {
            match quota {
                Ok(usage) => {
                    let blocked = collect_blocked_limits(&usage, false);
                    fields.push((
                        "Quota".to_string(),
                        if blocked.is_empty() {
                            "Ready".to_string()
                        } else {
                            format!("Blocked ({})", format_blocked_limits(&blocked))
                        },
                    ));
                    fields.push(("Main".to_string(), format_main_windows(&usage)));
                }
                Err(err) => {
                    fields.push((
                        "Quota".to_string(),
                        format!("Error ({})", first_line_of_error(&err.to_string())),
                    ));
                }
            }
        }
        print_panel(&format!("Profile {}", report.summary.name), &fields);
    }

    Ok(())
}

pub(super) fn handle_audit(args: AuditArgs) -> Result<()> {
    let query = AuditLogQuery {
        tail: args.tail,
        component: args.component.as_deref().map(str::trim).map(str::to_string),
        action: args.action.as_deref().map(str::trim).map(str::to_string),
        outcome: args.outcome.as_deref().map(str::trim).map(str::to_string),
    };
    let events = read_recent_audit_events(&query)?;

    if args.json {
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "audit_logs": audit_logs_json_value(),
            "filters": {
                "tail": query.tail,
                "component": query.component,
                "action": query.action,
                "outcome": query.outcome,
            },
            "events": events,
        }))
        .context("failed to serialize audit log output")?;
        println!("{json}");
        return Ok(());
    }

    println!(
        "{}",
        render_audit_events_human(&audit_log_path(), &query, &events)
    );

    Ok(())
}

pub(super) fn handle_cleanup() -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let runtime_log_dir = runtime_proxy_log_dir();
    let summary = perform_prodex_cleanup(&paths, &mut state)?;

    let fields = vec![
        ("Prodex root".to_string(), paths.root.display().to_string()),
        (
            "Duplicate profiles".to_string(),
            summary.duplicate_profiles_removed.to_string(),
        ),
        (
            "Duplicate managed homes".to_string(),
            summary.duplicate_managed_profile_homes_removed.to_string(),
        ),
        (
            "Runtime logs".to_string(),
            format!(
                "{} removed from {}",
                summary.runtime_logs_removed,
                runtime_log_dir.display()
            ),
        ),
        (
            "Runtime pointer".to_string(),
            if summary.stale_runtime_log_pointer_removed > 0 {
                "removed stale latest-pointer file".to_string()
            } else {
                "clean".to_string()
            },
        ),
        (
            "Temp login homes".to_string(),
            summary.stale_login_dirs_removed.to_string(),
        ),
        (
            "Orphan managed homes".to_string(),
            summary.orphan_managed_profile_dirs_removed.to_string(),
        ),
        (
            "Transient root files".to_string(),
            summary.transient_root_files_removed.to_string(),
        ),
        (
            "Stale root temp files".to_string(),
            summary.stale_root_temp_files_removed.to_string(),
        ),
        (
            "Dead broker leases".to_string(),
            summary.dead_runtime_broker_leases_removed.to_string(),
        ),
        (
            "Dead broker registries".to_string(),
            summary.dead_runtime_broker_registries_removed.to_string(),
        ),
        (
            "Total removed".to_string(),
            summary.total_removed().to_string(),
        ),
    ];
    print_panel("Cleanup", &fields);
    Ok(())
}

pub(super) fn deserialize_null_default<'de, D, T>(
    deserializer: D,
) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

pub(super) fn handle_quota(args: QuotaArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    if args.all {
        if state.profiles.is_empty() {
            bail!("no profiles configured");
        }
        if quota_watch_enabled(&args) {
            return watch_all_quotas(&paths, args.base_url.as_deref(), args.detail);
        }
        let reports = collect_quota_reports(&state, args.base_url.as_deref());
        print_quota_reports(&reports, args.detail);
        return Ok(());
    }

    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    if args.raw {
        let usage = fetch_usage_json(&codex_home, args.base_url.as_deref())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&usage).context("failed to render usage JSON")?
        );
        return Ok(());
    }

    if quota_watch_enabled(&args) {
        return watch_quota(&profile_name, &codex_home, args.base_url.as_deref());
    }

    let usage = fetch_usage(&codex_home, args.base_url.as_deref())?;
    println!("{}", render_profile_quota(&profile_name, &usage));
    Ok(())
}

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
    let codex_args = normalize_run_codex_args(&args.codex_args);
    let allow_auto_rotate = !args.no_auto_rotate;
    let include_code_review = is_review_invocation(&codex_args);
    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: args.profile.as_deref(),
        allow_auto_rotate,
        skip_quota_check: args.skip_quota_check,
        base_url: args.base_url.as_deref(),
        include_code_review,
        force_runtime_proxy: false,
    })?;
    let runtime_proxy = prepared.runtime_proxy;
    let runtime_args = runtime_proxy
        .as_ref()
        .map(|proxy| {
            if proxy.openai_mount_path == RUNTIME_PROXY_OPENAI_MOUNT_PATH {
                runtime_proxy_codex_args(proxy.listen_addr, &codex_args)
            } else {
                runtime_proxy_codex_args_with_mount_path(
                    proxy.listen_addr,
                    &proxy.openai_mount_path,
                    &codex_args,
                )
            }
        })
        .unwrap_or(codex_args);

    let status = run_child(
        &codex_bin(),
        &runtime_args,
        &prepared.codex_home,
        &[],
        &[],
        runtime_proxy.as_ref(),
    )?;
    // `std::process::exit` does not run destructors, so release the broker lease
    // before mirroring Codex's exit status back to the caller.
    drop(runtime_proxy);
    exit_with_status(status)
}

pub(super) fn prepare_runtime_launch(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let profile_name = resolve_profile_name(&state, request.profile)?;
    let mut selected_profile_name = profile_name.clone();
    let explicit_profile_requested = request.profile.is_some();
    let allow_auto_rotate = request.allow_auto_rotate;
    let include_code_review = request.include_code_review;
    let mut codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .codex_home
        .clone();

    if !request.skip_quota_check {
        if allow_auto_rotate && !explicit_profile_requested && state.profiles.len() > 1 {
            let current_report = probe_run_profile(&state, &profile_name, 0, request.base_url)?;
            if !run_profile_probe_is_ready(&current_report, include_code_review) {
                let persisted_usage_snapshots =
                    load_runtime_usage_snapshots(&paths, &state.profiles).unwrap_or_default();
                let reports = run_preflight_reports_with_current_first(
                    &state,
                    &profile_name,
                    current_report,
                    request.base_url,
                );
                let ready_candidates = ready_profile_candidates(
                    &reports,
                    include_code_review,
                    Some(&profile_name),
                    &state,
                    Some(&persisted_usage_snapshots),
                );
                let selected_report = reports.iter().find(|report| report.name == profile_name);

                if let Some(best_candidate) = ready_candidates.first() {
                    if best_candidate.name != profile_name {
                        print_wrapped_stderr(&section_header("Quota Preflight"));
                        let mut selection_message = format!(
                            "Using profile '{}' ({})",
                            best_candidate.name,
                            format_main_windows_compact(&best_candidate.usage)
                        );
                        if let Some(report) = selected_report {
                            match &report.result {
                                Ok(usage) => {
                                    let blocked =
                                        collect_blocked_limits(usage, include_code_review);
                                    if !blocked.is_empty() {
                                        print_wrapped_stderr(&format!(
                                            "Quota preflight blocked profile '{}': {}",
                                            profile_name,
                                            format_blocked_limits(&blocked)
                                        ));
                                        selection_message = format!(
                                            "Auto-rotating to profile '{}' using quota-pressure scoring ({}).",
                                            best_candidate.name,
                                            format_main_windows_compact(&best_candidate.usage)
                                        );
                                    } else {
                                        selection_message = format!(
                                            "Auto-selecting profile '{}' over active profile '{}' using quota-pressure scoring ({}).",
                                            best_candidate.name,
                                            profile_name,
                                            format_main_windows_compact(&best_candidate.usage)
                                        );
                                    }
                                }
                                Err(err) => {
                                    print_wrapped_stderr(&format!(
                                        "Warning: quota preflight failed for '{}': {err}",
                                        profile_name
                                    ));
                                    selection_message = format!(
                                        "Using ready profile '{}' after quota preflight failed ({})",
                                        best_candidate.name,
                                        format_main_windows_compact(&best_candidate.usage)
                                    );
                                }
                            }
                        }

                        codex_home = state
                            .profiles
                            .get(&best_candidate.name)
                            .with_context(|| {
                                format!("profile '{}' is missing", best_candidate.name)
                            })?
                            .codex_home
                            .clone();
                        selected_profile_name = best_candidate.name.clone();
                        state.active_profile = Some(best_candidate.name.clone());
                        state.save(&paths)?;
                        print_wrapped_stderr(&selection_message);
                    }
                } else if let Some(report) = selected_report {
                    match &report.result {
                        Ok(usage) => {
                            let blocked = collect_blocked_limits(usage, include_code_review);
                            print_wrapped_stderr(&section_header("Quota Preflight"));
                            print_wrapped_stderr(&format!(
                                "Quota preflight blocked profile '{}': {}",
                                profile_name,
                                format_blocked_limits(&blocked)
                            ));
                            print_wrapped_stderr("No ready profile was found.");
                            print_wrapped_stderr(&format!(
                                "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
                                profile_name
                            ));
                            std::process::exit(2);
                        }
                        Err(err) => {
                            print_wrapped_stderr(&section_header("Quota Preflight"));
                            print_wrapped_stderr(&format!(
                                "Warning: quota preflight failed for '{}': {err:#}",
                                profile_name
                            ));
                            print_wrapped_stderr("Continuing without quota gate.");
                        }
                    }
                }
            }
        } else {
            match fetch_usage(&codex_home, request.base_url) {
                Ok(usage) => {
                    let blocked = collect_blocked_limits(&usage, include_code_review);
                    if !blocked.is_empty() {
                        let alternatives = find_ready_profiles(
                            &state,
                            &profile_name,
                            request.base_url,
                            include_code_review,
                        );

                        print_wrapped_stderr(&section_header("Quota Preflight"));
                        print_wrapped_stderr(&format!(
                            "Quota preflight blocked profile '{}': {}",
                            profile_name,
                            format_blocked_limits(&blocked)
                        ));

                        if allow_auto_rotate {
                            if let Some(next_profile) = alternatives.first() {
                                let next_profile = next_profile.clone();
                                codex_home = state
                                    .profiles
                                    .get(&next_profile)
                                    .with_context(|| {
                                        format!("profile '{}' is missing", next_profile)
                                    })?
                                    .codex_home
                                    .clone();
                                selected_profile_name = next_profile.clone();
                                state.active_profile = Some(next_profile.clone());
                                state.save(&paths)?;
                                print_wrapped_stderr(&format!(
                                    "Auto-rotating to profile '{}'.",
                                    next_profile
                                ));
                            } else {
                                print_wrapped_stderr("No other ready profile was found.");
                                print_wrapped_stderr(&format!(
                                    "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
                                    profile_name
                                ));
                                std::process::exit(2);
                            }
                        } else {
                            if !alternatives.is_empty() {
                                print_wrapped_stderr(&format!(
                                    "Other profiles that look ready: {}",
                                    alternatives.join(", ")
                                ));
                                print_wrapped_stderr(
                                    "Rerun without `--no-auto-rotate` to allow fallback.",
                                );
                            }
                            print_wrapped_stderr(&format!(
                                "Inspect with `prodex quota --profile {}` or bypass with `prodex run --skip-quota-check`.",
                                profile_name
                            ));
                            std::process::exit(2);
                        }
                    }
                }
                Err(err) => {
                    print_wrapped_stderr(&section_header("Quota Preflight"));
                    print_wrapped_stderr(&format!(
                        "Warning: quota preflight failed for '{}': {err:#}",
                        profile_name
                    ));
                    print_wrapped_stderr("Continuing without quota gate.");
                }
            }
        }
    }

    record_run_selection(&mut state, &selected_profile_name);
    state.save(&paths)?;

    let managed = state
        .profiles
        .get(&selected_profile_name)
        .with_context(|| format!("profile '{}' is missing", selected_profile_name))?
        .managed;
    if managed {
        prepare_managed_codex_home(&paths, &codex_home)?;
    }

    let runtime_upstream_base_url = quota_base_url(request.base_url);
    let runtime_proxy = if request.force_runtime_proxy
        || should_enable_runtime_rotation_proxy(&state, &selected_profile_name, allow_auto_rotate)
    {
        Some(ensure_runtime_rotation_proxy_endpoint(
            &paths,
            &selected_profile_name,
            runtime_upstream_base_url.as_str(),
            include_code_review,
        )?)
    } else {
        None
    };

    Ok(PreparedRuntimeLaunch {
        paths,
        codex_home,
        managed,
        runtime_proxy,
    })
}

pub(super) fn handle_runtime_broker(args: RuntimeBrokerArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let proxy = start_runtime_rotation_proxy_with_listen_addr(
        &paths,
        &state,
        &args.current_profile,
        args.upstream_base_url.clone(),
        args.include_code_review,
        args.listen_addr.as_deref(),
    )?;
    if proxy.owner_lock.is_none() {
        return Ok(());
    }

    let metadata = RuntimeBrokerMetadata {
        broker_key: runtime_broker_key(&args.upstream_base_url, args.include_code_review),
        listen_addr: proxy.listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        current_profile: args.current_profile.clone(),
        include_code_review: args.include_code_review,
        instance_token: args.instance_token.clone(),
        admin_token: args.admin_token.clone(),
    };
    register_runtime_broker_metadata(&proxy.log_path, metadata.clone());
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        listen_addr: proxy.listen_addr.to_string(),
        started_at: metadata.started_at,
        upstream_base_url: args.upstream_base_url.clone(),
        include_code_review: args.include_code_review,
        current_profile: args.current_profile.clone(),
        instance_token: args.instance_token.clone(),
        admin_token: args.admin_token.clone(),
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };
    save_runtime_broker_registry(&paths, &args.broker_key, &registry)?;
    runtime_proxy_log_to_path(
        &proxy.log_path,
        &format!(
            "runtime_broker_started listen_addr={} broker_key={} current_profile={} include_code_review={}",
            proxy.listen_addr, args.broker_key, args.current_profile, args.include_code_review
        ),
    );
    audit_log_event_best_effort(
        "runtime_broker",
        "start",
        "success",
        serde_json::json!({
            "broker_key": args.broker_key,
            "listen_addr": proxy.listen_addr.to_string(),
            "current_profile": args.current_profile,
            "include_code_review": args.include_code_review,
            "upstream_base_url": args.upstream_base_url,
        }),
    );

    let startup_grace_until = metadata
        .started_at
        .saturating_add(runtime_broker_startup_grace_seconds());
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    let lease_scan_interval = Duration::from_millis(
        RUNTIME_BROKER_LEASE_SCAN_INTERVAL_MS.max(RUNTIME_BROKER_POLL_INTERVAL_MS),
    );
    let mut idle_started_at = None::<i64>;
    let mut cached_live_leases = 0usize;
    let mut last_lease_scan_at = Instant::now() - lease_scan_interval;
    loop {
        let active_requests = proxy.active_request_count.load(Ordering::SeqCst);
        if active_requests == 0 && last_lease_scan_at.elapsed() >= lease_scan_interval {
            cached_live_leases = cleanup_runtime_broker_stale_leases(&paths, &args.broker_key);
            last_lease_scan_at = Instant::now();
        }
        if cached_live_leases > 0 || active_requests > 0 {
            idle_started_at = None;
        } else {
            let now = Local::now().timestamp();
            if now < startup_grace_until {
                idle_started_at = None;
                thread::sleep(poll_interval);
                continue;
            }
            let idle_since = idle_started_at.get_or_insert(now);
            if now.saturating_sub(*idle_since) >= RUNTIME_BROKER_IDLE_GRACE_SECONDS {
                runtime_proxy_log_to_path(
                    &proxy.log_path,
                    &format!(
                        "runtime_broker_idle_shutdown broker_key={} idle_seconds={}",
                        args.broker_key,
                        now.saturating_sub(*idle_since)
                    ),
                );
                break;
            }
        }
        thread::sleep(poll_interval);
    }

    drop(proxy);
    remove_runtime_broker_registry_if_token_matches(&paths, &args.broker_key, &args.instance_token);
    Ok(())
}

pub(super) fn run_child(
    binary: &OsString,
    args: &[OsString],
    codex_home: &Path,
    extra_env: &[(&str, OsString)],
    removed_env: &[&str],
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<ExitStatus> {
    let mut command = Command::new(binary);
    command.args(args).env("CODEX_HOME", codex_home);
    for key in removed_env {
        command.env_remove(key);
    }
    for (key, value) in extra_env {
        command.env(key, value);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to execute {}", binary.to_string_lossy()))?;
    let _child_runtime_broker_lease = match runtime_proxy {
        Some(proxy) => match proxy.create_child_lease(child.id()) {
            Ok(lease) => Some(lease),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
            }
        },
        None => None,
    };
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for {}", binary.to_string_lossy()))?;
    Ok(status)
}

pub(super) fn exit_with_status(status: ExitStatus) -> Result<()> {
    std::process::exit(status.code().unwrap_or(1));
}

pub(super) fn collect_run_profile_reports(
    state: &AppState,
    profile_names: Vec<String>,
    base_url: Option<&str>,
) -> Vec<RunProfileProbeReport> {
    let jobs = profile_names
        .into_iter()
        .enumerate()
        .filter_map(|(order_index, name)| {
            let profile = state.profiles.get(&name)?;
            Some(RunProfileProbeJob {
                name,
                order_index,
                codex_home: profile.codex_home.clone(),
            })
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    map_parallel(jobs, |job| {
        let auth = read_auth_summary(&job.codex_home);
        let result = if auth.quota_compatible {
            fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string())
        } else {
            Err("auth mode is not quota-compatible".to_string())
        };

        RunProfileProbeReport {
            name: job.name,
            order_index: job.order_index,
            auth,
            result,
        }
    })
}

pub(super) fn probe_run_profile(
    state: &AppState,
    profile_name: &str,
    order_index: usize,
    base_url: Option<&str>,
) -> Result<RunProfileProbeReport> {
    let profile = state
        .profiles
        .get(profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let auth = read_auth_summary(&profile.codex_home);
    let result = if auth.quota_compatible {
        fetch_usage(&profile.codex_home, base_url).map_err(|err| err.to_string())
    } else {
        Err("auth mode is not quota-compatible".to_string())
    };

    Ok(RunProfileProbeReport {
        name: profile_name.to_string(),
        order_index,
        auth,
        result,
    })
}

pub(super) fn run_profile_probe_is_ready(
    report: &RunProfileProbeReport,
    include_code_review: bool,
) -> bool {
    match report.result.as_ref() {
        Ok(usage) => collect_blocked_limits(usage, include_code_review).is_empty(),
        Err(_) => false,
    }
}

pub(super) fn run_preflight_reports_with_current_first(
    state: &AppState,
    current_profile: &str,
    current_report: RunProfileProbeReport,
    base_url: Option<&str>,
) -> Vec<RunProfileProbeReport> {
    let mut reports = Vec::with_capacity(state.profiles.len());
    reports.push(current_report);
    reports.extend(
        collect_run_profile_reports(
            state,
            profile_rotation_order(state, current_profile),
            base_url,
        )
        .into_iter()
        .map(|mut report| {
            report.order_index += 1;
            report
        }),
    );
    reports
}

pub(super) fn ready_profile_candidates(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    preferred_profile: Option<&str>,
    state: &AppState,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
) -> Vec<ReadyProfileCandidate> {
    let candidates = reports
        .iter()
        .filter_map(|report| {
            if !report.auth.quota_compatible {
                return None;
            }

            let (usage, quota_source) = match report.result.as_ref() {
                Ok(usage) => (usage.clone(), RuntimeQuotaSource::LiveProbe),
                Err(_) => {
                    let snapshot = persisted_usage_snapshots
                        .and_then(|snapshots| snapshots.get(&report.name))?;
                    let now = Local::now().timestamp();
                    if !runtime_usage_snapshot_is_usable(snapshot, now) {
                        return None;
                    }
                    (
                        usage_from_runtime_usage_snapshot(snapshot),
                        RuntimeQuotaSource::PersistedSnapshot,
                    )
                }
            };
            if !collect_blocked_limits(&usage, include_code_review).is_empty() {
                return None;
            }

            Some(ReadyProfileCandidate {
                name: report.name.clone(),
                usage,
                order_index: report.order_index,
                preferred: preferred_profile == Some(report.name.as_str()),
                quota_source,
            })
        })
        .collect::<Vec<_>>();

    schedule_ready_profile_candidates(candidates, state, preferred_profile)
}

pub(super) fn schedule_ready_profile_candidates(
    mut candidates: Vec<ReadyProfileCandidate>,
    state: &AppState,
    preferred_profile: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    if candidates.len() <= 1 {
        return candidates;
    }

    let now = Local::now().timestamp();
    let best_total_pressure = candidates
        .iter()
        .map(|candidate| ready_profile_score(candidate).total_pressure)
        .min()
        .unwrap_or(i64::MAX);

    candidates.sort_by_key(|candidate| {
        ready_profile_runtime_sort_key(candidate, state, best_total_pressure, now)
    });

    if let Some(preferred_name) = preferred_profile
        && let Some(preferred_index) = candidates.iter().position(|candidate| {
            candidate.name == preferred_name
                && !profile_in_run_selection_cooldown(state, &candidate.name, now)
        })
    {
        let preferred_score = ready_profile_score(&candidates[preferred_index]).total_pressure;
        let selected_score = ready_profile_score(&candidates[0]).total_pressure;

        if preferred_index > 0
            && score_within_bps(
                preferred_score,
                selected_score,
                RUN_SELECTION_HYSTERESIS_BPS,
            )
        {
            let preferred_candidate = candidates.remove(preferred_index);
            candidates.insert(0, preferred_candidate);
        }
    }

    candidates
}

pub(super) type ReadyProfileSortKey = (
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
    usize,
    usize,
    usize,
);

pub(super) type ReadyProfileRuntimeSortKey = (usize, usize, i64, ReadyProfileSortKey);

pub(super) fn ready_profile_runtime_sort_key(
    candidate: &ReadyProfileCandidate,
    state: &AppState,
    best_total_pressure: i64,
    now: i64,
) -> ReadyProfileRuntimeSortKey {
    let score = ready_profile_score(candidate);
    let near_optimal = score_within_bps(
        score.total_pressure,
        best_total_pressure,
        RUN_SELECTION_NEAR_OPTIMAL_BPS,
    );
    let recently_used =
        near_optimal && profile_in_run_selection_cooldown(state, &candidate.name, now);
    let last_selected_at = if near_optimal {
        state
            .last_run_selected_at
            .get(&candidate.name)
            .copied()
            .unwrap_or(i64::MIN)
    } else {
        i64::MIN
    };

    (
        if near_optimal { 0usize } else { 1usize },
        if recently_used { 1usize } else { 0usize },
        last_selected_at,
        ready_profile_sort_key(candidate),
    )
}

pub(super) fn ready_profile_sort_key(candidate: &ReadyProfileCandidate) -> ReadyProfileSortKey {
    let score = ready_profile_score(candidate);

    (
        score.total_pressure,
        score.weekly_pressure,
        score.five_hour_pressure,
        Reverse(score.reserve_floor),
        Reverse(score.weekly_remaining),
        Reverse(score.five_hour_remaining),
        score.weekly_reset_at,
        score.five_hour_reset_at,
        runtime_quota_source_sort_key(RuntimeRouteKind::Responses, candidate.quota_source),
        if candidate.preferred { 0usize } else { 1usize },
        candidate.order_index,
    )
}

pub(super) fn ready_profile_score(candidate: &ReadyProfileCandidate) -> ReadyProfileScore {
    ready_profile_score_for_route(&candidate.usage, RuntimeRouteKind::Responses)
}

pub(super) fn ready_profile_score_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> ReadyProfileScore {
    let weekly = required_main_window_snapshot(usage, "weekly");
    let five_hour = required_main_window_snapshot(usage, "5h");

    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);
    let weekly_weight = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => 10,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 8,
    };
    let reserve_bias = match runtime_quota_pressure_band_for_route(usage, route_kind) {
        RuntimeQuotaPressureBand::Healthy => 0,
        RuntimeQuotaPressureBand::Thin => 250_000,
        RuntimeQuotaPressureBand::Critical => 1_000_000,
        RuntimeQuotaPressureBand::Exhausted | RuntimeQuotaPressureBand::Unknown => i64::MAX / 4,
    };

    ReadyProfileScore {
        total_pressure: reserve_bias
            .saturating_add(weekly_pressure.saturating_mul(weekly_weight))
            .saturating_add(five_hour_pressure),
        weekly_pressure,
        five_hour_pressure,
        reserve_floor: weekly_remaining.min(five_hour_remaining),
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at: weekly.map_or(i64::MAX, |window| window.reset_at),
        five_hour_reset_at: five_hour.map_or(i64::MAX, |window| window.reset_at),
    }
}

pub(super) fn profile_in_run_selection_cooldown(
    state: &AppState,
    profile_name: &str,
    now: i64,
) -> bool {
    let Some(last_selected_at) = state.last_run_selected_at.get(profile_name).copied() else {
        return false;
    };

    now.saturating_sub(last_selected_at) < RUN_SELECTION_COOLDOWN_SECONDS
}

pub(super) fn score_within_bps(candidate_score: i64, best_score: i64, bps: i64) -> bool {
    if candidate_score <= best_score {
        return true;
    }

    let lhs = i128::from(candidate_score).saturating_mul(10_000);
    let rhs = i128::from(best_score).saturating_mul(i128::from(10_000 + bps));
    lhs <= rhs
}

pub(super) fn required_main_window_snapshot(
    usage: &UsageResponse,
    label: &str,
) -> Option<MainWindowSnapshot> {
    let window = find_main_window(usage.rate_limit.as_ref()?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - Local::now().timestamp()).max(0)
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

pub(super) fn active_profile_selection_order(
    state: &AppState,
    current_profile: &str,
) -> Vec<String> {
    std::iter::once(current_profile.to_string())
        .chain(profile_rotation_order(state, current_profile))
        .collect()
}

pub(super) fn map_parallel<I, O, F>(inputs: Vec<I>, func: F) -> Vec<O>
where
    I: Send,
    O: Send,
    F: Fn(I) -> O + Sync,
{
    if inputs.len() <= 1 {
        return inputs.into_iter().map(func).collect();
    }

    thread::scope(|scope| {
        let func = &func;
        let mut handles = Vec::with_capacity(inputs.len());
        for input in inputs {
            handles.push(scope.spawn(move || func(input)));
        }

        handles
            .into_iter()
            .map(|handle| handle.join().expect("parallel worker panicked"))
            .collect()
    })
}
pub(super) fn find_ready_profiles(
    state: &AppState,
    current_profile: &str,
    base_url: Option<&str>,
    include_code_review: bool,
) -> Vec<String> {
    ready_profile_candidates(
        &collect_run_profile_reports(
            state,
            profile_rotation_order(state, current_profile),
            base_url,
        ),
        include_code_review,
        None,
        state,
        None,
    )
    .into_iter()
    .map(|candidate| candidate.name)
    .collect()
}

pub(super) fn profile_rotation_order(state: &AppState, current_profile: &str) -> Vec<String> {
    let names: Vec<String> = state.profiles.keys().cloned().collect();
    let Some(index) = names.iter().position(|name| name == current_profile) else {
        return names
            .into_iter()
            .filter(|name| name != current_profile)
            .collect();
    };

    names
        .iter()
        .skip(index + 1)
        .chain(names.iter().take(index))
        .cloned()
        .collect()
}

pub(super) fn is_review_invocation(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg == "review")
}

pub(super) fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path_for_compare(left) == normalize_path_for_compare(right)
}

pub(super) fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(super) fn absolutize(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    let current_dir = env::current_dir().context("failed to determine current directory")?;
    Ok(current_dir.join(path))
}

pub(super) fn legacy_default_codex_home() -> Result<PathBuf> {
    Ok(home_dir()
        .context("failed to determine home directory")?
        .join(DEFAULT_CODEX_DIR))
}

pub(super) fn prodex_default_shared_codex_root(root: &Path) -> PathBuf {
    root.join(DEFAULT_CODEX_DIR)
}

pub(super) fn resolve_shared_codex_root(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

pub(super) fn select_default_codex_home(
    shared_codex_root: &Path,
    legacy_codex_home: &Path,
    override_active: bool,
) -> PathBuf {
    if override_active || shared_codex_root.exists() || !legacy_codex_home.exists() {
        shared_codex_root.to_path_buf()
    } else {
        legacy_codex_home.to_path_buf()
    }
}

pub(super) fn default_codex_home(paths: &AppPaths) -> Result<PathBuf> {
    let legacy = legacy_default_codex_home()?;
    Ok(select_default_codex_home(
        &paths.shared_codex_root,
        &legacy,
        env::var_os("PRODEX_SHARED_CODEX_HOME").is_some(),
    ))
}

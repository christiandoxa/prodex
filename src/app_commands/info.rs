use super::*;

pub(crate) use prodex_app_reports::{
    InfoTokenUsageCounts, InfoTokenUsageSummary, collect_info_runtime_load_summary_from_text,
    collect_info_token_usage_summary_from_text,
};

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
    prodex_app_reports::build_info_quota_aggregate(
        reports,
        persisted_usage_snapshots,
        now,
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    )
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

pub(crate) fn collect_recent_runtime_log_paths(limit: usize) -> Vec<PathBuf> {
    let mut paths = prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir());
    paths.sort_by(|left, right| {
        runtime_log_modified(left)
            .cmp(&runtime_log_modified(right))
            .then_with(|| left.cmp(right))
    });
    paths.reverse();
    paths.truncate(limit);
    paths
}

fn runtime_log_modified(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(UNIX_EPOCH)
}

pub(crate) fn collect_info_token_usage_summary(log_paths: &[PathBuf]) -> InfoTokenUsageSummary {
    let mut summary = InfoTokenUsageSummary {
        log_count: log_paths.len(),
        ..InfoTokenUsageSummary::default()
    };
    for path in log_paths {
        let Ok(tail) = read_runtime_log_tail(path, INFO_RUNTIME_LOG_TAIL_BYTES) else {
            continue;
        };
        summary.merge(collect_info_token_usage_summary_from_text(
            &String::from_utf8_lossy(&tail),
        ));
    }
    summary
}

pub(crate) fn estimate_info_runway(
    observations: &[InfoRuntimeQuotaObservation],
    window: InfoQuotaWindow,
    current_remaining: i64,
    now: i64,
) -> Option<InfoRunwayEstimate> {
    prodex_app_reports::estimate_info_runway(
        observations,
        window,
        current_remaining,
        now,
        INFO_FORECAST_MIN_SPAN_SECONDS,
    )
}

pub(crate) fn format_info_process_summary(processes: &[ProdexProcessInfo]) -> String {
    let runtime_count = processes.iter().filter(|process| process.runtime).count();
    terminal_ui::format_info_process_summary_display(
        processes.len(),
        runtime_count,
        processes.iter().map(|process| process.pid),
        6,
    )
}

pub(crate) fn format_info_load_summary(
    summary: &InfoRuntimeLoadSummary,
    runtime_process_count: usize,
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
        INFO_RECENT_LOAD_WINDOW_SECONDS,
    )
}

pub(crate) fn format_info_quota_data_summary(aggregate: &InfoQuotaAggregate) -> String {
    terminal_ui::format_info_quota_data_summary_display(
        aggregate.quota_compatible_profiles,
        aggregate.live_profiles,
        aggregate.snapshot_profiles,
        aggregate.unavailable_profiles,
    )
}

pub(crate) fn format_info_token_usage_summary(summary: &InfoTokenUsageSummary) -> String {
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

pub(crate) fn format_runtime_policy_summary(summary: Option<&RuntimePolicySummary>) -> String {
    let Some(summary) = summary else {
        return terminal_ui::format_runtime_policy_summary_display(None, None);
    };

    let path = summary.path.display().to_string();
    terminal_ui::format_runtime_policy_summary_display(Some(path.as_str()), Some(summary.version))
}

pub(crate) fn format_runtime_logs_summary() -> String {
    let directory = runtime_proxy_log_dir().display().to_string();
    terminal_ui::format_runtime_logs_summary_display(
        &directory,
        runtime_proxy_log_format().as_str(),
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

pub(crate) fn format_runtime_tuning_budgets(snapshot: &RuntimeTuningSnapshot) -> String {
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

pub(crate) fn format_runtime_tuning_transport(snapshot: &RuntimeTuningSnapshot) -> String {
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
    let earliest_reset =
        earliest_reset_at.map(|reset_at| format_precise_reset_time(Some(reset_at)));
    terminal_ui::format_info_pool_remaining_display(
        total_remaining,
        profiles_with_data,
        earliest_reset.as_deref(),
    )
}

pub(crate) fn format_info_runway(
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
mod tests {
    use super::*;

    #[test]
    fn info_token_usage_summary_parses_text_runtime_log_markers() {
        let summary = collect_info_token_usage_summary_from_text(concat!(
            "[2026-04-29 10:00:00.000 +07:00] token_usage request=1 transport=http profile=main source=responses_unary input_tokens=100 cached_input_tokens=25 output_tokens=40 reasoning_tokens=8\n",
            "[2026-04-29 10:00:01.000 +07:00] token_usage request=2 transport=http profile=backup source=responses_sse input_tokens=10 cached_input_tokens=0 output_tokens=4 reasoning_tokens=1\n",
        ));

        assert_eq!(summary.event_count, 2);
        assert_eq!(summary.total.input_tokens, 110);
        assert_eq!(summary.total.cached_input_tokens, 25);
        assert_eq!(summary.total.output_tokens, 44);
        assert_eq!(summary.total.reasoning_tokens, 9);
        assert_eq!(summary.by_profile["main"].total.output_tokens, 40);
    }

    #[test]
    fn info_token_usage_summary_parses_json_runtime_log_markers() {
        let summary = collect_info_token_usage_summary_from_text(
            r#"{"timestamp":"2026-04-29 10:00:00.000 +07:00","message":"token_usage request=1 transport=http profile=main source=responses_unary input_tokens=100 cached_input_tokens=25 output_tokens=40 reasoning_tokens=8","event":"token_usage","fields":{"request":"1","transport":"http","profile":"main","source":"responses_unary","input_tokens":"100","cached_input_tokens":"25","output_tokens":"40","reasoning_tokens":"8"}}"#,
        );

        assert_eq!(summary.event_count, 1);
        assert_eq!(summary.total.input_tokens, 100);
        assert!(format_info_token_usage_summary(&summary).contains("input=100"));
    }
}

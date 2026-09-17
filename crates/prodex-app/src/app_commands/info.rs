use super::*;

#[cfg(test)]
pub(crate) use crate::reports::collect_info_runtime_load_summary_from_text;
pub(crate) use crate::reports::{
    classify_prodex_process_row, collect_info_runtime_load_summary_from_texts,
    format_info_pool_remaining, format_info_runway, format_info_token_usage_summary,
    parse_ps_process_rows, select_active_runtime_log_paths_with_prefix,
    select_recent_runtime_log_paths,
};

pub(crate) fn collect_prodex_processes() -> Vec<ProdexProcessInfo> {
    try_collect_prodex_processes().unwrap_or_default()
}

pub(crate) fn try_collect_prodex_processes() -> Result<Vec<ProdexProcessInfo>> {
    let current_pid = std::process::id();
    let current_basename = std::env::current_exe().ok().and_then(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
    });

    let mut processes = try_collect_process_rows()?
        .into_iter()
        .filter_map(|row| {
            classify_prodex_process_row(row, current_pid, current_basename.as_deref())
        })
        .collect::<Vec<_>>();
    processes.sort_by_key(|process| process.pid);
    Ok(processes)
}

pub(crate) fn collect_process_rows() -> Vec<ProcessRow> {
    try_collect_process_rows().unwrap_or_default()
}

pub(crate) fn try_collect_process_rows() -> Result<Vec<ProcessRow>> {
    match collect_process_rows_from_proc() {
        Some(rows) if !rows.is_empty() => Ok(rows),
        _ => collect_process_rows_from_ps(),
    }
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
    let mut command = Command::new("ps");
    command.args(["-Ao", "pid=,comm=,args="]);
    let output = crate::command_probe_output(&mut command, "process listing")
        .context("failed to execute ps for prodex process listing")?;
    if !output.status.success() {
        bail!("ps returned exit status {}", output.status);
    }
    let text = String::from_utf8(output.stdout).context("ps output was not valid UTF-8")?;
    Ok(parse_ps_process_rows(&text))
}

pub(crate) fn collect_active_runtime_log_paths(processes: &[ProdexProcessInfo]) -> Vec<PathBuf> {
    select_active_runtime_log_paths_with_prefix(
        processes,
        prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()),
        RUNTIME_PROXY_LOG_FILE_PREFIX,
    )
}

pub(crate) fn collect_info_runtime_load_summary(
    log_paths: &[PathBuf],
    now: i64,
) -> InfoRuntimeLoadSummary {
    let tails = log_paths.iter().filter_map(|path| {
        read_runtime_log_tail(path, INFO_RUNTIME_LOG_TAIL_BYTES)
            .ok()
            .map(|tail| String::from_utf8_lossy(&tail).into_owned())
    });
    collect_info_runtime_load_summary_from_texts(
        log_paths.len(),
        tails,
        now,
        INFO_RECENT_LOAD_WINDOW_SECONDS,
        INFO_FORECAST_LOOKBACK_SECONDS,
    )
}

pub(crate) fn collect_recent_runtime_log_paths(limit: usize) -> Vec<PathBuf> {
    select_recent_runtime_log_paths(
        prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir())
            .into_iter()
            .map(|path| {
                let modified = fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(UNIX_EPOCH);
                (path, modified)
            }),
        limit,
    )
}

pub(crate) fn estimate_info_runway(
    observations: &[InfoRuntimeQuotaObservation],
    window: InfoQuotaWindow,
    current_remaining: i64,
    now: i64,
) -> Option<InfoRunwayEstimate> {
    crate::reports::estimate_info_runway(
        observations,
        window,
        current_remaining,
        now,
        INFO_FORECAST_MIN_SPAN_SECONDS,
    )
}

pub(crate) fn format_info_load_summary(
    summary: &InfoRuntimeLoadSummary,
    runtime_process_count: usize,
) -> String {
    crate::reports::format_info_load_summary(
        summary,
        runtime_process_count,
        INFO_RECENT_LOAD_WINDOW_SECONDS,
    )
}

pub(crate) fn format_runtime_policy_summary(summary: Option<&RuntimePolicySummary>) -> String {
    let path = summary.map(|summary| summary.path.display().to_string());
    crate::reports::format_runtime_policy_summary(
        path.as_deref(),
        summary.map(|summary| summary.version),
    )
}

pub(crate) fn format_runtime_proxy_contract_summary() -> String {
    crate::reports::format_runtime_proxy_contract_summary()
}

pub(crate) fn format_runtime_logs_summary() -> String {
    let directory = runtime_proxy_log_dir().display().to_string();
    crate::reports::format_runtime_logs_summary(&directory, runtime_proxy_log_format().as_str())
}

pub(crate) fn runtime_policy_json_value(
    summary: Option<&RuntimePolicySummary>,
) -> serde_json::Value {
    let path = summary.map(|summary| summary.path.display().to_string());
    crate::reports::runtime_policy_json_value(
        path.as_deref(),
        summary.map(|summary| summary.version),
    )
}

pub(crate) fn runtime_logs_json_value() -> serde_json::Value {
    let directory = runtime_proxy_log_dir().display().to_string();
    crate::reports::runtime_logs_json_value(&directory, runtime_proxy_log_format().as_str())
}

pub(crate) fn format_secret_backend_summary() -> String {
    match configured_secret_backend_selection() {
        Ok(selection) => crate::reports::format_secret_backend_summary_parts(
            Some(selection.kind().as_str()),
            selection.keyring_service(),
            None,
        ),
        Err(err) => {
            let error = redaction_redact_secret_like_text(&err.to_string());
            crate::reports::format_secret_backend_summary_parts(None, None, Some(&error))
        }
    }
}

pub(crate) fn secret_backend_json_value() -> serde_json::Value {
    match configured_secret_backend_selection() {
        Ok(selection) => crate::reports::secret_backend_json_value_parts(
            Some(selection.kind().as_str()),
            selection.keyring_service(),
            None,
        ),
        Err(err) => {
            let error = redaction_redact_secret_like_text(&err.to_string());
            crate::reports::secret_backend_json_value_parts(None, None, Some(&error))
        }
    }
}

use super::*;

pub(crate) fn handle_info(args: InfoArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let policy = runtime_policy_summary().ok().flatten();
    let process_count = collect_process_rows().len();

    if args.json {
        let value = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "active_profile": state.active_profile,
            "profile_count": state.profiles.len(),
            "runtime_policy": runtime_policy_json_value(policy.as_ref()),
            "runtime_logs": runtime_logs_json_value(),
            "secret_backend": secret_backend_json_value(),
            "process_count": process_count,
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    let fields = vec![
        ("Version".to_string(), env!("CARGO_PKG_VERSION").to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.unwrap_or_else(|| "-".to_string()),
        ),
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Runtime policy".to_string(),
            format_runtime_policy_summary(policy.as_ref()),
        ),
        ("Runtime logs".to_string(), format_runtime_logs_summary()),
        (
            "Secret backend".to_string(),
            format_secret_backend_summary(),
        ),
        ("Prodex processes".to_string(), process_count.to_string()),
    ];
    terminal_ui::print_panel("Info", &fields)?;
    Ok(())
}

pub(crate) fn collect_process_rows() -> Vec<ProcessRow> {
    try_collect_process_rows().unwrap_or_default()
}

fn try_collect_process_rows() -> Result<Vec<ProcessRow>> {
    match collect_process_rows_from_proc() {
        Some(rows) if !rows.is_empty() => Ok(rows),
        _ => collect_process_rows_from_ps(),
    }
}

fn collect_process_rows_from_proc() -> Option<Vec<ProcessRow>> {
    let mut rows = Vec::new();
    for entry in fs::read_dir("/proc").ok()?.flatten() {
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
            .filter(|chunk| !chunk.is_empty())
            .filter_map(|chunk| String::from_utf8(chunk.to_vec()).ok())
            .collect::<Vec<_>>();
        rows.push(ProcessRow { pid, command, args });
    }
    Some(rows)
}

fn collect_process_rows_from_ps() -> Result<Vec<ProcessRow>> {
    let mut command = Command::new("ps");
    command.args(["-Ao", "pid=,comm=,args="]);
    let output = crate::command_probe_output(&mut command, "process listing")
        .context("failed to execute ps for prodex process listing")?;
    if !output.status.success() {
        bail!("ps returned exit status {}", output.status);
    }
    let text = String::from_utf8(output.stdout).context("ps output was not valid UTF-8")?;
    Ok(crate::reports::parse_ps_process_rows(&text))
}

pub(crate) fn collect_recent_runtime_log_paths(limit: usize) -> Vec<PathBuf> {
    crate::reports::select_recent_runtime_log_paths(
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

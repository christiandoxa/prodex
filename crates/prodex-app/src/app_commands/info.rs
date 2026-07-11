use super::*;
use redaction::redaction_redact_secret_like_text;

#[cfg(test)]
pub(crate) use prodex_app_reports::collect_info_runtime_load_summary_from_text;
pub(crate) use prodex_app_reports::{
    InfoTokenUsageSummary, classify_prodex_process_row,
    collect_info_runtime_load_summary_from_texts, collect_info_token_usage_summary_from_texts,
    format_info_pool_remaining, format_info_process_summary, format_info_quota_data_summary,
    format_info_runway, format_info_token_usage_summary, format_runtime_tuning_budgets,
    format_runtime_tuning_transport, format_runtime_tuning_workers, parse_ps_process_rows,
    select_active_runtime_log_paths_with_prefix, select_recent_runtime_log_paths,
};

pub(crate) fn collect_info_quota_aggregate(
    paths: &AppPaths,
    state: &AppState,
    now: i64,
) -> InfoQuotaAggregate {
    let codex_runtime_profiles = state
        .profiles
        .iter()
        .filter(|(_, profile)| profile.provider.supports_codex_runtime())
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();

    if codex_runtime_profiles.is_empty() {
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
    let reports = collect_run_profile_reports(state, codex_runtime_profiles, None, false);
    build_info_quota_aggregate(&reports, &persisted_usage_snapshots, now)
}

pub(crate) fn format_info_provider_summary(profiles: &BTreeMap<String, ProfileEntry>) -> String {
    if profiles.is_empty() {
        return "none".to_string();
    }

    let mut counts = BTreeMap::<&'static str, usize>::new();
    for profile in profiles.values() {
        *counts.entry(profile.provider.label()).or_default() += 1;
    }

    counts
        .into_iter()
        .map(|(provider, count)| format!("{provider}={count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn format_info_provider_capabilities_summary(
    profiles: &BTreeMap<String, ProfileEntry>,
) -> String {
    if profiles.is_empty() {
        return "none".to_string();
    }

    let mut native = 0;
    let mut adapter = 0;
    let mut external_cli = 0;
    let mut unsupported = 0;
    let mut openai_format = 0;
    let mut quota_shapes = BTreeMap::<&'static str, usize>::new();

    for profile in profiles.values() {
        let capabilities = profile.provider.capabilities();
        if capabilities.uses_openai_client_format {
            openai_format += 1;
        }
        match capabilities.runtime_route_policy {
            prodex_state::RuntimeRoutePolicy::NativeCodex => native += 1,
            prodex_state::RuntimeRoutePolicy::ResponsesAdapter => adapter += 1,
            prodex_state::RuntimeRoutePolicy::ExternalCli => external_cli += 1,
            prodex_state::RuntimeRoutePolicy::Unsupported => unsupported += 1,
        }
        *quota_shapes
            .entry(capabilities.quota_shape.label())
            .or_default() += 1;
    }

    let quota_shapes = quota_shapes
        .into_iter()
        .map(|(shape, count)| format!("{shape}={count}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "routes native={native}, adapter={adapter}, external-cli={external_cli}, unsupported={unsupported}; openai-format={openai_format}; quota {quota_shapes}"
    )
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
                let modified = runtime_log_modified(&path);
                (path, modified)
            }),
        limit,
    )
}

fn runtime_log_modified(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(UNIX_EPOCH)
}

pub(crate) fn collect_info_token_usage_summary(log_paths: &[PathBuf]) -> InfoTokenUsageSummary {
    let tails = log_paths.iter().filter_map(|path| {
        read_runtime_log_tail(path, INFO_RUNTIME_LOG_TAIL_BYTES)
            .ok()
            .map(|tail| String::from_utf8_lossy(&tail).into_owned())
    });
    collect_info_token_usage_summary_from_texts(log_paths.len(), tails)
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

pub(crate) fn format_info_load_summary(
    summary: &InfoRuntimeLoadSummary,
    runtime_process_count: usize,
) -> String {
    prodex_app_reports::format_info_load_summary(
        summary,
        runtime_process_count,
        INFO_RECENT_LOAD_WINDOW_SECONDS,
    )
}

pub(crate) fn format_runtime_policy_summary(summary: Option<&RuntimePolicySummary>) -> String {
    let path = summary.map(|summary| summary.path.display().to_string());
    prodex_app_reports::format_runtime_policy_summary(
        path.as_deref(),
        summary.map(|summary| summary.version),
    )
}

pub(crate) fn format_runtime_proxy_contract_summary() -> String {
    prodex_app_reports::format_runtime_proxy_contract_summary()
}

pub(crate) fn format_runtime_proxy_preset() -> String {
    runtime_policy_proxy()
        .and_then(|policy| policy.preset().map(|preset| preset.as_str().to_string()))
        .unwrap_or_else(|| "default".to_string())
}

pub(crate) fn format_runtime_logs_summary() -> String {
    let directory = runtime_proxy_log_dir().display().to_string();
    prodex_app_reports::format_runtime_logs_summary(&directory, runtime_proxy_log_format().as_str())
}

pub(crate) fn runtime_policy_json_value(
    summary: Option<&RuntimePolicySummary>,
) -> serde_json::Value {
    let path = summary.map(|summary| summary.path.display().to_string());
    prodex_app_reports::runtime_policy_json_value(
        path.as_deref(),
        summary.map(|summary| summary.version),
    )
}

pub(crate) fn runtime_logs_json_value() -> serde_json::Value {
    let directory = runtime_proxy_log_dir().display().to_string();
    prodex_app_reports::runtime_logs_json_value(&directory, runtime_proxy_log_format().as_str())
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
    let selection = secret_store::SecretBackendSelection::from_kind(backend, keyring_service)
        .map_err(anyhow::Error::new)?;
    if let secret_store::SecretBackendSelection::Keyring(backend) = &selection
        && !backend.is_supported()
    {
        anyhow::bail!("{}", backend.unsupported_reason());
    }
    Ok(selection)
}

pub(crate) fn format_secret_backend_summary() -> String {
    match configured_secret_backend_selection() {
        Ok(selection) => prodex_app_reports::format_secret_backend_summary_parts(
            Some(selection.kind().as_str()),
            selection.keyring_service(),
            None,
        ),
        Err(err) => {
            let error = secret_backend_redacted_error(&err);
            prodex_app_reports::format_secret_backend_summary_parts(None, None, Some(&error))
        }
    }
}

pub(crate) fn secret_backend_json_value() -> serde_json::Value {
    match configured_secret_backend_selection() {
        Ok(selection) => prodex_app_reports::secret_backend_json_value_parts(
            Some(selection.kind().as_str()),
            selection.keyring_service(),
            None,
        ),
        Err(err) => {
            let error = secret_backend_redacted_error(&err);
            prodex_app_reports::secret_backend_json_value_parts(None, None, Some(&error))
        }
    }
}

fn secret_backend_redacted_error(err: &anyhow::Error) -> String {
    redaction_redact_secret_like_text(&err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_backend_error_redacts_secret_like_material() {
        let err = anyhow::anyhow!(
            "failed: Authorization: Bearer fixture-token-123 url=https://example.test?api_key=sk-fixture-123"
        );

        let message = secret_backend_redacted_error(&err);

        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("fixture-token-123"));
        assert!(!message.contains("sk-fixture-123"));
    }
}

use super::*;
use prodex_runtime_doctor::read_runtime_log_tail;
pub(crate) use prodex_update_notice::format_info_codex_version;

pub(crate) use crate::reports::{
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
            five_hour_profiles_with_data: 0,
            weekly_profiles_with_data: 0,
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
    let labels = profiles
        .values()
        .map(|profile| profile.provider.label())
        .collect::<Vec<_>>();
    prodex_mojo_core::info_render::format_provider_summary(&labels)
        .expect("compiled Mojo provider summary failed")
}

pub(crate) fn format_info_provider_capabilities_summary(
    profiles: &BTreeMap<String, ProfileEntry>,
) -> String {
    use prodex_mojo_core::info_render::{InfoProviderCapability, InfoProviderRoutePolicy};
    let facts = profiles
        .values()
        .map(|profile| {
            let capabilities = profile.provider.capabilities();
            InfoProviderCapability {
                route_policy: match capabilities.runtime_route_policy {
                    prodex_state::RuntimeRoutePolicy::NativeCodex => {
                        InfoProviderRoutePolicy::NativeCodex
                    }
                    prodex_state::RuntimeRoutePolicy::ResponsesAdapter => {
                        InfoProviderRoutePolicy::ResponsesAdapter
                    }
                    prodex_state::RuntimeRoutePolicy::ExternalCli => {
                        InfoProviderRoutePolicy::ExternalCli
                    }
                    prodex_state::RuntimeRoutePolicy::Unsupported => {
                        InfoProviderRoutePolicy::Unsupported
                    }
                },
                uses_openai_client_format: capabilities.uses_openai_client_format,
                quota_shape: capabilities.quota_shape.label(),
            }
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::info_render::format_provider_capabilities_summary(&facts)
        .expect("compiled Mojo provider capability summary failed")
}

pub(crate) fn build_info_quota_aggregate(
    reports: &[RunProfileProbeReport],
    persisted_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
) -> InfoQuotaAggregate {
    crate::reports::build_info_quota_aggregate(
        reports,
        persisted_usage_snapshots,
        now,
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    )
}

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

    let processes = try_collect_process_rows()?
        .into_iter()
        .filter_map(|row| {
            classify_prodex_process_row(row, current_pid, current_basename.as_deref())
        })
        .collect::<Vec<_>>();
    order_prodex_process_rows(processes)
}

/// Keep the original detected process records. The compiled info renderer
/// owns PID ordering and stable equal-PID precedence; no Rust sorting policy.
fn order_prodex_process_rows(processes: Vec<ProdexProcessInfo>) -> Result<Vec<ProdexProcessInfo>> {
    let pids = processes
        .iter()
        .map(|process| process.pid)
        .collect::<Vec<_>>();
    let selected =
        prodex_mojo_core::info_render::info_process_pid_order(&pids).map_err(|error| {
            anyhow::anyhow!("compiled Mojo info process PID ordering failed: {error:?}")
        })?;
    let mut original = processes.into_iter().map(Some).collect::<Vec<_>>();
    Ok(selected
        .into_iter()
        .map(|index| {
            original[index]
                .take()
                .expect("validated Mojo process indices are unique")
        })
        .collect())
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

pub(crate) fn format_audit_logs_summary() -> String {
    crate::audit_log::format_audit_logs_summary()
}

pub(crate) fn format_info_prodex_version(paths: &AppPaths) -> Result<String> {
    let current = prodex_update_notice::current_prodex_version();
    Ok(match prodex_update_notice::prodex_version_status(paths)? {
        prodex_update_notice::ProdexVersionStatus::UpToDate => {
            format!("{current} (up to date)")
        }
        prodex_update_notice::ProdexVersionStatus::UpdateAvailable(latest) => {
            format!("{current} (update available: {latest})")
        }
        prodex_update_notice::ProdexVersionStatus::Unknown => {
            format!("{current} (update check unavailable)")
        }
    })
}

pub(crate) fn format_runtime_proxy_preset() -> String {
    runtime_policy_proxy()
        .and_then(|policy| policy.preset().map(|preset| preset.as_str().to_string()))
        .unwrap_or_else(|| "default".to_string())
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

#[cfg(test)]
mod parity_tests {
    use super::*;

    #[test]
    fn audit_logs_summary_reports_the_real_configured_path_and_existence() {
        let root = crate::test_support::test_temp_root()
            .join(format!("prodex-info-audit-parity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("audit directory");
        let _guard = crate::test_support::TestEnvVarGuard::set(
            "PRODEX_AUDIT_LOG_DIR",
            &root.to_string_lossy(),
        );
        let path = root.join("prodex-audit.log");

        assert_eq!(
            format_audit_logs_summary(),
            format!("{} (missing)", path.display())
        );

        std::fs::write(&path, b"audit\n").expect("audit log");
        assert_eq!(
            format_audit_logs_summary(),
            format!("{} (exists)", path.display())
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
#[path = "info_provider_tests.rs"]
mod provider_tests;

#[cfg(test)]
mod compiled_info_pid_order_caller_tests {
    use super::*;

    fn process(pid: u32, command: &str, runtime: bool) -> ProdexProcessInfo {
        ProdexProcessInfo {
            pid,
            command: command.to_owned(),
            runtime,
        }
    }

    #[test]
    fn compiled_info_process_pid_order_retains_full_original_rows_on_equal_pid() {
        let processes = vec![
            process(42, "first duplicate", true),
            process(u32::MAX, "highest", false),
            process(0, "lowest", true),
            process(17, "middle", false),
            process(42, "second duplicate", false),
            process(1, "early", true),
        ];
        let result = order_prodex_process_rows(processes).unwrap();
        assert_eq!(
            result.iter().map(|row| row.pid).collect::<Vec<_>>(),
            [0, 1, 17, 42, 42, u32::MAX],
        );
        assert_eq!(result[3].command, "first duplicate");
        assert_eq!(result[4].command, "second duplicate");
        assert!(result[3].runtime);
        assert!(!result[4].runtime);
        assert!(order_prodex_process_rows(Vec::new()).unwrap().is_empty());
    }

    #[test]
    fn compiled_info_process_pid_order_handles_many_processes_stably() {
        let processes = (0..1025)
            .rev()
            .map(|pid| process(pid, &format!("proc-{pid}"), pid % 2 == 0))
            .collect::<Vec<_>>();
        let ordered = order_prodex_process_rows(processes).unwrap();
        assert_eq!(ordered.len(), 1025);
        assert_eq!(ordered[0].pid, 0);
        assert_eq!(ordered[1024].pid, 1024);
        assert_eq!(ordered[0].command, "proc-0");
        assert!(ordered.windows(2).all(|pair| pair[0].pid <= pair[1].pid));
    }
}

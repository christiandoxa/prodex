use super::*;

pub(crate) fn handle_info(_args: InfoArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let policy_summary = runtime_policy_summary()?;
    let runtime_tuning = collect_runtime_tuning_snapshot();
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
        (
            "Runtime workers".to_string(),
            format_runtime_tuning_workers(&runtime_tuning),
        ),
        (
            "Runtime budgets".to_string(),
            format_runtime_tuning_budgets(&runtime_tuning),
        ),
        (
            "Runtime transport".to_string(),
            format_runtime_tuning_transport(&runtime_tuning),
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

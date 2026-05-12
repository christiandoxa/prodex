use super::*;

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

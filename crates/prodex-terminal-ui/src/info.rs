#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InfoLoadSummaryDisplay {
    pub log_count: usize,
    pub active_inflight_units: usize,
    pub recent_selection_events: usize,
    pub recent_first_timestamp: Option<i64>,
    pub recent_last_timestamp: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsageCounts {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsageProfileDisplay<'a> {
    pub profile: &'a str,
    pub total: TokenUsageCounts,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfoRunwayEstimateDisplay<'a> {
    pub burn_per_hour: f64,
    pub observed_profiles: usize,
    pub observed_span_seconds: i64,
    pub exhaust_at: i64,
    pub exhaust_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoRunwayResetDisplay<'a> {
    pub reset_at: i64,
    pub reset_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningWorkersDisplay {
    pub worker_count: usize,
    pub long_lived_worker_count: usize,
    pub async_worker_count: usize,
    pub probe_refresh_worker_count: usize,
    pub active_request_limit: usize,
    pub long_lived_queue_capacity: usize,
    pub lane_responses: usize,
    pub lane_compact: usize,
    pub lane_websocket: usize,
    pub lane_standard: usize,
    pub websocket_connect_worker_count: usize,
    pub websocket_connect_queue_capacity: usize,
    pub websocket_connect_overflow_capacity: usize,
    pub websocket_dns_worker_count: usize,
    pub websocket_dns_queue_capacity: usize,
    pub websocket_dns_overflow_capacity: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningBudgetsDisplay {
    pub precommit_attempt_limit: usize,
    pub precommit_budget_ms: u64,
    pub pressure_precommit_attempt_limit: usize,
    pub pressure_precommit_budget_ms: u64,
    pub continuation_precommit_attempt_limit: usize,
    pub continuation_precommit_budget_ms: u64,
    pub admission_wait_budget_ms: u64,
    pub pressure_admission_wait_budget_ms: u64,
    pub long_lived_queue_wait_budget_ms: u64,
    pub pressure_long_lived_queue_wait_budget_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningTransportDisplay {
    pub http_connect_timeout_ms: u64,
    pub stream_idle_timeout_ms: u64,
    pub sse_lookahead_timeout_ms: u64,
    pub websocket_connect_timeout_ms: u64,
    pub websocket_precommit_progress_timeout_ms: u64,
    pub websocket_happy_eyeballs_delay_ms: u64,
    pub websocket_previous_response_reuse_stale_ms: u64,
    pub profile_inflight_soft_limit: usize,
    pub profile_inflight_hard_limit: usize,
}

pub fn format_info_process_summary_display(
    total_count: usize,
    runtime_count: usize,
    pids: impl IntoIterator<Item = u32>,
    max_visible_pids: usize,
) -> String {
    if total_count == 0 {
        return "No".to_string();
    }

    let pid_list = pids
        .into_iter()
        .take(max_visible_pids)
        .map(|pid| pid.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = total_count.saturating_sub(max_visible_pids);
    let extra = if remaining > 0 {
        format!(" (+{remaining} more)")
    } else {
        String::new()
    };

    format!("Yes ({total_count} total, {runtime_count} runtime; pids: {pid_list}{extra})")
}

pub fn format_info_load_summary_display(
    summary: InfoLoadSummaryDisplay,
    runtime_process_count: usize,
    recent_load_window_seconds: i64,
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
        .unwrap_or_else(|| format!("{}m", recent_load_window_seconds / 60));

    format!(
        "{} selection event(s) over {}; inflight units {}; {} active runtime log(s)",
        summary.recent_selection_events,
        activity_span,
        summary.active_inflight_units,
        summary.log_count
    )
}

pub fn format_info_quota_data_summary_display(
    quota_compatible_profiles: usize,
    live_profiles: usize,
    snapshot_profiles: usize,
    unavailable_profiles: usize,
) -> String {
    if quota_compatible_profiles == 0 {
        return "No quota-compatible profiles".to_string();
    }

    format!(
        "{quota_compatible_profiles} quota-compatible profile(s): live={live_profiles}, snapshot={snapshot_profiles}, unavailable={unavailable_profiles}"
    )
}

pub fn format_info_token_usage_summary_display<'a>(
    event_count: usize,
    log_count: usize,
    total: TokenUsageCounts,
    by_profile: impl IntoIterator<Item = TokenUsageProfileDisplay<'a>>,
) -> String {
    if event_count == 0 {
        return format!("No token_usage events found in {log_count} recent runtime log(s)");
    }

    let top_profiles = by_profile
        .into_iter()
        .take(4)
        .map(|entry| {
            format!(
                "{}:{} in/{} cached/{} out/{} reasoning",
                entry.profile,
                entry.total.input_tokens,
                entry.total.cached_input_tokens,
                entry.total.output_tokens,
                entry.total.reasoning_tokens
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let suffix = if top_profiles.is_empty() {
        String::new()
    } else {
        format!("; by profile: {top_profiles}")
    };

    format!(
        "{} event(s), logs={}: input={}, cached_input={}, output={}, reasoning={}{}",
        event_count,
        log_count,
        total.input_tokens,
        total.cached_input_tokens,
        total.output_tokens,
        total.reasoning_tokens,
        suffix
    )
}

pub fn format_runtime_policy_summary_display(path: Option<&str>, version: Option<u32>) -> String {
    path.zip(version)
        .map(|(path, version)| format!("{path} (v{version})"))
        .unwrap_or_else(|| "disabled".to_string())
}

pub fn format_runtime_logs_summary_display(directory: &str, format: &str) -> String {
    format!("{directory} ({format})")
}

pub fn format_runtime_tuning_workers_display(snapshot: RuntimeTuningWorkersDisplay) -> String {
    format!(
        "workers proxy={}, long-lived={}, async={}, probe-refresh={}; active={}, queue={}; lanes responses={}, compact={}, websocket={}, standard={}; ws-connect workers={}, queue={}, overflow={}; ws-dns workers={}, queue={}, overflow={}",
        snapshot.worker_count,
        snapshot.long_lived_worker_count,
        snapshot.async_worker_count,
        snapshot.probe_refresh_worker_count,
        snapshot.active_request_limit,
        snapshot.long_lived_queue_capacity,
        snapshot.lane_responses,
        snapshot.lane_compact,
        snapshot.lane_websocket,
        snapshot.lane_standard,
        snapshot.websocket_connect_worker_count,
        snapshot.websocket_connect_queue_capacity,
        snapshot.websocket_connect_overflow_capacity,
        snapshot.websocket_dns_worker_count,
        snapshot.websocket_dns_queue_capacity,
        snapshot.websocket_dns_overflow_capacity
    )
}

pub fn format_runtime_tuning_budgets_display(snapshot: RuntimeTuningBudgetsDisplay) -> String {
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

pub fn format_runtime_tuning_transport_display(snapshot: RuntimeTuningTransportDisplay) -> String {
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

pub fn format_info_pool_remaining_display(
    total_remaining: i64,
    profiles_with_data: usize,
    earliest_reset_text: Option<&str>,
) -> String {
    if profiles_with_data == 0 {
        return "Unavailable".to_string();
    }

    let mut value = format!("{total_remaining}% across {profiles_with_data} profile(s)");
    if let Some(reset_text) = earliest_reset_text {
        value.push_str(&format!("; earliest reset {reset_text}"));
    }
    value
}

pub fn format_info_runway_display(
    profiles_with_data: usize,
    current_remaining: i64,
    earliest_reset: Option<InfoRunwayResetDisplay<'_>>,
    estimate: Option<InfoRunwayEstimateDisplay<'_>>,
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
    if let Some(reset) = earliest_reset
        && reset.reset_at <= estimate.exhaust_at
    {
        return format!(
            "Earliest reset {} arrives before the no-reset runway (~{} at {} aggregated-%/h, {} profile(s), observed over {})",
            reset.reset_text,
            format_relative_duration(estimate.exhaust_at.saturating_sub(now)),
            burn,
            estimate.observed_profiles,
            observed
        );
    }

    format!(
        "{} (~{}) at {} aggregated-%/h from {} profile(s), observed over {}, no-reset estimate",
        estimate.exhaust_text,
        format_relative_duration(estimate.exhaust_at.saturating_sub(now)),
        burn,
        estimate.observed_profiles,
        observed
    )
}

pub fn format_relative_duration(seconds: i64) -> String {
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

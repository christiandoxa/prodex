use std::fmt::Display;

fn display_usize(value: usize) -> u64 {
    u64::try_from(value).expect("display count fits u64")
}

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

pub fn format_info_process_summary_display<I, T>(
    total_count: usize,
    runtime_count: usize,
    processes: I,
    max_visible_processes: usize,
) -> String
where
    I: IntoIterator<Item = T>,
    T: Display,
{
    let processes = processes
        .into_iter()
        .take(max_visible_processes)
        .map(|process| process.to_string())
        .collect::<Vec<_>>();
    prodex_mojo_core::info_render::format_process_summary(
        total_count,
        runtime_count,
        max_visible_processes,
        &processes,
    )
    .expect("Mojo info process-summary renderer returned invalid output")
}

pub fn format_info_load_summary_display(
    summary: InfoLoadSummaryDisplay,
    runtime_process_count: usize,
    recent_load_window_seconds: i64,
) -> String {
    prodex_mojo_core::info_render::format_load_summary(
        summary.log_count,
        summary.active_inflight_units,
        summary.recent_selection_events,
        summary.recent_first_timestamp,
        summary.recent_last_timestamp,
        runtime_process_count,
        recent_load_window_seconds,
    )
    .expect("Mojo info load-summary renderer returned invalid output")
}

pub fn format_info_quota_data_summary_display(
    quota_compatible_profiles: usize,
    live_profiles: usize,
    snapshot_profiles: usize,
    unavailable_profiles: usize,
) -> String {
    prodex_mojo_core::info_render::format_quota_data_summary([
        display_usize(quota_compatible_profiles),
        display_usize(live_profiles),
        display_usize(snapshot_profiles),
        display_usize(unavailable_profiles),
    ])
    .expect("Mojo info quota-data renderer returned invalid output")
}

pub fn format_info_token_usage_summary_display<'a>(
    event_count: usize,
    log_count: usize,
    total: TokenUsageCounts,
    by_profile: impl IntoIterator<Item = TokenUsageProfileDisplay<'a>>,
) -> String {
    let profiles = by_profile
        .into_iter()
        .take(4)
        .map(
            |entry| prodex_mojo_core::info_render::InfoTokenUsageProfile {
                profile: entry.profile,
                input_tokens: entry.total.input_tokens,
                cached_input_tokens: entry.total.cached_input_tokens,
                output_tokens: entry.total.output_tokens,
                reasoning_tokens: entry.total.reasoning_tokens,
            },
        )
        .collect::<Vec<_>>();
    prodex_mojo_core::info_render::format_token_usage_summary(
        event_count,
        log_count,
        [
            total.input_tokens,
            total.cached_input_tokens,
            total.output_tokens,
            total.reasoning_tokens,
        ],
        &profiles,
    )
    .expect("Mojo info token-usage renderer returned invalid output")
}

pub fn format_runtime_policy_summary_display(path: Option<&str>, version: Option<u32>) -> String {
    prodex_mojo_core::info_render::format_runtime_policy_summary(path, version)
        .expect("Mojo runtime-policy summary renderer returned invalid output")
}

pub fn format_runtime_logs_summary_display(directory: &str, format: &str) -> String {
    prodex_mojo_core::info_render::format_runtime_logs_summary(directory, format)
        .expect("Mojo runtime-log summary renderer returned invalid output")
}

pub fn format_runtime_tuning_workers_display(snapshot: RuntimeTuningWorkersDisplay) -> String {
    prodex_mojo_core::info_render::format_runtime_tuning_workers([
        display_usize(snapshot.worker_count),
        display_usize(snapshot.long_lived_worker_count),
        display_usize(snapshot.async_worker_count),
        display_usize(snapshot.probe_refresh_worker_count),
        display_usize(snapshot.active_request_limit),
        display_usize(snapshot.long_lived_queue_capacity),
        display_usize(snapshot.lane_responses),
        display_usize(snapshot.lane_compact),
        display_usize(snapshot.lane_websocket),
        display_usize(snapshot.lane_standard),
        display_usize(snapshot.websocket_connect_worker_count),
        display_usize(snapshot.websocket_connect_queue_capacity),
        display_usize(snapshot.websocket_connect_overflow_capacity),
        display_usize(snapshot.websocket_dns_worker_count),
        display_usize(snapshot.websocket_dns_queue_capacity),
        display_usize(snapshot.websocket_dns_overflow_capacity),
    ])
    .expect("Mojo runtime-tuning worker renderer returned invalid output")
}

pub fn format_runtime_tuning_budgets_display(snapshot: RuntimeTuningBudgetsDisplay) -> String {
    prodex_mojo_core::info_render::format_runtime_tuning_budgets([
        display_usize(snapshot.precommit_attempt_limit),
        snapshot.precommit_budget_ms,
        display_usize(snapshot.pressure_precommit_attempt_limit),
        snapshot.pressure_precommit_budget_ms,
        display_usize(snapshot.continuation_precommit_attempt_limit),
        snapshot.continuation_precommit_budget_ms,
        snapshot.admission_wait_budget_ms,
        snapshot.pressure_admission_wait_budget_ms,
        snapshot.long_lived_queue_wait_budget_ms,
        snapshot.pressure_long_lived_queue_wait_budget_ms,
    ])
    .expect("Mojo runtime-tuning budget renderer returned invalid output")
}

pub fn format_runtime_tuning_transport_display(snapshot: RuntimeTuningTransportDisplay) -> String {
    prodex_mojo_core::info_render::format_runtime_tuning_transport([
        snapshot.http_connect_timeout_ms,
        snapshot.stream_idle_timeout_ms,
        snapshot.sse_lookahead_timeout_ms,
        snapshot.websocket_connect_timeout_ms,
        snapshot.websocket_precommit_progress_timeout_ms,
        snapshot.websocket_happy_eyeballs_delay_ms,
        snapshot.websocket_previous_response_reuse_stale_ms,
        display_usize(snapshot.profile_inflight_soft_limit),
        display_usize(snapshot.profile_inflight_hard_limit),
    ])
    .expect("Mojo runtime-tuning transport renderer returned invalid output")
}

pub fn format_info_pool_remaining_display(
    total_remaining: i64,
    profiles_with_data: usize,
    earliest_reset_text: Option<&str>,
) -> String {
    prodex_mojo_core::info_render::format_pool_remaining(
        total_remaining,
        profiles_with_data,
        earliest_reset_text,
    )
    .expect("Mojo info pool-remaining renderer returned invalid output")
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
    prodex_mojo_core::info_render::format_relative_duration(seconds)
        .expect("Mojo relative-duration renderer returned invalid output")
}

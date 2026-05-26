use super::*;

pub(super) use prodex_runtime_tuning::{
    RuntimeProxyLaneLimitOverrides, RuntimeTuningLaneLimits, RuntimeTuningPrecommitBudget,
    RuntimeTuningSnapshot, RuntimeTuningSnapshotInput, percent_override_with_policy,
    runtime_probe_refresh_worker_count_default, runtime_proxy_active_request_limit_default,
    runtime_proxy_async_worker_count_default, runtime_proxy_lane_limits_from_overrides,
    runtime_proxy_log_queue_capacity_default, runtime_proxy_long_lived_queue_capacity_default,
    runtime_proxy_long_lived_worker_count_default, runtime_proxy_worker_count_default,
    runtime_take_fault_injection, runtime_websocket_dns_resolve_overflow_capacity_default,
    runtime_websocket_dns_resolve_queue_capacity_default,
    runtime_websocket_dns_resolve_worker_count_default,
    runtime_websocket_tcp_connect_overflow_capacity_default,
    runtime_websocket_tcp_connect_queue_capacity_default,
    runtime_websocket_tcp_connect_worker_count_default, timeout_override_ms_with_policy,
    usize_override_with_policy, usize_override_with_policy_allow_zero,
};

pub(crate) fn collect_runtime_tuning_snapshot() -> RuntimeTuningSnapshot {
    let worker_count = runtime_proxy_worker_count();
    let long_lived_worker_count = runtime_proxy_long_lived_worker_count();
    let long_lived_queue_capacity =
        runtime_proxy_long_lived_queue_capacity(long_lived_worker_count);
    let active_request_limit =
        runtime_proxy_active_request_limit(worker_count, long_lived_worker_count);
    let lane_limits =
        runtime_proxy_lane_limits(active_request_limit, worker_count, long_lived_worker_count);
    let (precommit_attempt_limit, precommit_budget) = runtime_proxy_precommit_budget(false, false);
    let (pressure_precommit_attempt_limit, pressure_precommit_budget) =
        runtime_proxy_precommit_budget(false, true);
    let (continuation_precommit_attempt_limit, continuation_precommit_budget) =
        runtime_proxy_precommit_budget(true, false);
    let websocket_connect_worker_count = runtime_websocket_tcp_connect_worker_count();
    let websocket_connect_queue_capacity =
        runtime_websocket_tcp_connect_queue_capacity(websocket_connect_worker_count);
    let websocket_connect_overflow_capacity = runtime_websocket_tcp_connect_overflow_capacity(
        websocket_connect_worker_count,
        websocket_connect_queue_capacity,
    );
    let websocket_dns_worker_count = runtime_websocket_dns_resolve_worker_count();
    let websocket_dns_queue_capacity =
        runtime_websocket_dns_resolve_queue_capacity(websocket_dns_worker_count);
    let websocket_dns_overflow_capacity = runtime_websocket_dns_resolve_overflow_capacity(
        websocket_dns_worker_count,
        websocket_dns_queue_capacity,
    );

    RuntimeTuningSnapshotInput {
        worker_count,
        long_lived_worker_count,
        async_worker_count: runtime_proxy_async_worker_count(),
        probe_refresh_worker_count: runtime_probe_refresh_worker_count(),
        long_lived_queue_capacity,
        active_request_limit,
        lane_limits: RuntimeTuningLaneLimits {
            responses: lane_limits.responses,
            compact: lane_limits.compact,
            websocket: lane_limits.websocket,
            standard: lane_limits.standard,
        },
        precommit: RuntimeTuningPrecommitBudget {
            attempt_limit: precommit_attempt_limit,
            budget: precommit_budget,
        },
        pressure_precommit: RuntimeTuningPrecommitBudget {
            attempt_limit: pressure_precommit_attempt_limit,
            budget: pressure_precommit_budget,
        },
        continuation_precommit: RuntimeTuningPrecommitBudget {
            attempt_limit: continuation_precommit_attempt_limit,
            budget: continuation_precommit_budget,
        },
        admission_wait_budget_ms: runtime_proxy_admission_wait_budget_ms(),
        pressure_admission_wait_budget_ms: runtime_proxy_pressure_admission_wait_budget_ms(),
        long_lived_queue_wait_budget_ms: runtime_proxy_long_lived_queue_wait_budget_ms(),
        pressure_long_lived_queue_wait_budget_ms:
            runtime_proxy_pressure_long_lived_queue_wait_budget_ms(),
        http_connect_timeout_ms: runtime_proxy_http_connect_timeout_ms(),
        stream_idle_timeout_ms: runtime_proxy_stream_idle_timeout_ms(),
        sse_lookahead_timeout_ms: runtime_proxy_sse_lookahead_timeout_ms(),
        websocket_connect_timeout_ms: runtime_proxy_websocket_connect_timeout_ms(),
        websocket_happy_eyeballs_delay_ms: runtime_proxy_websocket_happy_eyeballs_delay_ms(),
        websocket_precommit_progress_timeout_ms:
            runtime_proxy_websocket_precommit_progress_timeout_ms(),
        websocket_connect_worker_count,
        websocket_connect_queue_capacity,
        websocket_connect_overflow_capacity,
        websocket_dns_worker_count,
        websocket_dns_queue_capacity,
        websocket_dns_overflow_capacity,
        websocket_previous_response_reuse_stale_ms:
            runtime_proxy_websocket_previous_response_reuse_stale_ms(),
        profile_inflight_soft_limit: runtime_proxy_profile_inflight_soft_limit(),
        profile_inflight_hard_limit: runtime_proxy_profile_inflight_hard_limit(),
    }
    .into_snapshot()
}

pub(super) fn runtime_proxy_http_connect_timeout_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS",
        runtime_policy_proxy().and_then(|policy| policy.http_connect_timeout_ms),
        RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_stream_idle_timeout_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS",
        runtime_policy_proxy().and_then(|policy| policy.stream_idle_timeout_ms),
        RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_compact_request_timeout_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS",
        runtime_policy_proxy().and_then(|policy| policy.compact_request_timeout_ms),
        RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_sse_lookahead_timeout_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS",
        runtime_policy_proxy().and_then(|policy| policy.sse_lookahead_timeout_ms),
        RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_prefetch_backpressure_retry_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS",
        runtime_policy_proxy().and_then(|policy| policy.prefetch_backpressure_retry_ms),
        RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS,
    )
}

pub(super) fn runtime_proxy_prefetch_backpressure_timeout_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS",
        runtime_policy_proxy().and_then(|policy| policy.prefetch_backpressure_timeout_ms),
        RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_prefetch_max_buffered_bytes() -> usize {
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES",
        runtime_policy_proxy().and_then(|policy| policy.prefetch_max_buffered_bytes),
        RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES,
    )
}

pub(super) fn runtime_proxy_websocket_connect_timeout_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS",
        runtime_policy_proxy().and_then(|policy| policy.websocket_connect_timeout_ms),
        RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_websocket_happy_eyeballs_delay_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS",
        runtime_policy_proxy().and_then(|policy| policy.websocket_happy_eyeballs_delay_ms),
        RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS,
    )
}

pub(super) fn runtime_proxy_websocket_precommit_progress_timeout_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS",
        runtime_policy_proxy().and_then(|policy| policy.websocket_precommit_progress_timeout_ms),
        RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS,
    )
}

pub(super) fn runtime_websocket_tcp_connect_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    usize_override_with_policy(
        "PRODEX_RUNTIME_WEBSOCKET_CONNECT_WORKER_COUNT",
        runtime_policy_proxy().and_then(|policy| policy.websocket_connect_worker_count),
        runtime_websocket_tcp_connect_worker_count_default(parallelism),
    )
    .max(1)
}

pub(super) fn runtime_websocket_tcp_connect_queue_capacity(worker_count: usize) -> usize {
    usize_override_with_policy(
        "PRODEX_RUNTIME_WEBSOCKET_CONNECT_QUEUE_CAPACITY",
        runtime_policy_proxy().and_then(|policy| policy.websocket_connect_queue_capacity),
        runtime_websocket_tcp_connect_queue_capacity_default(worker_count),
    )
    .max(worker_count)
    .max(1)
}

pub(super) fn runtime_websocket_tcp_connect_overflow_capacity(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    usize_override_with_policy_allow_zero(
        "PRODEX_RUNTIME_WEBSOCKET_CONNECT_OVERFLOW_CAPACITY",
        runtime_policy_proxy().and_then(|policy| policy.websocket_connect_overflow_capacity),
        runtime_websocket_tcp_connect_overflow_capacity_default(worker_count, queue_capacity),
    )
}

pub(super) fn runtime_websocket_dns_resolve_worker_count() -> usize {
    let parallelism = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(2);
    usize_override_with_policy(
        "PRODEX_RUNTIME_WEBSOCKET_DNS_WORKER_COUNT",
        runtime_policy_proxy().and_then(|policy| policy.websocket_dns_worker_count),
        runtime_websocket_dns_resolve_worker_count_default(parallelism),
    )
    .max(1)
}

pub(super) fn runtime_websocket_dns_resolve_queue_capacity(worker_count: usize) -> usize {
    usize_override_with_policy(
        "PRODEX_RUNTIME_WEBSOCKET_DNS_QUEUE_CAPACITY",
        runtime_policy_proxy().and_then(|policy| policy.websocket_dns_queue_capacity),
        runtime_websocket_dns_resolve_queue_capacity_default(worker_count),
    )
    .max(worker_count)
    .max(1)
}

pub(super) fn runtime_websocket_dns_resolve_overflow_capacity(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    usize_override_with_policy_allow_zero(
        "PRODEX_RUNTIME_WEBSOCKET_DNS_OVERFLOW_CAPACITY",
        runtime_policy_proxy().and_then(|policy| policy.websocket_dns_overflow_capacity),
        runtime_websocket_dns_resolve_overflow_capacity_default(worker_count, queue_capacity),
    )
}

pub(super) fn runtime_broker_ready_timeout_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS",
        runtime_policy_proxy().and_then(|policy| policy.broker_ready_timeout_ms),
        RUNTIME_BROKER_READY_TIMEOUT_MS,
    )
}

pub(super) fn runtime_broker_health_connect_timeout_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS",
        runtime_policy_proxy().and_then(|policy| policy.broker_health_connect_timeout_ms),
        RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS,
    )
}

pub(super) fn runtime_broker_health_read_timeout_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS",
        runtime_policy_proxy().and_then(|policy| policy.broker_health_read_timeout_ms),
        RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_websocket_previous_response_reuse_stale_ms() -> u64 {
    env::var("PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or_else(|| {
            runtime_policy_proxy()
                .and_then(|policy| policy.websocket_previous_response_reuse_stale_ms)
        })
        .unwrap_or(RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS)
}

pub(super) fn runtime_proxy_admission_wait_budget_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS",
        runtime_policy_proxy().and_then(|policy| policy.admission_wait_budget_ms),
        RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS,
    )
}

pub(super) fn runtime_proxy_pressure_admission_wait_budget_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS",
        runtime_policy_proxy().and_then(|policy| policy.pressure_admission_wait_budget_ms),
        RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS,
    )
}

pub(super) fn runtime_proxy_long_lived_queue_wait_budget_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
        runtime_policy_proxy().and_then(|policy| policy.long_lived_queue_wait_budget_ms),
        RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS,
    )
}

pub(super) fn runtime_proxy_pressure_long_lived_queue_wait_budget_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
        runtime_policy_proxy().and_then(|policy| policy.pressure_long_lived_queue_wait_budget_ms),
        RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS,
    )
}

pub(super) fn runtime_proxy_sync_probe_pressure_pause_ms() -> u64 {
    timeout_override_ms_with_policy(
        "PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS",
        runtime_policy_proxy().and_then(|policy| policy.sync_probe_pressure_pause_ms),
        RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS,
    )
}

pub(super) fn runtime_proxy_profile_inflight_soft_limit() -> usize {
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT",
        runtime_policy_proxy().and_then(|policy| policy.profile_inflight_soft_limit),
        RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT,
    )
}

pub(super) fn runtime_proxy_profile_inflight_hard_limit() -> usize {
    usize_override_with_policy(
        "PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT",
        runtime_policy_proxy().and_then(|policy| policy.profile_inflight_hard_limit),
        RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT,
    )
}

pub(super) fn runtime_proxy_responses_quota_critical_floor_percent() -> i64 {
    percent_override_with_policy(
        "PRODEX_RUNTIME_PROXY_RESPONSES_CRITICAL_FLOOR_PERCENT",
        runtime_policy_proxy().and_then(|policy| policy.responses_critical_floor_percent),
        2,
    )
    .clamp(1, 10)
}

pub(super) fn runtime_startup_sync_probe_warm_limit() -> usize {
    usize_override_with_policy(
        "PRODEX_RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT",
        runtime_policy_proxy().and_then(|policy| policy.startup_sync_probe_warm_limit),
        RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT,
    )
    .min(RUNTIME_STARTUP_PROBE_WARM_LIMIT)
}

#[cfg(test)]
#[path = "../tests/src/runtime_tuning.rs"]
mod tests;

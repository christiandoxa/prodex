use std::collections::BTreeMap;
use std::env;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Debug, Clone)]
struct RuntimeFaultBudget {
    raw_value: String,
    remaining: usize,
}

pub fn timeout_override_ms_with_policy(
    env_key: &str,
    policy_value: Option<u64>,
    default_ms: u64,
) -> u64 {
    env_positive_u64(env_key)
        .or(policy_value.filter(|value| *value > 0))
        .unwrap_or(default_ms)
}

pub fn percent_override_with_policy(
    env_key: &str,
    policy_value: Option<i64>,
    default_value: i64,
) -> i64 {
    env_positive_i64(env_key)
        .or(policy_value.filter(|value| *value > 0))
        .unwrap_or(default_value)
}

pub fn usize_override_with_policy(
    env_key: &str,
    policy_value: Option<usize>,
    default_value: usize,
) -> usize {
    env_usize(env_key, false)
        .or(policy_value.filter(|value| *value > 0))
        .unwrap_or(default_value)
}

pub fn usize_override_with_policy_allow_zero(
    env_key: &str,
    policy_value: Option<usize>,
    default_value: usize,
) -> usize {
    env_usize(env_key, true)
        .or(policy_value)
        .unwrap_or(default_value)
}

fn env_positive_u64(env_key: &str) -> Option<u64> {
    let value = env::var(env_key).ok()?;
    if value.is_empty() {
        panic!("{env_key} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        panic!("{env_key} must not contain whitespace");
    }
    let parsed = value
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{env_key} must be an unsigned integer"));
    if parsed == 0 {
        panic!("{env_key} must be greater than zero");
    }
    Some(parsed)
}

fn env_positive_i64(env_key: &str) -> Option<i64> {
    let value = env::var(env_key).ok()?;
    if value.is_empty() {
        panic!("{env_key} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        panic!("{env_key} must not contain whitespace");
    }
    let parsed = value
        .parse::<i64>()
        .unwrap_or_else(|_| panic!("{env_key} must be a positive integer"));
    if parsed <= 0 {
        panic!("{env_key} must be greater than zero");
    }
    Some(parsed)
}

fn env_usize(env_key: &str, allow_zero: bool) -> Option<usize> {
    let value = env::var(env_key).ok()?;
    if value.is_empty() {
        panic!("{env_key} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        panic!("{env_key} must not contain whitespace");
    }
    let parsed = value
        .parse::<usize>()
        .unwrap_or_else(|_| panic!("{env_key} must be an unsigned integer"));
    if !allow_zero && parsed == 0 {
        panic!("{env_key} must be greater than zero");
    }
    Some(parsed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningLaneLimits {
    pub responses: usize,
    pub compact: usize,
    pub websocket: usize,
    pub standard: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningSnapshot {
    pub worker_count: usize,
    pub long_lived_worker_count: usize,
    pub async_worker_count: usize,
    pub probe_refresh_worker_count: usize,
    pub long_lived_queue_capacity: usize,
    pub active_request_limit: usize,
    pub lane_limits: RuntimeTuningLaneLimits,
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
    pub http_connect_timeout_ms: u64,
    pub stream_idle_timeout_ms: u64,
    pub sse_lookahead_timeout_ms: u64,
    pub websocket_connect_timeout_ms: u64,
    pub websocket_happy_eyeballs_delay_ms: u64,
    pub websocket_precommit_progress_timeout_ms: u64,
    pub websocket_connect_worker_count: usize,
    pub websocket_connect_queue_capacity: usize,
    pub websocket_connect_overflow_capacity: usize,
    pub websocket_dns_worker_count: usize,
    pub websocket_dns_queue_capacity: usize,
    pub websocket_dns_overflow_capacity: usize,
    pub websocket_previous_response_reuse_stale_ms: u64,
    pub profile_inflight_soft_limit: usize,
    pub profile_inflight_hard_limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningPrecommitBudget {
    pub attempt_limit: usize,
    pub budget: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningSnapshotInput {
    pub worker_count: usize,
    pub long_lived_worker_count: usize,
    pub async_worker_count: usize,
    pub probe_refresh_worker_count: usize,
    pub long_lived_queue_capacity: usize,
    pub active_request_limit: usize,
    pub lane_limits: RuntimeTuningLaneLimits,
    pub precommit: RuntimeTuningPrecommitBudget,
    pub pressure_precommit: RuntimeTuningPrecommitBudget,
    pub continuation_precommit: RuntimeTuningPrecommitBudget,
    pub admission_wait_budget_ms: u64,
    pub pressure_admission_wait_budget_ms: u64,
    pub long_lived_queue_wait_budget_ms: u64,
    pub pressure_long_lived_queue_wait_budget_ms: u64,
    pub http_connect_timeout_ms: u64,
    pub stream_idle_timeout_ms: u64,
    pub sse_lookahead_timeout_ms: u64,
    pub websocket_connect_timeout_ms: u64,
    pub websocket_happy_eyeballs_delay_ms: u64,
    pub websocket_precommit_progress_timeout_ms: u64,
    pub websocket_connect_worker_count: usize,
    pub websocket_connect_queue_capacity: usize,
    pub websocket_connect_overflow_capacity: usize,
    pub websocket_dns_worker_count: usize,
    pub websocket_dns_queue_capacity: usize,
    pub websocket_dns_overflow_capacity: usize,
    pub websocket_previous_response_reuse_stale_ms: u64,
    pub profile_inflight_soft_limit: usize,
    pub profile_inflight_hard_limit: usize,
}

pub fn runtime_duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

pub fn runtime_tuning_snapshot_from_input(
    input: RuntimeTuningSnapshotInput,
) -> RuntimeTuningSnapshot {
    input.into_snapshot()
}

impl RuntimeTuningSnapshotInput {
    pub fn into_snapshot(self) -> RuntimeTuningSnapshot {
        let input = self;
        RuntimeTuningSnapshot {
            worker_count: input.worker_count,
            long_lived_worker_count: input.long_lived_worker_count,
            async_worker_count: input.async_worker_count,
            probe_refresh_worker_count: input.probe_refresh_worker_count,
            long_lived_queue_capacity: input.long_lived_queue_capacity,
            active_request_limit: input.active_request_limit,
            lane_limits: input.lane_limits,
            precommit_attempt_limit: input.precommit.attempt_limit,
            precommit_budget_ms: runtime_duration_ms(input.precommit.budget),
            pressure_precommit_attempt_limit: input.pressure_precommit.attempt_limit,
            pressure_precommit_budget_ms: runtime_duration_ms(input.pressure_precommit.budget),
            continuation_precommit_attempt_limit: input.continuation_precommit.attempt_limit,
            continuation_precommit_budget_ms: runtime_duration_ms(
                input.continuation_precommit.budget,
            ),
            admission_wait_budget_ms: input.admission_wait_budget_ms,
            pressure_admission_wait_budget_ms: input.pressure_admission_wait_budget_ms,
            long_lived_queue_wait_budget_ms: input.long_lived_queue_wait_budget_ms,
            pressure_long_lived_queue_wait_budget_ms: input
                .pressure_long_lived_queue_wait_budget_ms,
            http_connect_timeout_ms: input.http_connect_timeout_ms,
            stream_idle_timeout_ms: input.stream_idle_timeout_ms,
            sse_lookahead_timeout_ms: input.sse_lookahead_timeout_ms,
            websocket_connect_timeout_ms: input.websocket_connect_timeout_ms,
            websocket_happy_eyeballs_delay_ms: input.websocket_happy_eyeballs_delay_ms,
            websocket_precommit_progress_timeout_ms: input.websocket_precommit_progress_timeout_ms,
            websocket_connect_worker_count: input.websocket_connect_worker_count,
            websocket_connect_queue_capacity: input.websocket_connect_queue_capacity,
            websocket_connect_overflow_capacity: input.websocket_connect_overflow_capacity,
            websocket_dns_worker_count: input.websocket_dns_worker_count,
            websocket_dns_queue_capacity: input.websocket_dns_queue_capacity,
            websocket_dns_overflow_capacity: input.websocket_dns_overflow_capacity,
            websocket_previous_response_reuse_stale_ms: input
                .websocket_previous_response_reuse_stale_ms,
            profile_inflight_soft_limit: input.profile_inflight_soft_limit,
            profile_inflight_hard_limit: input.profile_inflight_hard_limit,
        }
    }
}

pub fn runtime_proxy_worker_count_default(parallelism: usize) -> usize {
    parallelism.clamp(4, 12)
}

pub fn runtime_proxy_long_lived_worker_count_default(parallelism: usize) -> usize {
    parallelism.saturating_mul(2).clamp(8, 24)
}

pub fn runtime_probe_refresh_worker_count_default(parallelism: usize) -> usize {
    parallelism.clamp(2, 4)
}

pub fn runtime_proxy_async_worker_count_default(parallelism: usize) -> usize {
    parallelism.clamp(2, 4)
}

pub fn runtime_proxy_long_lived_queue_capacity_default(worker_count: usize) -> usize {
    worker_count.saturating_mul(8).clamp(128, 1024)
}

pub fn runtime_proxy_active_request_limit_default(
    worker_count: usize,
    long_lived_worker_count: usize,
) -> usize {
    worker_count
        .saturating_add(long_lived_worker_count.saturating_mul(3))
        .clamp(64, 512)
}

pub fn runtime_proxy_log_queue_capacity_default(parallelism: usize) -> usize {
    parallelism.saturating_mul(256).clamp(1024, 8192)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeProxyLaneLimitOverrides {
    pub responses: Option<usize>,
    pub compact: Option<usize>,
    pub websocket: Option<usize>,
    pub standard: Option<usize>,
}

pub fn runtime_proxy_lane_limits_from_overrides(
    global_limit: usize,
    worker_count: usize,
    long_lived_worker_count: usize,
    overrides: RuntimeProxyLaneLimitOverrides,
) -> RuntimeTuningLaneLimits {
    let global_limit = global_limit.max(1);
    RuntimeTuningLaneLimits {
        responses: overrides
            .responses
            .filter(|value| *value > 0)
            .unwrap_or_else(|| (global_limit.saturating_mul(3) / 4).clamp(4, global_limit))
            .min(global_limit)
            .max(1),
        compact: overrides
            .compact
            .filter(|value| *value > 0)
            .unwrap_or_else(|| (global_limit / 4).clamp(2, 6).min(global_limit))
            .min(global_limit)
            .max(1),
        websocket: overrides
            .websocket
            .filter(|value| *value > 0)
            .unwrap_or_else(|| long_lived_worker_count.clamp(2, global_limit))
            .min(global_limit)
            .max(1),
        standard: overrides
            .standard
            .filter(|value| *value > 0)
            .unwrap_or_else(|| {
                worker_count
                    .saturating_mul(2)
                    .clamp(8, 24)
                    .min(global_limit)
            })
            .min(global_limit)
            .max(1),
    }
}

pub fn runtime_websocket_tcp_connect_worker_count_default(parallelism: usize) -> usize {
    parallelism.clamp(4, 16)
}

pub fn runtime_websocket_tcp_connect_queue_capacity_default(worker_count: usize) -> usize {
    worker_count.saturating_mul(8).clamp(32, 128)
}

pub fn runtime_websocket_tcp_connect_overflow_capacity_default(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    queue_capacity
        .saturating_mul(4)
        .max(worker_count)
        .clamp(32, 512)
}

pub fn runtime_websocket_dns_resolve_worker_count_default(parallelism: usize) -> usize {
    parallelism.clamp(2, 8)
}

pub fn runtime_websocket_dns_resolve_queue_capacity_default(worker_count: usize) -> usize {
    worker_count.saturating_mul(4).clamp(16, 64)
}

pub fn runtime_websocket_dns_resolve_overflow_capacity_default(
    worker_count: usize,
    queue_capacity: usize,
) -> usize {
    queue_capacity
        .saturating_mul(2)
        .max(worker_count)
        .clamp(16, 128)
}

fn runtime_fault_counters() -> &'static Mutex<BTreeMap<String, RuntimeFaultBudget>> {
    static COUNTERS: OnceLock<Mutex<BTreeMap<String, RuntimeFaultBudget>>> = OnceLock::new();
    COUNTERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn runtime_take_fault_injection(env_key: &str) -> bool {
    let raw_value = env::var(env_key).ok().unwrap_or_default();
    let configured = raw_value.parse::<usize>().unwrap_or(0);
    runtime_take_fault_injection_budget(env_key, configured)
}

pub fn runtime_take_fault_injection_budget(env_key: &str, configured: usize) -> bool {
    let raw_value = configured.to_string();
    if configured == 0 {
        if let Ok(mut counters) = runtime_fault_counters().lock() {
            counters.remove(env_key);
        }
        return false;
    }

    let Ok(mut counters) = runtime_fault_counters().lock() else {
        return false;
    };
    let counter = counters
        .entry(env_key.to_string())
        .or_insert_with(|| RuntimeFaultBudget {
            raw_value: raw_value.clone(),
            remaining: configured,
        });
    if counter.raw_value != raw_value {
        counter.raw_value = raw_value;
        counter.remaining = configured;
    }
    if counter.remaining == 0 {
        return false;
    }
    counter.remaining -= 1;
    true
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;

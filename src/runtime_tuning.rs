use super::*;

#[derive(Debug, Clone)]
struct RuntimeFaultBudget {
    raw_value: String,
    remaining: usize,
}

pub(super) fn timeout_override_ms_with_policy(
    env_key: &str,
    policy_value: Option<u64>,
    default_ms: u64,
) -> u64 {
    env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .or(policy_value.filter(|value| *value > 0))
        .unwrap_or(default_ms)
}

fn percent_override_with_policy(
    env_key: &str,
    policy_value: Option<i64>,
    default_value: i64,
) -> i64 {
    env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .or(policy_value.filter(|value| *value > 0))
        .unwrap_or(default_value)
}

pub(super) fn usize_override_with_policy(
    env_key: &str,
    policy_value: Option<usize>,
    default_value: usize,
) -> usize {
    env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .or(policy_value.filter(|value| *value > 0))
        .unwrap_or(default_value)
}

fn runtime_fault_counters() -> &'static Mutex<BTreeMap<String, RuntimeFaultBudget>> {
    static COUNTERS: OnceLock<Mutex<BTreeMap<String, RuntimeFaultBudget>>> = OnceLock::new();
    COUNTERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(super) fn runtime_take_fault_injection(env_key: &str) -> bool {
    let raw_value = env::var(env_key).ok().unwrap_or_default();
    let configured = raw_value.parse::<usize>().unwrap_or(0);
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

pub(super) fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

use super::*;

#[derive(Debug, Clone)]
struct RuntimeFaultBudget {
    raw_value: String,
    remaining: usize,
}

fn timeout_override_ms(env_key: &str, default_ms: u64) -> u64 {
    env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_ms)
}

pub(super) fn usize_override(env_key: &str, default_value: usize) -> usize {
    env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
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
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS",
        RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_stream_idle_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS",
        RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_sse_lookahead_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS",
        RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_websocket_connect_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS",
        RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_websocket_precommit_progress_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS",
        RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS,
    )
}

pub(super) fn runtime_broker_ready_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS",
        RUNTIME_BROKER_READY_TIMEOUT_MS,
    )
}

pub(super) fn runtime_broker_health_connect_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS",
        RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS,
    )
}

pub(super) fn runtime_broker_health_read_timeout_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS",
        RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS,
    )
}

pub(super) fn runtime_proxy_websocket_previous_response_reuse_stale_ms() -> u64 {
    env::var("PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS)
}

pub(super) fn runtime_proxy_admission_wait_budget_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS",
        RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS,
    )
}

pub(super) fn runtime_proxy_pressure_admission_wait_budget_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS",
        RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS,
    )
}

pub(super) fn runtime_proxy_admission_wait_poll_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_POLL_MS",
        RUNTIME_PROXY_ADMISSION_WAIT_POLL_MS,
    )
}

pub(super) fn runtime_proxy_long_lived_queue_wait_budget_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
        RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS,
    )
}

pub(super) fn runtime_proxy_pressure_long_lived_queue_wait_budget_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
        RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS,
    )
}

pub(super) fn runtime_proxy_long_lived_queue_wait_poll_ms() -> u64 {
    timeout_override_ms(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_POLL_MS",
        RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_POLL_MS,
    )
}

pub(super) fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

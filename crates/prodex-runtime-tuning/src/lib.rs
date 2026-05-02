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
    env::var(env_key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .or(policy_value.filter(|value| *value > 0))
        .unwrap_or(default_ms)
}

pub fn percent_override_with_policy(
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

pub fn usize_override_with_policy(
    env_key: &str,
    policy_value: Option<usize>,
    default_value: usize,
) -> usize {
    env::var(env_key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .or(policy_value.filter(|value| *value > 0))
        .unwrap_or(default_value)
}

pub fn usize_override_with_policy_allow_zero(
    env_key: &str,
    policy_value: Option<usize>,
    default_value: usize,
) -> usize {
    env::var(env_key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .or(policy_value)
        .unwrap_or(default_value)
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

pub fn runtime_duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
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
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        key: String,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &str, value: &str) -> Self {
            let previous = env::var_os(key);
            unsafe {
                env::set_var(key, value);
            }
            Self {
                key: key.to_string(),
                previous,
            }
        }

        fn unset(key: &str) -> Self {
            let previous = env::var_os(key);
            unsafe {
                env::remove_var(key);
            }
            Self {
                key: key.to_string(),
                previous,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                if let Some(previous) = &self.previous {
                    env::set_var(&self.key, previous);
                } else {
                    env::remove_var(&self.key);
                }
            }
        }
    }

    #[test]
    fn usize_override_ignores_zero_env_and_prefers_positive_env() {
        let _lock = env_lock().lock().unwrap();
        let _guard = EnvGuard::set("PRODEX_TEST_TUNING_USIZE", "0");
        assert_eq!(
            usize_override_with_policy("PRODEX_TEST_TUNING_USIZE", Some(7), 3),
            7
        );

        let _guard = EnvGuard::set("PRODEX_TEST_TUNING_USIZE", "11");
        assert_eq!(
            usize_override_with_policy("PRODEX_TEST_TUNING_USIZE", Some(7), 3),
            11
        );
    }

    #[test]
    fn fault_injection_budget_resets_when_env_changes() {
        let _lock = env_lock().lock().unwrap();
        let _unset = EnvGuard::unset("PRODEX_TEST_TUNING_FAULT");
        let _guard = EnvGuard::set("PRODEX_TEST_TUNING_FAULT", "2");
        assert!(runtime_take_fault_injection("PRODEX_TEST_TUNING_FAULT"));
        assert!(runtime_take_fault_injection("PRODEX_TEST_TUNING_FAULT"));
        assert!(!runtime_take_fault_injection("PRODEX_TEST_TUNING_FAULT"));

        let _guard = EnvGuard::set("PRODEX_TEST_TUNING_FAULT", "1");
        assert!(runtime_take_fault_injection("PRODEX_TEST_TUNING_FAULT"));
        assert!(!runtime_take_fault_injection("PRODEX_TEST_TUNING_FAULT"));
    }

    #[test]
    fn websocket_tcp_connect_defaults_are_bounded() {
        assert_eq!(runtime_websocket_tcp_connect_worker_count_default(1), 4);
        assert_eq!(runtime_websocket_tcp_connect_worker_count_default(8), 8);
        assert_eq!(runtime_websocket_tcp_connect_worker_count_default(64), 16);

        assert_eq!(runtime_websocket_tcp_connect_queue_capacity_default(1), 32);
        assert_eq!(runtime_websocket_tcp_connect_queue_capacity_default(8), 64);
        assert_eq!(
            runtime_websocket_tcp_connect_queue_capacity_default(64),
            128
        );

        assert_eq!(
            runtime_websocket_tcp_connect_overflow_capacity_default(2, 8),
            32
        );
        assert_eq!(
            runtime_websocket_tcp_connect_overflow_capacity_default(16, 128),
            512
        );
    }

    #[test]
    fn websocket_dns_defaults_are_bounded() {
        assert_eq!(runtime_websocket_dns_resolve_worker_count_default(1), 2);
        assert_eq!(runtime_websocket_dns_resolve_worker_count_default(6), 6);
        assert_eq!(runtime_websocket_dns_resolve_worker_count_default(32), 8);

        assert_eq!(runtime_websocket_dns_resolve_queue_capacity_default(1), 16);
        assert_eq!(runtime_websocket_dns_resolve_queue_capacity_default(8), 32);
        assert_eq!(runtime_websocket_dns_resolve_queue_capacity_default(64), 64);

        assert_eq!(
            runtime_websocket_dns_resolve_overflow_capacity_default(2, 8),
            16
        );
        assert_eq!(
            runtime_websocket_dns_resolve_overflow_capacity_default(16, 64),
            128
        );
    }
}

use crate::{
    RuntimeProxyLaneLimitOverrides, RuntimeTuningLaneLimits, RuntimeTuningPrecommitBudget,
    RuntimeTuningSnapshotInput, runtime_probe_refresh_worker_count_default,
    runtime_proxy_active_request_limit_default, runtime_proxy_async_worker_count_default,
    runtime_proxy_lane_limits_from_overrides, runtime_proxy_log_queue_capacity_default,
    runtime_proxy_long_lived_queue_capacity_default, runtime_proxy_long_lived_worker_count_default,
    runtime_proxy_worker_count_default, runtime_take_fault_injection,
    runtime_tuning_snapshot_from_input, runtime_websocket_dns_resolve_overflow_capacity_default,
    runtime_websocket_dns_resolve_queue_capacity_default,
    runtime_websocket_dns_resolve_worker_count_default,
    runtime_websocket_tcp_connect_overflow_capacity_default,
    runtime_websocket_tcp_connect_queue_capacity_default,
    runtime_websocket_tcp_connect_worker_count_default, usize_override_with_policy,
};
use std::env;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

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

#[test]
fn runtime_tuning_snapshot_from_input_converts_duration_budgets() {
    let snapshot = runtime_tuning_snapshot_from_input(RuntimeTuningSnapshotInput {
        worker_count: 2,
        long_lived_worker_count: 3,
        async_worker_count: 4,
        probe_refresh_worker_count: 5,
        long_lived_queue_capacity: 6,
        active_request_limit: 7,
        lane_limits: RuntimeTuningLaneLimits {
            responses: 8,
            compact: 9,
            websocket: 10,
            standard: 11,
        },
        precommit: RuntimeTuningPrecommitBudget {
            attempt_limit: 12,
            budget: Duration::from_millis(13),
        },
        pressure_precommit: RuntimeTuningPrecommitBudget {
            attempt_limit: 14,
            budget: Duration::from_millis(15),
        },
        continuation_precommit: RuntimeTuningPrecommitBudget {
            attempt_limit: 16,
            budget: Duration::from_millis(17),
        },
        admission_wait_budget_ms: 18,
        pressure_admission_wait_budget_ms: 19,
        long_lived_queue_wait_budget_ms: 20,
        pressure_long_lived_queue_wait_budget_ms: 21,
        http_connect_timeout_ms: 22,
        stream_idle_timeout_ms: 23,
        sse_lookahead_timeout_ms: 24,
        websocket_connect_timeout_ms: 25,
        websocket_happy_eyeballs_delay_ms: 26,
        websocket_precommit_progress_timeout_ms: 27,
        websocket_connect_worker_count: 28,
        websocket_connect_queue_capacity: 29,
        websocket_connect_overflow_capacity: 30,
        websocket_dns_worker_count: 31,
        websocket_dns_queue_capacity: 32,
        websocket_dns_overflow_capacity: 33,
        websocket_previous_response_reuse_stale_ms: 34,
        profile_inflight_soft_limit: 35,
        profile_inflight_hard_limit: 36,
    });

    assert_eq!(snapshot.precommit_attempt_limit, 12);
    assert_eq!(snapshot.precommit_budget_ms, 13);
    assert_eq!(snapshot.pressure_precommit_attempt_limit, 14);
    assert_eq!(snapshot.pressure_precommit_budget_ms, 15);
    assert_eq!(snapshot.continuation_precommit_attempt_limit, 16);
    assert_eq!(snapshot.continuation_precommit_budget_ms, 17);
    assert_eq!(snapshot.websocket_dns_overflow_capacity, 33);
    assert_eq!(snapshot.profile_inflight_hard_limit, 36);
}

#[test]
fn runtime_proxy_worker_defaults_are_bounded() {
    assert_eq!(runtime_proxy_worker_count_default(1), 4);
    assert_eq!(runtime_proxy_worker_count_default(8), 8);
    assert_eq!(runtime_proxy_worker_count_default(64), 12);

    assert_eq!(runtime_proxy_long_lived_worker_count_default(1), 8);
    assert_eq!(runtime_proxy_long_lived_worker_count_default(8), 16);
    assert_eq!(runtime_proxy_long_lived_worker_count_default(64), 24);

    assert_eq!(runtime_probe_refresh_worker_count_default(1), 2);
    assert_eq!(runtime_probe_refresh_worker_count_default(3), 3);
    assert_eq!(runtime_probe_refresh_worker_count_default(64), 4);

    assert_eq!(runtime_proxy_async_worker_count_default(1), 2);
    assert_eq!(runtime_proxy_async_worker_count_default(3), 3);
    assert_eq!(runtime_proxy_async_worker_count_default(64), 4);
}

#[test]
fn runtime_proxy_capacity_defaults_are_bounded() {
    assert_eq!(runtime_proxy_long_lived_queue_capacity_default(1), 128);
    assert_eq!(runtime_proxy_long_lived_queue_capacity_default(32), 256);
    assert_eq!(runtime_proxy_long_lived_queue_capacity_default(256), 1024);

    assert_eq!(runtime_proxy_active_request_limit_default(1, 1), 64);
    assert_eq!(runtime_proxy_active_request_limit_default(8, 32), 104);
    assert_eq!(runtime_proxy_active_request_limit_default(128, 256), 512);

    assert_eq!(runtime_proxy_log_queue_capacity_default(1), 1024);
    assert_eq!(runtime_proxy_log_queue_capacity_default(8), 2048);
    assert_eq!(runtime_proxy_log_queue_capacity_default(64), 8192);
}

#[test]
fn runtime_proxy_lane_limits_keep_defaults_and_clamp_overrides() {
    assert_eq!(
        runtime_proxy_lane_limits_from_overrides(
            64,
            12,
            16,
            RuntimeProxyLaneLimitOverrides::default(),
        ),
        RuntimeTuningLaneLimits {
            responses: 48,
            compact: 6,
            websocket: 16,
            standard: 6,
        }
    );

    assert_eq!(
        runtime_proxy_lane_limits_from_overrides(
            8,
            1,
            99,
            RuntimeProxyLaneLimitOverrides {
                responses: Some(0),
                compact: Some(99),
                websocket: Some(3),
                standard: Some(0),
            },
        ),
        RuntimeTuningLaneLimits {
            responses: 6,
            compact: 8,
            websocket: 3,
            standard: 2,
        }
    );
}

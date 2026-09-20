use std::sync::atomic::{AtomicI64, Ordering};

pub(super) use prodex_runtime_tuning::{
    RuntimeProxyLaneLimitOverrides, RuntimeTuningDefaults, RuntimeTuningLaneLimits,
    RuntimeTuningPrecommitBudget, RuntimeTuningSnapshot, RuntimeTuningSnapshotInput,
    runtime_proxy_active_request_limit_default, runtime_proxy_lane_limits_from_overrides,
    runtime_proxy_long_lived_queue_capacity_default, runtime_take_fault_injection,
    runtime_take_fault_injection_budget, runtime_tuning_defaults,
    runtime_websocket_dns_resolve_overflow_capacity_default,
    runtime_websocket_dns_resolve_queue_capacity_default,
    runtime_websocket_tcp_connect_overflow_capacity_default,
    runtime_websocket_tcp_connect_queue_capacity_default,
};

const RUNTIME_PROXY_DEFAULT_MAX_REQUEST_BODY_BYTES: u64 = 64 * 1024 * 1024;

mod config;
mod environment;
mod types;
use environment::{RuntimeConfigEnvironment, RuntimeConfigParser};
pub(crate) use types::{
    ConfigError, ConfigErrors, RuntimeConfig, RuntimeGeminiConfig, RuntimeWebsocketEnvironment,
};

pub(crate) fn collect_runtime_tuning_snapshot(config: &RuntimeConfig) -> RuntimeTuningSnapshot {
    config.tuning
}

impl RuntimeConfig {
    pub(crate) fn from_env_policy_and_cli(
        paths: &prodex_core::AppPaths,
    ) -> Result<Self, ConfigErrors> {
        let environment = RuntimeConfigEnvironment::read_process();
        Self::from_environment(paths, environment)
    }

    pub(crate) fn force_http_response_transport(&self) -> bool {
        self.governance.inspection != prodex_config::GovernanceRolloutMode::Off
    }

    #[cfg(test)]
    pub(crate) fn offline_default(paths: &prodex_core::AppPaths) -> Result<Self, ConfigErrors> {
        Self::from_environment(paths, RuntimeConfigEnvironment::default())
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn compatibility_current() -> Self {
        let paths = prodex_core::AppPaths::discover()
            .unwrap_or_else(|error| panic!("failed to discover runtime configuration: {error}"));
        Self::from_env_policy_and_cli(&paths).unwrap_or_else(|errors| panic!("{errors}"))
    }
}

pub(super) fn runtime_proxy_responses_quota_critical_floor_percent() -> i64 {
    let configured = RUNTIME_RESPONSES_QUOTA_CRITICAL_FLOOR_PERCENT.load(Ordering::Relaxed);
    if configured > 0 {
        return configured;
    }

    // Compatibility fallback for pure helpers invoked before proxy startup. Production proxy
    // startup always publishes the validated value before binding a listener.
    #[cfg(test)]
    return RuntimeConfig::compatibility_current().responses_quota_critical_floor_percent;

    #[cfg(not(test))]
    1
}

static RUNTIME_RESPONSES_QUOTA_CRITICAL_FLOOR_PERCENT: AtomicI64 = AtomicI64::new(0);

#[cfg(test)]
mod test_compat;
#[cfg(test)]
pub(crate) use test_compat::{
    runtime_proxy_profile_inflight_hard_limit, runtime_proxy_profile_inflight_soft_limit,
    runtime_proxy_stream_idle_timeout_ms, runtime_proxy_websocket_precommit_progress_timeout_ms,
};

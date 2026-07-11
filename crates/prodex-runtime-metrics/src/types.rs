use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerLaneMetrics {
    pub active: u64,
    pub limit: u64,
    pub admissions_total: u64,
    pub releases_total: u64,
    pub global_limit_rejections_total: u64,
    pub lane_limit_rejections_total: u64,
    pub release_underflows_total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerTrafficMetrics {
    pub responses: RuntimeBrokerLaneMetrics,
    pub compact: RuntimeBrokerLaneMetrics,
    pub websocket: RuntimeBrokerLaneMetrics,
    pub standard: RuntimeBrokerLaneMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuationMetrics {
    pub response_bindings: u64,
    pub turn_state_bindings: u64,
    pub session_id_bindings: u64,
    pub warm: u64,
    pub verified: u64,
    pub suspect: u64,
    pub dead: u64,
    pub failure_counts: RuntimeBrokerContinuationSignalMetrics,
    pub not_found_streaks: RuntimeBrokerContinuationSignalMetrics,
    pub stale_verified_bindings: RuntimeBrokerContinuationSignalMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuationSignalMetrics {
    pub response: u64,
    pub turn_state: u64,
    pub session_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuityFailureReasonMetrics {
    pub chain_retried_owner: BTreeMap<String, u64>,
    pub chain_dead_upstream_confirmed: BTreeMap<String, u64>,
    pub stale_continuation: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerRouteContinuityMetrics {
    pub responses: u64,
    pub compact: u64,
    pub websocket: u64,
    pub standard: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerPreviousResponseContinuityMetrics {
    pub negative_cache_entries: RuntimeBrokerRouteContinuityMetrics,
    pub negative_cache_failures: RuntimeBrokerRouteContinuityMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerSnapshot {
    pub broker_key: String,
    pub listen_addr: String,
    pub pid: u32,
    pub started_at_unix_seconds: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub persistence_role: String,
    pub prodex_version: Option<String>,
    pub executable_path: Option<String>,
    pub executable_sha256: Option<String>,
    pub active_requests: u64,
    pub active_request_limit: u64,
    pub local_overload_backoff_remaining_seconds: u64,
    pub runtime_state_lock_wait: RuntimeBrokerStateLockWaitMetrics,
    pub traffic: RuntimeBrokerTrafficMetrics,
    pub profile_inflight: BTreeMap<String, u64>,
    pub active_request_release_underflows_total: u64,
    pub profile_inflight_admissions_total: u64,
    pub profile_inflight_releases_total: u64,
    pub profile_inflight_release_underflows_total: u64,
    pub retry_backoffs: u64,
    pub transport_backoffs: u64,
    pub route_circuits: u64,
    pub degraded_profiles: u64,
    pub degraded_routes: u64,
    pub continuations: RuntimeBrokerContinuationMetrics,
    pub previous_response_continuity: RuntimeBrokerPreviousResponseContinuityMetrics,
    pub continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeBrokerStateLockWaitMetrics {
    pub wait_total_ns: u64,
    pub wait_count: u64,
    pub wait_max_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrometheusTextOptions {
    pub include_help: bool,
}

impl Default for PrometheusTextOptions {
    fn default() -> Self {
        Self { include_help: true }
    }
}

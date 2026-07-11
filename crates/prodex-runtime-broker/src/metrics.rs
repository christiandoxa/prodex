use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerLaneMetrics {
    pub active: usize,
    pub limit: usize,
    pub admissions_total: u64,
    #[serde(default)]
    pub releases_total: u64,
    pub global_limit_rejections_total: u64,
    pub lane_limit_rejections_total: u64,
    #[serde(default)]
    pub release_underflows_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerTrafficMetrics {
    pub responses: RuntimeBrokerLaneMetrics,
    pub compact: RuntimeBrokerLaneMetrics,
    pub websocket: RuntimeBrokerLaneMetrics,
    pub standard: RuntimeBrokerLaneMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuationSignalMetrics {
    pub response: usize,
    pub turn_state: usize,
    pub session_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuityFailureReasonMetrics {
    #[serde(default)]
    pub chain_retried_owner: BTreeMap<String, usize>,
    #[serde(default)]
    pub chain_dead_upstream_confirmed: BTreeMap<String, usize>,
    #[serde(default)]
    pub stale_continuation: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerRouteContinuityMetrics {
    pub responses: usize,
    pub compact: usize,
    pub websocket: usize,
    pub standard: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerPreviousResponseContinuityMetrics {
    pub negative_cache_entries: RuntimeBrokerRouteContinuityMetrics,
    pub negative_cache_failures: RuntimeBrokerRouteContinuityMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerContinuationMetrics {
    pub response_bindings: usize,
    pub turn_state_bindings: usize,
    pub session_id_bindings: usize,
    pub warm: usize,
    pub verified: usize,
    pub suspect: usize,
    pub dead: usize,
    #[serde(default)]
    pub failure_counts: RuntimeBrokerContinuationSignalMetrics,
    #[serde(default)]
    pub not_found_streaks: RuntimeBrokerContinuationSignalMetrics,
    #[serde(default)]
    pub stale_verified_bindings: RuntimeBrokerContinuationSignalMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerMetrics {
    pub health: RuntimeBrokerHealth,
    pub active_request_limit: usize,
    pub local_overload_backoff_remaining_seconds: u64,
    #[serde(default)]
    pub runtime_state_lock_wait: RuntimeStateLockWaitMetrics,
    #[serde(default)]
    pub admission_wait: RuntimeWaitDurationMetrics,
    #[serde(default)]
    pub long_lived_queue_wait: RuntimeWaitDurationMetrics,
    pub traffic: RuntimeBrokerTrafficMetrics,
    pub profile_inflight: BTreeMap<String, usize>,
    #[serde(default)]
    pub active_request_release_underflows_total: u64,
    #[serde(default)]
    pub profile_inflight_admissions_total: u64,
    #[serde(default)]
    pub profile_inflight_releases_total: u64,
    #[serde(default)]
    pub profile_inflight_release_underflows_total: u64,
    pub retry_backoffs: usize,
    pub transport_backoffs: usize,
    pub route_circuits: usize,
    pub degraded_profiles: usize,
    pub degraded_routes: usize,
    pub continuations: RuntimeBrokerContinuationMetrics,
    #[serde(default)]
    pub previous_response_continuity: RuntimeBrokerPreviousResponseContinuityMetrics,
    #[serde(default)]
    pub continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics,
}

#[derive(Debug, Clone)]
pub struct RuntimeBrokerMetricsSnapshotInput<'a> {
    pub metadata: &'a RuntimeBrokerMetadata,
    pub pid: u32,
    pub active_requests: usize,
    pub persistence_owner: bool,
    pub active_request_limit: usize,
    pub local_overload_backoff_remaining_seconds: u64,
    pub runtime_state_lock_wait: RuntimeStateLockWaitMetrics,
    pub admission_wait: RuntimeWaitDurationMetrics,
    pub long_lived_queue_wait: RuntimeWaitDurationMetrics,
    pub traffic: RuntimeBrokerTrafficMetrics,
    pub profile_inflight: &'a BTreeMap<String, usize>,
    pub profile_retry_backoff_until: &'a BTreeMap<String, i64>,
    pub profile_transport_backoff_until: &'a BTreeMap<String, i64>,
    pub profile_route_circuit_open_until: &'a BTreeMap<String, i64>,
    pub profile_health: &'a BTreeMap<String, RuntimeProfileHealth>,
    pub continuation_statuses: &'a RuntimeContinuationStatuses,
    pub continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics,
    pub now: i64,
    pub health_decay_seconds: i64,
    pub stale_verified_seconds: i64,
    pub previous_response_negative_cache_seconds: i64,
}

pub fn runtime_broker_metrics_from_snapshot_input(
    input: RuntimeBrokerMetricsSnapshotInput<'_>,
) -> RuntimeBrokerMetrics {
    let degraded_health = runtime_broker_degraded_health_metrics(
        input.profile_health,
        input.now,
        input.health_decay_seconds,
    );

    RuntimeBrokerMetrics {
        health: RuntimeBrokerHealth::from_metadata(
            input.metadata,
            input.pid,
            input.active_requests,
            input.persistence_owner,
        ),
        active_request_limit: input.active_request_limit,
        local_overload_backoff_remaining_seconds: input.local_overload_backoff_remaining_seconds,
        runtime_state_lock_wait: input.runtime_state_lock_wait,
        admission_wait: input.admission_wait,
        long_lived_queue_wait: input.long_lived_queue_wait,
        traffic: input.traffic,
        profile_inflight: input.profile_inflight.clone(),
        active_request_release_underflows_total: 0,
        profile_inflight_admissions_total: 0,
        profile_inflight_releases_total: 0,
        profile_inflight_release_underflows_total: 0,
        retry_backoffs: input
            .profile_retry_backoff_until
            .values()
            .filter(|until| **until > input.now)
            .count(),
        transport_backoffs: input
            .profile_transport_backoff_until
            .values()
            .filter(|until| **until > input.now)
            .count(),
        route_circuits: input
            .profile_route_circuit_open_until
            .values()
            .filter(|until| **until > input.now)
            .count(),
        degraded_profiles: degraded_health.profiles,
        degraded_routes: degraded_health.routes,
        continuations: runtime_broker_continuation_metrics(
            input.continuation_statuses,
            input.now,
            input.stale_verified_seconds,
        ),
        previous_response_continuity: runtime_broker_previous_response_continuity_metrics(
            input.profile_health,
            input.now,
            input.previous_response_negative_cache_seconds,
        ),
        continuity_failure_reasons: input.continuity_failure_reasons,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBrokerMetricsGuardCounters {
    pub active_request_release_underflows_total: u64,
    pub profile_inflight_admissions_total: u64,
    pub profile_inflight_releases_total: u64,
    pub profile_inflight_release_underflows_total: u64,
}

impl RuntimeBrokerMetrics {
    pub fn with_guard_counters(mut self, counters: RuntimeBrokerMetricsGuardCounters) -> Self {
        self.active_request_release_underflows_total =
            counters.active_request_release_underflows_total;
        self.profile_inflight_admissions_total = counters.profile_inflight_admissions_total;
        self.profile_inflight_releases_total = counters.profile_inflight_releases_total;
        self.profile_inflight_release_underflows_total =
            counters.profile_inflight_release_underflows_total;
        self
    }
}

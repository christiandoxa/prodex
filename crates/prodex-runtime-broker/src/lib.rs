//! Runtime broker registry, health, and metrics transfer models.
//!
//! This crate intentionally stays side-effect-free. The binary crate still owns
//! broker leases, proxy handles, and registry file I/O.

use prodex_runtime_state::RuntimeStateLockWaitMetrics;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerRegistry {
    pub pid: u32,
    pub listen_addr: String,
    pub started_at: i64,
    pub upstream_base_url: String,
    pub include_code_review: bool,
    #[serde(default)]
    pub upstream_no_proxy: bool,
    pub current_profile: String,
    pub instance_token: String,
    pub admin_token: String,
    #[serde(default)]
    pub prodex_version: Option<String>,
    #[serde(default)]
    pub executable_path: Option<String>,
    #[serde(default)]
    pub executable_sha256: Option<String>,
    #[serde(default)]
    pub openai_mount_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerHealth {
    pub pid: u32,
    pub started_at: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub active_requests: usize,
    pub instance_token: String,
    pub persistence_role: String,
    #[serde(default)]
    pub prodex_version: Option<String>,
    #[serde(default)]
    pub executable_path: Option<String>,
    #[serde(default)]
    pub executable_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerLaneMetrics {
    pub active: usize,
    pub limit: usize,
    pub admissions_total: u64,
    pub global_limit_rejections_total: u64,
    pub lane_limit_rejections_total: u64,
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
    pub traffic: RuntimeBrokerTrafficMetrics,
    pub profile_inflight: BTreeMap<String, usize>,
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

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeBrokerObservation {
    pub broker_key: String,
    pub listen_addr: String,
    pub metrics: RuntimeBrokerMetrics,
}

#[derive(Debug, Clone)]
pub struct RuntimeBrokerMetadata {
    pub broker_key: String,
    pub listen_addr: String,
    pub started_at: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
    pub instance_token: String,
    pub admin_token: String,
    pub prodex_version: Option<String>,
    pub executable_path: Option<String>,
    pub executable_sha256: Option<String>,
}

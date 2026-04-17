use crate::{
    AppPaths, AppState, AuthSummary, ResponseProfileBinding, RuntimeProxyLaneAdmission,
    RuntimeQuotaWindowStatus, UsageAuth, UsageResponse, deserialize_null_default, secret_store,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime as TokioRuntime;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeRotationProxyShared {
    pub(crate) async_client: reqwest::Client,
    pub(crate) async_runtime: Arc<TokioRuntime>,
    pub(crate) runtime: Arc<Mutex<RuntimeRotationState>>,
    pub(crate) log_path: PathBuf,
    pub(crate) request_sequence: Arc<AtomicU64>,
    pub(crate) state_save_revision: Arc<AtomicU64>,
    pub(crate) local_overload_backoff_until: Arc<AtomicU64>,
    pub(crate) active_request_count: Arc<AtomicUsize>,
    pub(crate) active_request_limit: usize,
    pub(crate) lane_admission: RuntimeProxyLaneAdmission,
}

#[derive(Debug)]
pub(crate) struct StateFileLock {
    pub(crate) file: fs::File,
}

impl Drop for StateFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeRotationState {
    pub(crate) paths: AppPaths,
    pub(crate) state: AppState,
    pub(crate) upstream_base_url: String,
    pub(crate) include_code_review: bool,
    pub(crate) current_profile: String,
    pub(crate) profile_usage_auth: BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    pub(crate) turn_state_bindings: BTreeMap<String, ResponseProfileBinding>,
    pub(crate) session_id_bindings: BTreeMap<String, ResponseProfileBinding>,
    pub(crate) continuation_statuses: RuntimeContinuationStatuses,
    pub(crate) profile_probe_cache: BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    pub(crate) profile_usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    pub(crate) profile_retry_backoff_until: BTreeMap<String, i64>,
    pub(crate) profile_transport_backoff_until: BTreeMap<String, i64>,
    pub(crate) profile_route_circuit_open_until: BTreeMap<String, i64>,
    pub(crate) profile_inflight: BTreeMap<String, usize>,
    pub(crate) profile_health: BTreeMap<String, RuntimeProfileHealth>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeProfileUsageAuthCacheEntry {
    pub(crate) auth: UsageAuth,
    pub(crate) location: secret_store::SecretLocation,
    pub(crate) revision: Option<secret_store::SecretRevision>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeProfileProbeCacheEntry {
    pub(crate) checked_at: i64,
    pub(crate) auth: AuthSummary,
    pub(crate) result: std::result::Result<UsageResponse, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeProfileUsageSnapshot {
    pub(crate) checked_at: i64,
    pub(crate) five_hour_status: RuntimeQuotaWindowStatus,
    pub(crate) five_hour_remaining_percent: i64,
    pub(crate) five_hour_reset_at: i64,
    pub(crate) weekly_status: RuntimeQuotaWindowStatus,
    pub(crate) weekly_remaining_percent: i64,
    pub(crate) weekly_reset_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RuntimeProfileBackoffs {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub(crate) retry_backoff_until: BTreeMap<String, i64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub(crate) transport_backoff_until: BTreeMap<String, i64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub(crate) route_circuit_open_until: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeContinuationJournal {
    #[serde(default)]
    pub(crate) saved_at: i64,
    #[serde(default)]
    pub(crate) continuations: RuntimeContinuationStore,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeContinuationStore {
    #[serde(default)]
    pub(crate) response_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    pub(crate) session_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    pub(crate) turn_state_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    pub(crate) session_id_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    pub(crate) statuses: RuntimeContinuationStatuses,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeContinuationStatuses {
    #[serde(default)]
    pub(crate) response: BTreeMap<String, RuntimeContinuationBindingStatus>,
    #[serde(default)]
    pub(crate) turn_state: BTreeMap<String, RuntimeContinuationBindingStatus>,
    #[serde(default)]
    pub(crate) session_id: BTreeMap<String, RuntimeContinuationBindingStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub(crate) enum RuntimeContinuationBindingLifecycle {
    #[default]
    Warm,
    Verified,
    Suspect,
    Dead,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeContinuationBindingStatus {
    #[serde(default)]
    pub(crate) state: RuntimeContinuationBindingLifecycle,
    #[serde(default)]
    pub(crate) confidence: u32,
    #[serde(default)]
    pub(crate) last_touched_at: Option<i64>,
    #[serde(default)]
    pub(crate) last_verified_at: Option<i64>,
    #[serde(default)]
    pub(crate) last_verified_route: Option<String>,
    #[serde(default)]
    pub(crate) last_not_found_at: Option<i64>,
    #[serde(default)]
    pub(crate) not_found_streak: u32,
    #[serde(default)]
    pub(crate) success_count: u32,
    #[serde(default)]
    pub(crate) failure_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProbeCacheFreshness {
    Fresh,
    StaleUsable,
    Expired,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RuntimeProfileHealth {
    pub(crate) score: u32,
    pub(crate) updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeRouteKind {
    Responses,
    Compact,
    Websocket,
    Standard,
}

pub(crate) const RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX: &str = "__compact_session__:";
pub(crate) const RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX: &str = "__compact_turn_state__:";
pub(crate) const RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX: &str = "__response_turn_state__:";

use crate::{
    AppPaths, AppState, AuthSummary, ResponseProfileBinding, RuntimeProxyLaneAdmission,
    RuntimeQuotaWindowStatus, UsageAuth, UsageResponse,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime as TokioRuntime;

pub(crate) use prodex_runtime_state::{
    RuntimeContinuationBindingLifecycle, RuntimeContinuationBindingStatus,
    RuntimeContinuationStatuses, RuntimeProbeCacheFreshness, RuntimeProfileBackoffs,
    RuntimeProfileHealth, RuntimeRouteKind,
};

pub(crate) type RuntimeContinuationJournal =
    prodex_runtime_state::RuntimeContinuationJournal<ResponseProfileBinding>;
pub(crate) type RuntimeContinuationStore =
    prodex_runtime_state::RuntimeContinuationStore<ResponseProfileBinding>;
pub(crate) type RuntimeProfileUsageSnapshot =
    prodex_runtime_state::RuntimeProfileUsageSnapshot<RuntimeQuotaWindowStatus>;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeRotationProxyShared {
    pub(crate) upstream_no_proxy: bool,
    pub(crate) async_client: reqwest::Client,
    pub(crate) async_runtime: Arc<TokioRuntime>,
    pub(crate) runtime: Arc<Mutex<RuntimeRotationState>>,
    pub(crate) log_path: PathBuf,
    pub(crate) request_sequence: Arc<AtomicU64>,
    pub(crate) state_save_revision: Arc<AtomicU64>,
    pub(crate) local_overload_backoff_until: Arc<AtomicU64>,
    pub(crate) active_request_count: Arc<AtomicUsize>,
    pub(crate) active_request_limit: usize,
    pub(crate) runtime_state_lock_wait_counters: Arc<RuntimeStateLockWaitMetricCounters>,
    pub(crate) lane_admission: RuntimeProxyLaneAdmission,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeStateLockWaitMetrics {
    pub(crate) wait_total_ns: u64,
    pub(crate) wait_count: u64,
    pub(crate) wait_max_ns: u64,
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeStateLockWaitMetricCounters {
    wait_total_ns: AtomicU64,
    wait_count: AtomicU64,
    wait_max_ns: AtomicU64,
}

impl RuntimeStateLockWaitMetricCounters {
    fn record_wait(&self, wait: Duration) {
        let wait_ns = wait.as_nanos().min(u128::from(u64::MAX)) as u64;
        self.wait_total_ns.fetch_add(wait_ns, Ordering::Relaxed);
        self.wait_count.fetch_add(1, Ordering::Relaxed);
        let mut current_max = self.wait_max_ns.load(Ordering::Relaxed);
        while wait_ns > current_max {
            match self.wait_max_ns.compare_exchange_weak(
                current_max,
                wait_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current_max = observed,
            }
        }
    }

    fn snapshot(&self) -> RuntimeStateLockWaitMetrics {
        RuntimeStateLockWaitMetrics {
            wait_total_ns: self.wait_total_ns.load(Ordering::Relaxed),
            wait_count: self.wait_count.load(Ordering::Relaxed),
            wait_max_ns: self.wait_max_ns.load(Ordering::Relaxed),
        }
    }

    #[cfg(test)]
    fn reset(&self) {
        self.wait_total_ns.store(0, Ordering::Relaxed);
        self.wait_count.store(0, Ordering::Relaxed);
        self.wait_max_ns.store(0, Ordering::Relaxed);
    }
}

impl RuntimeRotationProxyShared {
    pub(crate) fn new_runtime_state_lock_wait_counters() -> Arc<RuntimeStateLockWaitMetricCounters>
    {
        Arc::new(RuntimeStateLockWaitMetricCounters::default())
    }

    pub(crate) fn lock_runtime_state(
        &self,
    ) -> Result<
        MutexGuard<'_, RuntimeRotationState>,
        PoisonError<MutexGuard<'_, RuntimeRotationState>>,
    > {
        let started_at = Instant::now();
        let lock = self.runtime.lock();
        self.record_runtime_state_lock_wait(started_at.elapsed());
        lock
    }

    pub(crate) fn record_runtime_state_lock_wait(&self, wait: Duration) {
        self.runtime_state_lock_wait_counters.record_wait(wait);
    }

    #[allow(dead_code)]
    pub(crate) fn runtime_state_lock_wait_metrics(&self) -> RuntimeStateLockWaitMetrics {
        self.runtime_state_lock_wait_counters.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn reset_runtime_state_lock_wait_metrics_for_test(&self) {
        self.runtime_state_lock_wait_counters.reset();
    }
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

pub(crate) const RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX: &str = "__compact_session__:";
pub(crate) const RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX: &str = "__compact_turn_state__:";
pub(crate) const RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX: &str = "__response_turn_state__:";

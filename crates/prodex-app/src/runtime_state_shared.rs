use crate::{
    AppPaths, AppState, ResponseProfileBinding, RuntimeConfig, RuntimeProxyLaneAdmission,
    RuntimeQuotaWindowStatus, UsageAuth,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime as TokioRuntime;

#[path = "runtime_state_shared/artifact_store.rs"]
mod artifact_store;
#[cfg(test)]
#[path = "runtime_state_shared/artifact_tests.rs"]
mod artifact_tests;
#[path = "runtime_state_shared/artifact_types.rs"]
mod artifact_types;
#[path = "runtime_state_shared/line_index.rs"]
mod line_index;
#[path = "runtime_state_shared/semantic_index.rs"]
mod semantic_index;
#[path = "runtime_state_shared/state.rs"]
mod state;

pub(crate) use artifact_store::*;
pub(crate) use artifact_types::*;
use line_index::*;
use semantic_index::*;
pub(crate) use state::*;

pub(crate) use prodex_runtime_state::{
    RuntimeContinuationBindingLifecycle, RuntimeContinuationBindingStatus,
    RuntimeContinuationStatuses, RuntimeProbeCacheFreshness, RuntimeProfileBackoffs,
    RuntimeProfileHealth, RuntimeRouteKind, RuntimeStateLockWaitMetricCounters,
    RuntimeStateLockWaitMetrics, RuntimeWaitDurationMetricCounters,
};
pub(crate) use prodex_shared_types::RuntimeProfileProbeCacheEntry;

pub(crate) type RuntimeContinuationJournal =
    prodex_runtime_state::RuntimeContinuationJournal<ResponseProfileBinding>;
pub(crate) type RuntimeContinuationStore =
    prodex_runtime_state::RuntimeContinuationStore<ResponseProfileBinding>;
pub(crate) type RuntimeProfileUsageSnapshot =
    prodex_runtime_state::RuntimeProfileUsageSnapshot<RuntimeQuotaWindowStatus>;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeRotationProxyShared {
    pub(crate) runtime_config: Arc<RuntimeConfig>,
    pub(crate) upstream_no_proxy: bool,
    pub(crate) auto_redeem_enabled: bool,
    pub(crate) async_client: reqwest::Client,
    pub(crate) compact_client: reqwest::Client,
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

    pub(crate) fn runtime_state_lock_wait_metrics(&self) -> RuntimeStateLockWaitMetrics {
        self.runtime_state_lock_wait_counters.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn reset_runtime_state_lock_wait_metrics_for_test(&self) {
        self.runtime_state_lock_wait_counters.reset();
    }
}

const RUNTIME_SMART_CONTEXT_MAX_ARTIFACTS: usize = 128;
const RUNTIME_SMART_CONTEXT_MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const RUNTIME_SMART_CONTEXT_MAX_ARTIFACT_BYTES: usize = 1024 * 1024;
const RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_RANGES: usize = 256;
const RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES: usize = 16 * 1024;
const RUNTIME_SMART_CONTEXT_SEMANTIC_SCHEMA_VERSION: u8 = 1;
const RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES: usize = 256;
const RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_FIELD_BYTES: usize = 512;
const RUNTIME_SMART_CONTEXT_MAX_SYMBOL_PREFIX_LINES: usize = 6;
const RUNTIME_SMART_CONTEXT_MAX_SYMBOL_RANGE_LINES: usize = 24;
const RUNTIME_SMART_CONTEXT_MAX_SYMBOL_SIGNATURE_LINES: usize = 6;
const RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS: usize = 256;
const RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_FINGERPRINTS: usize = 64;
const RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_OCCURRENCES: usize = 8;
const RUNTIME_SMART_CONTEXT_CHUNK_WINDOW_LINES: usize = 32;
const RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES: usize = 256;
const RUNTIME_SMART_CONTEXT_REPO_MAP_PREWARM_SCHEMA_VERSION: u8 = 1;

type RuntimeSmartContextArtifactRepoMapKey = (
    RuntimeSmartContextArtifactRepoMapEntryKind,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

#[derive(Debug)]
pub(crate) struct StateFileLock {
    pub(crate) file: fs::File,
}

impl Drop for StateFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

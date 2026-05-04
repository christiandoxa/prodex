use crate::{
    AppPaths, AppState, ResponseProfileBinding, RuntimeProxyLaneAdmission,
    RuntimeQuotaWindowStatus, UsageAuth,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime as TokioRuntime;

pub(crate) use prodex_runtime_state::{
    RuntimeContinuationBindingLifecycle, RuntimeContinuationBindingStatus,
    RuntimeContinuationStatuses, RuntimeProbeCacheFreshness, RuntimeProfileBackoffs,
    RuntimeProfileHealth, RuntimeRouteKind, RuntimeStateLockWaitMetricCounters,
    RuntimeStateLockWaitMetrics,
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

const RUNTIME_SMART_CONTEXT_MAX_ARTIFACTS: usize = 128;
const RUNTIME_SMART_CONTEXT_MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const RUNTIME_SMART_CONTEXT_MAX_ARTIFACT_BYTES: usize = 1024 * 1024;

static RUNTIME_SMART_CONTEXT_ARTIFACT_PROCESS_LOCKS: OnceLock<
    Mutex<BTreeMap<PathBuf, Arc<Mutex<()>>>>,
> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeSmartContextArtifact {
    pub(crate) id: String,
    pub(crate) byte_len: usize,
    pub(crate) content_hash: String,
    pub(crate) text: String,
    pub(crate) sequence: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RuntimeSmartContextArtifactStore {
    artifacts: BTreeMap<String, RuntimeSmartContextArtifact>,
    total_bytes: usize,
}

impl RuntimeSmartContextArtifactStore {
    pub(crate) fn load_from_path(path: &Path) -> Self {
        let Some(raw) = fs::read_to_string(path).ok() else {
            return Self::default();
        };
        let Ok(mut store) = serde_json::from_str::<Self>(&raw) else {
            return Self::default();
        };
        store.recompute_total_bytes();
        store.enforce_limits();
        store
    }

    #[cfg(test)]
    pub(crate) fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        self.save_merged_to_path(path).map(|_| ())
    }

    pub(crate) fn save_merged_to_path(&self, path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let process_lock = runtime_smart_context_artifact_process_lock(path);
        let _process_guard = process_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _lock = crate::runtime_store::acquire_json_file_lock(path)?;
        let mut merged = Self::load_from_path(path);
        merged.merge_from(self);
        merged.write_to_path_unlocked(path)?;
        Ok(merged)
    }

    pub(crate) fn artifact_count(&self) -> usize {
        self.artifacts.len()
    }

    pub(crate) fn insert_text(
        &mut self,
        sequence: u64,
        text: &str,
    ) -> Option<runtime_proxy_crate::SmartContextArtifactRef> {
        if text.len() > RUNTIME_SMART_CONTEXT_MAX_ARTIFACT_BYTES {
            return None;
        }
        let content_hash = runtime_proxy_crate::smart_context_hash_text(text);
        let id = content_hash.clone();
        if let Some(existing) = self.artifacts.get_mut(&id) {
            existing.sequence = sequence;
            return Some(runtime_proxy_crate::SmartContextArtifactRef {
                id: existing.id.clone(),
                byte_len: existing.byte_len,
                content_hash: existing.content_hash.clone(),
            });
        }

        let byte_len = text.len();
        self.artifacts.insert(
            id.clone(),
            RuntimeSmartContextArtifact {
                id: id.clone(),
                byte_len,
                content_hash: content_hash.clone(),
                text: text.to_string(),
                sequence,
            },
        );
        self.total_bytes = self.total_bytes.saturating_add(byte_len);
        self.enforce_limits();
        Some(runtime_proxy_crate::SmartContextArtifactRef {
            id,
            byte_len,
            content_hash,
        })
    }

    pub(crate) fn get_text(&self, id: &str) -> Option<String> {
        self.artifacts.get(id).map(|artifact| artifact.text.clone())
    }

    pub(crate) fn contains(&self, id: &str) -> bool {
        self.artifacts.contains_key(id)
    }

    fn merge_from(&mut self, incoming: &Self) {
        for (id, incoming_artifact) in &incoming.artifacts {
            self.artifacts
                .entry(id.clone())
                .and_modify(|current| {
                    if incoming_artifact.sequence >= current.sequence {
                        *current = incoming_artifact.clone();
                    }
                })
                .or_insert_with(|| incoming_artifact.clone());
        }
        self.recompute_total_bytes();
        self.enforce_limits();
    }

    fn write_to_path_unlocked(&self, path: &Path) -> anyhow::Result<()> {
        let raw = serde_json::to_vec(self)?;
        let temp_path = crate::runtime_store::unique_state_temp_file_path(path);
        fs::write(&temp_path, raw)?;
        if let Err(err) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(err.into());
        }
        Ok(())
    }

    fn recompute_total_bytes(&mut self) {
        self.total_bytes = self
            .artifacts
            .values()
            .map(|artifact| artifact.byte_len)
            .sum();
    }

    fn enforce_limits(&mut self) {
        while self.artifacts.len() > RUNTIME_SMART_CONTEXT_MAX_ARTIFACTS
            || self.total_bytes > RUNTIME_SMART_CONTEXT_MAX_TOTAL_BYTES
        {
            let Some(oldest_id) = self
                .artifacts
                .values()
                .min_by_key(|artifact| artifact.sequence)
                .map(|artifact| artifact.id.clone())
            else {
                break;
            };
            if let Some(removed) = self.artifacts.remove(&oldest_id) {
                self.total_bytes = self.total_bytes.saturating_sub(removed.byte_len);
            }
        }
    }
}

fn runtime_smart_context_artifact_process_lock(path: &Path) -> Arc<Mutex<()>> {
    let locks =
        RUNTIME_SMART_CONTEXT_ARTIFACT_PROCESS_LOCKS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut locks = locks
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Arc::clone(
        locks
            .entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(()))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_smart_context_artifact_save_merges_existing_file() {
        let path = smart_context_artifact_temp_path("merge-save");
        remove_smart_context_artifact_temp_files(&path);

        let mut first = RuntimeSmartContextArtifactStore::default();
        let alpha = first.insert_text(1, "alpha").expect("alpha artifact");
        first.save_to_path(&path).expect("first store saved");

        let mut second = RuntimeSmartContextArtifactStore::default();
        let beta = second.insert_text(2, "beta").expect("beta artifact");
        second.save_to_path(&path).expect("second store saved");

        let loaded = RuntimeSmartContextArtifactStore::load_from_path(&path);
        assert_eq!(loaded.artifact_count(), 2);
        assert_eq!(loaded.get_text(&alpha.id).as_deref(), Some("alpha"));
        assert_eq!(loaded.get_text(&beta.id).as_deref(), Some("beta"));

        remove_smart_context_artifact_temp_files(&path);
    }

    #[test]
    fn runtime_smart_context_artifact_concurrent_saves_keep_all_artifacts() {
        let path = Arc::new(smart_context_artifact_temp_path("concurrent-merge-save"));
        remove_smart_context_artifact_temp_files(path.as_ref());
        let thread_count = 8;
        let barrier = Arc::new(Barrier::new(thread_count));

        let handles = (0..thread_count)
            .map(|index| {
                let path = Arc::clone(&path);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let text = format!("artifact-{index}");
                    let mut store = RuntimeSmartContextArtifactStore::default();
                    let artifact = store
                        .insert_text(index as u64 + 1, &text)
                        .expect("artifact inserted");
                    barrier.wait();
                    store.save_to_path(path.as_ref()).expect("store saved");
                    (artifact.id, text)
                })
            })
            .collect::<Vec<_>>();

        let expected = handles
            .into_iter()
            .map(|handle| handle.join().expect("save thread joined"))
            .collect::<Vec<_>>();

        let loaded = RuntimeSmartContextArtifactStore::load_from_path(path.as_ref());
        assert_eq!(loaded.artifact_count(), thread_count);
        for (id, text) in expected {
            assert_eq!(loaded.get_text(&id).as_deref(), Some(text.as_str()));
        }

        remove_smart_context_artifact_temp_files(path.as_ref());
    }

    fn smart_context_artifact_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "prodex-app-smart-context-artifacts-{name}-{}-{nanos}.json",
            std::process::id()
        ))
    }

    fn remove_smart_context_artifact_temp_files(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(crate::runtime_store::json_lock_file_path(path));
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

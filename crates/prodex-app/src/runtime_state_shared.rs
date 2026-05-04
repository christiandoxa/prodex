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
const RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_RANGES: usize = 256;
const RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES: usize = 16 * 1024;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) line_index: Option<RuntimeSmartContextArtifactLineIndex>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactLineIndex {
    #[serde(default)]
    pub(crate) complete: bool,
    #[serde(default)]
    pub(crate) critical_ranges: Vec<RuntimeSmartContextArtifactLineRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactLineRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) byte_len: usize,
    pub(crate) content_hash: String,
    pub(crate) text: String,
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
            if !Self::artifact_matches_text(existing, text, &content_hash) {
                return None;
            }
            existing.sequence = sequence;
            if existing.line_index.is_none() {
                existing.line_index = Some(runtime_smart_context_artifact_line_index(text));
            }
            return Some(Self::artifact_ref(existing));
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
                line_index: Some(runtime_smart_context_artifact_line_index(text)),
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

    pub(crate) fn line_index(&self, id: &str) -> Option<&RuntimeSmartContextArtifactLineIndex> {
        self.artifacts
            .get(id)
            .and_then(|artifact| artifact.line_index.as_ref())
    }

    pub(crate) fn artifact_ref_for_exact_text(
        &self,
        text: &str,
    ) -> Option<runtime_proxy_crate::SmartContextArtifactRef> {
        let content_hash = runtime_proxy_crate::smart_context_hash_text(text);
        let artifact = self.artifacts.get(&content_hash)?;
        Self::artifact_matches_text(artifact, text, &content_hash)
            .then(|| Self::artifact_ref(artifact))
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

    fn artifact_matches_text(
        artifact: &RuntimeSmartContextArtifact,
        text: &str,
        content_hash: &str,
    ) -> bool {
        artifact.content_hash == content_hash
            && artifact.byte_len == text.len()
            && artifact.text == text
    }

    fn artifact_ref(
        artifact: &RuntimeSmartContextArtifact,
    ) -> runtime_proxy_crate::SmartContextArtifactRef {
        runtime_proxy_crate::SmartContextArtifactRef {
            id: artifact.id.clone(),
            byte_len: artifact.byte_len,
            content_hash: artifact.content_hash.clone(),
        }
    }
}

fn runtime_smart_context_artifact_line_index(text: &str) -> RuntimeSmartContextArtifactLineIndex {
    let mut ranges = prodex_context::critical_signal_lost_line_ranges_with_options(
        text,
        "",
        prodex_context::CriticalSignalLineRangeOptions {
            context_lines: 1,
            max_ranges: RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_RANGES.saturating_add(1),
            max_range_lines: 6,
        },
    );
    let complete = ranges.len() <= RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_RANGES;
    ranges.truncate(RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_RANGES);

    let lines = text.lines().collect::<Vec<_>>();
    let mut indexed_ranges = Vec::new();
    let mut index_complete = complete;
    for range in ranges {
        if range.start == 0 || range.start > lines.len() || range.end < range.start {
            index_complete = false;
            continue;
        }
        let end = range.end.min(lines.len());
        let excerpt = lines[range.start - 1..end].join("\n");
        if excerpt.len() > RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES {
            index_complete = false;
            continue;
        }
        indexed_ranges.push(RuntimeSmartContextArtifactLineRange {
            start: range.start,
            end,
            byte_len: excerpt.len(),
            content_hash: runtime_proxy_crate::smart_context_hash_text(&excerpt),
            text: excerpt,
        });
    }

    RuntimeSmartContextArtifactLineIndex {
        complete: index_complete,
        critical_ranges: indexed_ranges,
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
    fn runtime_smart_context_artifact_ref_for_exact_text_requires_exact_hash_match() {
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store
            .insert_text(1, "repeatable command output")
            .expect("artifact inserted");

        let found = store
            .artifact_ref_for_exact_text("repeatable command output")
            .expect("exact text should resolve to artifact ref");
        assert_eq!(found.id, artifact.id);
        assert_eq!(found.content_hash, artifact.content_hash);
        assert_eq!(found.byte_len, artifact.byte_len);
        assert!(
            store
                .artifact_ref_for_exact_text("repeatable command output with suffix")
                .is_none(),
            "near matches must not reuse artifacts"
        );
        store
            .artifacts
            .get_mut(&artifact.id)
            .expect("stored artifact")
            .content_hash = runtime_proxy_crate::smart_context_hash_text("stale content");
        assert!(
            store
                .artifact_ref_for_exact_text("repeatable command output")
                .is_none(),
            "stale artifact metadata must not produce an exact ref"
        );
    }

    #[test]
    fn runtime_smart_context_artifact_insert_stores_critical_line_index() {
        let text = "\
setup
error: hidden failure
src/main.rs:22:5
test result: FAILED. 0 passed; 1 failed
tail";
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(1, text).expect("artifact inserted");

        let index = store
            .line_index(&artifact.id)
            .expect("new artifacts should carry a line index");
        assert!(index.complete);
        assert!(
            index
                .critical_ranges
                .iter()
                .any(|range| range.text.contains("error: hidden failure"))
        );
        assert!(
            index
                .critical_ranges
                .iter()
                .any(|range| range.text.contains("src/main.rs:22:5"))
        );
        assert!(
            index
                .critical_ranges
                .iter()
                .any(|range| range.text.contains("test result: FAILED"))
        );
        for range in &index.critical_ranges {
            assert_eq!(range.byte_len, range.text.len());
            assert_eq!(
                range.content_hash,
                runtime_proxy_crate::smart_context_hash_text(&range.text)
            );
        }
        assert_eq!(store.get_text(&artifact.id).as_deref(), Some(text));
    }

    #[test]
    fn runtime_smart_context_artifact_json_without_line_index_still_loads() {
        let text = "error: old failure\nsrc/main.rs:22:5";
        let content_hash = runtime_proxy_crate::smart_context_hash_text(text);
        let mut artifacts = serde_json::Map::new();
        artifacts.insert(
            content_hash.clone(),
            serde_json::json!({
                "id": content_hash.clone(),
                "byte_len": text.len(),
                "content_hash": runtime_proxy_crate::smart_context_hash_text(text),
                "text": text,
                "sequence": 1
            }),
        );
        let raw = serde_json::json!({
            "artifacts": artifacts,
            "total_bytes": text.len()
        });

        let mut store: RuntimeSmartContextArtifactStore =
            serde_json::from_value(raw).expect("legacy artifact store should deserialize");

        assert_eq!(store.get_text(&content_hash).as_deref(), Some(text));
        assert!(store.line_index(&content_hash).is_none());

        store
            .insert_text(2, text)
            .expect("matching legacy artifact should refresh metadata");

        assert!(
            store
                .line_index(&content_hash)
                .is_some_and(|index| index.complete)
        );
        assert_eq!(store.get_text(&content_hash).as_deref(), Some(text));
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

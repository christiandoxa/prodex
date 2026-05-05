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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) chunk_index: Option<RuntimeSmartContextArtifactChunkIndex>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactLineIndex {
    #[serde(default)]
    pub(crate) complete: bool,
    #[serde(default, skip_serializing_if = "runtime_smart_context_u8_is_zero")]
    pub(crate) semantic_schema_version: u8,
    #[serde(
        default = "runtime_smart_context_semantic_index_complete_default",
        skip_serializing_if = "runtime_smart_context_bool_is_true"
    )]
    pub(crate) semantic_complete: bool,
    #[serde(
        default = "runtime_smart_context_semantic_index_complete_default",
        skip_serializing_if = "runtime_smart_context_bool_is_true"
    )]
    pub(crate) symbol_complete: bool,
    #[serde(default)]
    pub(crate) critical_ranges: Vec<RuntimeSmartContextArtifactLineRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) file_location_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) diff_hunk_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) test_failure_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) error_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) symbol_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) command_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactLineRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) byte_len: usize,
    pub(crate) content_hash: String,
    pub(crate) text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactSemanticLineRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) byte_len: usize,
    pub(crate) content_hash: String,
    pub(crate) text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) column: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) old_start: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) old_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) new_start: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) new_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) symbol: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactChunkIndex {
    #[serde(default)]
    pub(crate) complete: bool,
    #[serde(default)]
    pub(crate) chunks: Vec<RuntimeSmartContextArtifactChunkFingerprint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) duplicate_chunks: Vec<RuntimeSmartContextArtifactDuplicateChunkFingerprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactChunkFingerprint {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) byte_len: usize,
    pub(crate) content_hash: String,
    pub(crate) kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactDuplicateChunkFingerprint {
    pub(crate) byte_len: usize,
    pub(crate) content_hash: String,
    pub(crate) occurrence_count: usize,
    #[serde(default)]
    pub(crate) occurrences: Vec<RuntimeSmartContextArtifactChunkOccurrence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactChunkOccurrence {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactManifestEntry {
    pub(crate) id: String,
    pub(crate) byte_len: usize,
    pub(crate) content_hash: String,
    pub(crate) critical_range_count: usize,
    pub(crate) file_location_range_count: usize,
    pub(crate) diff_hunk_range_count: usize,
    pub(crate) test_failure_range_count: usize,
    pub(crate) error_range_count: usize,
    pub(crate) command_kind: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RuntimeSmartContextArtifactStore {
    artifacts: BTreeMap<String, RuntimeSmartContextArtifact>,
    total_bytes: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    static_context_fingerprints: Vec<RuntimeSmartContextStaticFingerprintMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    static_context_prompt_cache_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextStaticFingerprintMetadata {
    pub(crate) id: String,
    pub(crate) content_hash: String,
    pub(crate) byte_len: usize,
}

impl RuntimeSmartContextArtifactStore {
    pub(crate) fn load_from_path(path: &Path) -> Self {
        let Some(raw) = fs::read_to_string(path).ok() else {
            return Self::default();
        };
        let Ok(mut store) = serde_json::from_str::<Self>(&raw) else {
            return Self::default();
        };
        store.validate_loaded_metadata();
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

    pub(crate) fn set_static_context_fingerprints(
        &mut self,
        prompt_cache_hash: Option<String>,
        fingerprints: Vec<runtime_proxy_crate::SmartContextFingerprint>,
    ) {
        self.static_context_prompt_cache_hash = prompt_cache_hash;
        self.static_context_fingerprints = fingerprints
            .into_iter()
            .filter(|fingerprint| {
                fingerprint.kind == runtime_proxy_crate::SmartContextFingerprintKind::StaticContext
                    && !fingerprint.id.trim().is_empty()
                    && !fingerprint.content_hash.trim().is_empty()
            })
            .map(|fingerprint| RuntimeSmartContextStaticFingerprintMetadata {
                id: fingerprint.id,
                content_hash: fingerprint.content_hash,
                byte_len: fingerprint.byte_len,
            })
            .collect();
    }

    #[cfg(test)]
    pub(crate) fn static_context_fingerprints(
        &self,
    ) -> Vec<runtime_proxy_crate::SmartContextFingerprint> {
        self.static_context_fingerprints
            .iter()
            .map(|fingerprint| runtime_proxy_crate::SmartContextFingerprint {
                id: fingerprint.id.clone(),
                kind: runtime_proxy_crate::SmartContextFingerprintKind::StaticContext,
                content_hash: fingerprint.content_hash.clone(),
                byte_len: fingerprint.byte_len,
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn static_context_prompt_cache_hash(&self) -> Option<&str> {
        self.static_context_prompt_cache_hash.as_deref()
    }

    pub(crate) fn artifact_manifest_entries(
        &self,
        limit: usize,
    ) -> Vec<RuntimeSmartContextArtifactManifestEntry> {
        if limit == 0 {
            return Vec::new();
        }
        let mut artifacts = self.artifacts.values().collect::<Vec<_>>();
        artifacts.sort_by(|left, right| {
            right
                .sequence
                .cmp(&left.sequence)
                .then_with(|| left.id.cmp(&right.id))
        });
        artifacts
            .into_iter()
            .take(limit)
            .map(|artifact| {
                let line_index = artifact.line_index.as_ref();
                RuntimeSmartContextArtifactManifestEntry {
                    id: artifact.id.clone(),
                    byte_len: artifact.byte_len,
                    content_hash: artifact.content_hash.clone(),
                    critical_range_count: line_index.map_or(0, |index| index.critical_ranges.len()),
                    file_location_range_count: line_index
                        .map_or(0, |index| index.file_location_ranges.len()),
                    diff_hunk_range_count: line_index
                        .map_or(0, |index| index.diff_hunk_ranges.len()),
                    test_failure_range_count: line_index
                        .map_or(0, |index| index.test_failure_ranges.len()),
                    error_range_count: line_index.map_or(0, |index| index.error_ranges.len()),
                    command_kind: line_index.and_then(|index| index.command_kind.clone()),
                }
            })
            .collect()
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
            let refresh_line_index = runtime_smart_context_artifact_line_index_needs_refresh(
                existing.line_index.as_ref(),
            );
            if refresh_line_index || existing.chunk_index.is_none() {
                let line_index = if refresh_line_index {
                    runtime_smart_context_artifact_line_index(text)
                } else {
                    existing
                        .line_index
                        .clone()
                        .expect("line index should exist when refresh is not needed")
                };
                if refresh_line_index {
                    existing.line_index = Some(line_index.clone());
                }
                if refresh_line_index || existing.chunk_index.is_none() {
                    existing.chunk_index = Some(runtime_smart_context_artifact_chunk_index(
                        text,
                        &line_index,
                    ));
                }
            }
            return Some(Self::artifact_ref(existing));
        }

        let byte_len = text.len();
        let line_index = runtime_smart_context_artifact_line_index(text);
        let chunk_index = runtime_smart_context_artifact_chunk_index(text, &line_index);
        self.artifacts.insert(
            id.clone(),
            RuntimeSmartContextArtifact {
                id: id.clone(),
                byte_len,
                content_hash: content_hash.clone(),
                text: text.to_string(),
                sequence,
                line_index: Some(line_index),
                chunk_index: Some(chunk_index),
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

    #[allow(dead_code)]
    pub(crate) fn chunk_index(&self, id: &str) -> Option<&RuntimeSmartContextArtifactChunkIndex> {
        self.artifacts
            .get(id)
            .and_then(|artifact| artifact.chunk_index.as_ref())
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
        if !incoming.static_context_fingerprints.is_empty()
            || incoming.static_context_prompt_cache_hash.is_some()
        {
            self.static_context_fingerprints = incoming.static_context_fingerprints.clone();
            self.static_context_prompt_cache_hash =
                incoming.static_context_prompt_cache_hash.clone();
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

    fn validate_loaded_metadata(&mut self) {
        self.artifacts.retain(|id, artifact| {
            id == &artifact.content_hash
                && Self::artifact_matches_text(
                    artifact,
                    &artifact.text,
                    &runtime_proxy_crate::smart_context_hash_text(&artifact.text),
                )
        });

        for artifact in self.artifacts.values_mut() {
            let refresh_line_index = runtime_smart_context_artifact_line_index_needs_refresh(
                artifact.line_index.as_ref(),
            );
            if refresh_line_index || artifact.chunk_index.is_none() {
                let line_index = if refresh_line_index {
                    runtime_smart_context_artifact_line_index(&artifact.text)
                } else {
                    artifact
                        .line_index
                        .clone()
                        .expect("line index should exist when refresh is not needed")
                };
                if refresh_line_index {
                    artifact.line_index = Some(line_index.clone());
                }
                if refresh_line_index || artifact.chunk_index.is_none() {
                    artifact.chunk_index = Some(runtime_smart_context_artifact_chunk_index(
                        &artifact.text,
                        &line_index,
                    ));
                }
            }
        }

        self.static_context_fingerprints.retain(|fingerprint| {
            !fingerprint.id.trim().is_empty() && !fingerprint.content_hash.trim().is_empty()
        });
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

    let semantic_index = runtime_smart_context_artifact_semantic_line_index(&lines);

    RuntimeSmartContextArtifactLineIndex {
        complete: index_complete,
        semantic_schema_version: RUNTIME_SMART_CONTEXT_SEMANTIC_SCHEMA_VERSION,
        semantic_complete: semantic_index.complete && semantic_index.symbol_complete,
        symbol_complete: semantic_index.symbol_complete,
        critical_ranges: indexed_ranges,
        file_location_ranges: semantic_index.file_location_ranges,
        diff_hunk_ranges: semantic_index.diff_hunk_ranges,
        test_failure_ranges: semantic_index.test_failure_ranges,
        error_ranges: semantic_index.error_ranges,
        symbol_ranges: semantic_index.symbol_ranges,
        command_kind: runtime_smart_context_infer_command_kind(&lines),
    }
}

fn runtime_smart_context_artifact_line_index_needs_refresh(
    line_index: Option<&RuntimeSmartContextArtifactLineIndex>,
) -> bool {
    match line_index {
        Some(line_index) => {
            line_index.semantic_schema_version < RUNTIME_SMART_CONTEXT_SEMANTIC_SCHEMA_VERSION
        }
        None => true,
    }
}

fn runtime_smart_context_artifact_chunk_index(
    text: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
) -> RuntimeSmartContextArtifactChunkIndex {
    let lines = text.lines().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut complete = line_index.semantic_complete && line_index.symbol_complete;

    for range in &line_index.file_location_ranges {
        runtime_smart_context_push_chunk_fingerprint(&mut chunks, &mut complete, "file", range);
    }
    for range in &line_index.diff_hunk_ranges {
        runtime_smart_context_push_chunk_fingerprint(&mut chunks, &mut complete, "diff", range);
    }
    for range in &line_index.test_failure_ranges {
        runtime_smart_context_push_chunk_fingerprint(&mut chunks, &mut complete, "test", range);
    }
    for range in &line_index.error_ranges {
        runtime_smart_context_push_chunk_fingerprint(&mut chunks, &mut complete, "error", range);
    }
    for range in &line_index.symbol_ranges {
        runtime_smart_context_push_chunk_fingerprint(&mut chunks, &mut complete, "symbol", range);
    }

    if chunks.is_empty() {
        runtime_smart_context_push_window_chunk_fingerprints(&mut chunks, &mut complete, &lines);
    }

    let (duplicate_chunks, duplicate_metadata_complete) =
        runtime_smart_context_duplicate_chunk_fingerprints(&chunks);
    RuntimeSmartContextArtifactChunkIndex {
        complete: complete && duplicate_metadata_complete,
        chunks,
        duplicate_chunks,
    }
}

fn runtime_smart_context_push_chunk_fingerprint(
    chunks: &mut Vec<RuntimeSmartContextArtifactChunkFingerprint>,
    complete: &mut bool,
    kind: &str,
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) {
    if chunks.len() >= RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS {
        *complete = false;
        return;
    }
    if range.byte_len != range.text.len()
        || range.content_hash != runtime_proxy_crate::smart_context_hash_text(&range.text)
    {
        *complete = false;
        return;
    }
    chunks.push(RuntimeSmartContextArtifactChunkFingerprint {
        start: range.start,
        end: range.end,
        byte_len: range.byte_len,
        content_hash: range.content_hash.clone(),
        kind: kind.to_string(),
        label: range.label.clone(),
        path: range.path.clone(),
        code: range.code.clone(),
        symbol: range.symbol.clone(),
    });
}

fn runtime_smart_context_push_window_chunk_fingerprints(
    chunks: &mut Vec<RuntimeSmartContextArtifactChunkFingerprint>,
    complete: &mut bool,
    lines: &[&str],
) {
    let mut start = 1usize;
    while start <= lines.len() {
        if chunks.len() >= RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS {
            *complete = false;
            return;
        }
        let end = (start + RUNTIME_SMART_CONTEXT_CHUNK_WINDOW_LINES - 1).min(lines.len());
        let Some(text) = runtime_smart_context_line_excerpt(lines, start, end) else {
            *complete = false;
            start = end.saturating_add(1);
            continue;
        };
        chunks.push(RuntimeSmartContextArtifactChunkFingerprint {
            start,
            end,
            byte_len: text.len(),
            content_hash: runtime_proxy_crate::smart_context_hash_text(&text),
            kind: "window".to_string(),
            label: None,
            path: None,
            code: None,
            symbol: None,
        });
        start = end.saturating_add(1);
    }
}

fn runtime_smart_context_duplicate_chunk_fingerprints(
    chunks: &[RuntimeSmartContextArtifactChunkFingerprint],
) -> (
    Vec<RuntimeSmartContextArtifactDuplicateChunkFingerprint>,
    bool,
) {
    let mut grouped =
        BTreeMap::<(String, usize), Vec<&RuntimeSmartContextArtifactChunkFingerprint>>::new();
    for chunk in chunks {
        grouped
            .entry((chunk.content_hash.clone(), chunk.byte_len))
            .or_default()
            .push(chunk);
    }

    let mut duplicates = Vec::new();
    let mut complete = true;
    for ((content_hash, byte_len), occurrences) in grouped {
        if occurrences.len() < 2 {
            continue;
        }
        if duplicates.len() >= RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_FINGERPRINTS {
            complete = false;
            break;
        }
        let occurrences_complete =
            occurrences.len() <= RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_OCCURRENCES;
        complete &= occurrences_complete;
        duplicates.push(RuntimeSmartContextArtifactDuplicateChunkFingerprint {
            byte_len,
            content_hash,
            occurrence_count: occurrences.len(),
            occurrences: occurrences
                .into_iter()
                .take(RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_OCCURRENCES)
                .map(|chunk| RuntimeSmartContextArtifactChunkOccurrence {
                    start: chunk.start,
                    end: chunk.end,
                    kind: chunk.kind.clone(),
                })
                .collect(),
        });
    }
    (duplicates, complete)
}

#[derive(Default)]
struct RuntimeSmartContextArtifactSemanticLineIndexParts {
    file_location_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    diff_hunk_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    test_failure_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    error_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    symbol_ranges: Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    complete: bool,
    symbol_complete: bool,
}

#[derive(Debug, Clone)]
struct RuntimeSmartContextParsedFileLocation {
    path: String,
    line: usize,
    column: Option<usize>,
}

#[derive(Debug, Clone)]
struct RuntimeSmartContextParsedDiffHunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
}

#[derive(Default)]
struct RuntimeSmartContextSemanticRangeMetadata {
    label: Option<String>,
    path: Option<String>,
    line: Option<usize>,
    column: Option<usize>,
    old_start: Option<usize>,
    old_count: Option<usize>,
    new_start: Option<usize>,
    new_count: Option<usize>,
    code: Option<String>,
    symbol: Option<String>,
}

#[derive(Debug, Clone)]
struct RuntimeSmartContextParsedSymbolLine {
    label: &'static str,
    symbol: String,
    style: RuntimeSmartContextSymbolRangeStyle,
}

#[derive(Debug, Clone, Copy)]
enum RuntimeSmartContextSymbolRangeStyle {
    Brace,
    Python,
}

fn runtime_smart_context_artifact_semantic_line_index(
    lines: &[&str],
) -> RuntimeSmartContextArtifactSemanticLineIndexParts {
    let mut parts = RuntimeSmartContextArtifactSemanticLineIndexParts {
        complete: true,
        symbol_complete: true,
        ..Default::default()
    };
    let mut remaining = RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES;
    let mut current_diff_path: Option<String> = None;

    for (index, line) in lines.iter().enumerate() {
        let line_number = index + 1;

        if let Some(path) = runtime_smart_context_parse_diff_file_path(line) {
            current_diff_path = Some(path);
        }

        if let Some(hunk) = runtime_smart_context_parse_diff_hunk(line) {
            let end = runtime_smart_context_diff_hunk_end(lines, index);
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("diff_hunk".to_string()),
                path: current_diff_path.clone(),
                old_start: Some(hunk.old_start),
                old_count: Some(hunk.old_count),
                new_start: Some(hunk.new_start),
                new_count: Some(hunk.new_count),
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.diff_hunk_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number,
                end,
                metadata,
            );
        }

        if let Some(location) = runtime_smart_context_parse_file_location(line) {
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("file_location".to_string()),
                path: Some(location.path),
                line: Some(location.line),
                column: location.column,
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.file_location_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number,
                line_number,
                metadata,
            );
        }

        if runtime_smart_context_is_test_failure_line(line) {
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("test_failure".to_string()),
                symbol: runtime_smart_context_parse_test_symbol(line),
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.test_failure_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number.saturating_sub(1).max(1),
                (line_number + 1).min(lines.len()),
                metadata,
            );
        }

        if let Some(code) = runtime_smart_context_parse_error_code(line) {
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("error".to_string()),
                code: Some(code),
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.error_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number,
                line_number,
                metadata,
            );
        }
    }

    for (index, _) in lines.iter().enumerate() {
        let Some(symbol) = runtime_smart_context_parse_symbol_line(lines, index) else {
            continue;
        };
        let (start, end) = runtime_smart_context_symbol_range_bounds(lines, index, symbol.style);
        let metadata = RuntimeSmartContextSemanticRangeMetadata {
            label: Some(symbol.label.to_string()),
            line: Some(index + 1),
            symbol: Some(symbol.symbol),
            ..Default::default()
        };
        runtime_smart_context_push_semantic_range(
            &mut parts.symbol_ranges,
            &mut remaining,
            &mut parts.symbol_complete,
            lines,
            start,
            end,
            metadata,
        );
    }

    parts
}

fn runtime_smart_context_push_semantic_range(
    target: &mut Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    remaining: &mut usize,
    complete: &mut bool,
    lines: &[&str],
    start: usize,
    end: usize,
    metadata: RuntimeSmartContextSemanticRangeMetadata,
) {
    if *remaining == 0 {
        *complete = false;
        return;
    }
    let Some(text) = runtime_smart_context_line_excerpt(lines, start, end) else {
        *complete = false;
        return;
    };
    if target.iter().any(|range| {
        range.start == start
            && range.end == end
            && range.label == metadata.label
            && range.path == metadata.path
            && range.code == metadata.code
            && range.symbol == metadata.symbol
    }) {
        return;
    }
    let byte_len = text.len();
    target.push(RuntimeSmartContextArtifactSemanticLineRange {
        start,
        end,
        byte_len,
        content_hash: runtime_proxy_crate::smart_context_hash_text(&text),
        text,
        label: metadata.label,
        path: metadata.path,
        line: metadata.line,
        column: metadata.column,
        old_start: metadata.old_start,
        old_count: metadata.old_count,
        new_start: metadata.new_start,
        new_count: metadata.new_count,
        code: metadata.code,
        symbol: metadata.symbol,
    });
    *remaining = remaining.saturating_sub(1);
}

fn runtime_smart_context_line_excerpt(lines: &[&str], start: usize, end: usize) -> Option<String> {
    if start == 0 || start > lines.len() || end < start {
        return None;
    }
    let end = end.min(lines.len());
    let excerpt = lines[start - 1..end].join("\n");
    (excerpt.len() <= RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES).then_some(excerpt)
}

fn runtime_smart_context_parse_symbol_line(
    lines: &[&str],
    index: usize,
) -> Option<RuntimeSmartContextParsedSymbolLine> {
    let line = lines.get(index)?.trim_start();
    if line.is_empty()
        || line.starts_with("//")
        || line.starts_with("/*")
        || line.starts_with('*')
        || line.starts_with("#[")
        || line.starts_with('@')
    {
        return None;
    }

    let rust_test = runtime_smart_context_has_rust_test_attribute(lines, index);
    if let Some(symbol) = runtime_smart_context_identifier_after_keyword(line, "fn") {
        return Some(RuntimeSmartContextParsedSymbolLine {
            label: if rust_test { "test_symbol" } else { "function" },
            symbol,
            style: RuntimeSmartContextSymbolRangeStyle::Brace,
        });
    }

    if let Some(symbol) = runtime_smart_context_parse_python_symbol(line) {
        return Some(RuntimeSmartContextParsedSymbolLine {
            label: if symbol.starts_with("test_") {
                "test_symbol"
            } else {
                "function"
            },
            symbol,
            style: RuntimeSmartContextSymbolRangeStyle::Python,
        });
    }

    if let Some(symbol) = runtime_smart_context_parse_js_test_symbol(line) {
        return Some(RuntimeSmartContextParsedSymbolLine {
            label: "test_symbol",
            symbol,
            style: RuntimeSmartContextSymbolRangeStyle::Brace,
        });
    }

    if let Some(symbol) = runtime_smart_context_parse_js_function_symbol(line) {
        return Some(RuntimeSmartContextParsedSymbolLine {
            label: "function",
            symbol,
            style: RuntimeSmartContextSymbolRangeStyle::Brace,
        });
    }

    for keyword in ["struct", "enum", "trait", "impl", "mod", "class"] {
        if let Some(symbol) = runtime_smart_context_identifier_after_keyword(line, keyword) {
            return Some(RuntimeSmartContextParsedSymbolLine {
                label: "symbol",
                symbol: if keyword == "impl" {
                    format!("impl {symbol}")
                } else {
                    symbol
                },
                style: RuntimeSmartContextSymbolRangeStyle::Brace,
            });
        }
    }

    None
}

fn runtime_smart_context_symbol_range_bounds(
    lines: &[&str],
    declaration_index: usize,
    style: RuntimeSmartContextSymbolRangeStyle,
) -> (usize, usize) {
    let start_index = runtime_smart_context_symbol_prefix_start(lines, declaration_index);
    let end_index = match style {
        RuntimeSmartContextSymbolRangeStyle::Python => {
            runtime_smart_context_python_symbol_end(lines, declaration_index)
        }
        RuntimeSmartContextSymbolRangeStyle::Brace => {
            runtime_smart_context_brace_symbol_end(lines, declaration_index)
        }
    };
    (start_index + 1, end_index + 1)
}

fn runtime_smart_context_symbol_prefix_start(lines: &[&str], declaration_index: usize) -> usize {
    let mut start = declaration_index;
    let lower_bound =
        declaration_index.saturating_sub(RUNTIME_SMART_CONTEXT_MAX_SYMBOL_PREFIX_LINES);
    while start > lower_bound {
        let previous = lines[start - 1].trim_start();
        if previous.is_empty()
            || previous.starts_with("#[")
            || previous.starts_with('@')
            || previous.starts_with("//")
        {
            start -= 1;
        } else {
            break;
        }
    }
    start
}

fn runtime_smart_context_brace_symbol_end(lines: &[&str], declaration_index: usize) -> usize {
    let max_end = (declaration_index + RUNTIME_SMART_CONTEXT_MAX_SYMBOL_RANGE_LINES - 1)
        .min(lines.len().saturating_sub(1));
    let mut balance = 0isize;
    let mut saw_open = false;
    for (index, line) in lines
        .iter()
        .enumerate()
        .take(max_end + 1)
        .skip(declaration_index)
    {
        for ch in line.chars() {
            if ch == '{' {
                saw_open = true;
                balance += 1;
            } else if ch == '}' && saw_open {
                balance -= 1;
            }
        }
        if saw_open && balance <= 0 {
            return index;
        }
        if !saw_open
            && index > declaration_index
            && index - declaration_index >= RUNTIME_SMART_CONTEXT_MAX_SYMBOL_SIGNATURE_LINES
        {
            return index;
        }
        if !saw_open && line.trim_end().ends_with(';') {
            return index;
        }
    }
    max_end
}

fn runtime_smart_context_python_symbol_end(lines: &[&str], declaration_index: usize) -> usize {
    let base_indent = runtime_smart_context_leading_whitespace(lines[declaration_index]);
    let max_end = (declaration_index + RUNTIME_SMART_CONTEXT_MAX_SYMBOL_RANGE_LINES - 1)
        .min(lines.len().saturating_sub(1));
    let mut end = declaration_index;
    for (index, line) in lines
        .iter()
        .enumerate()
        .take(max_end + 1)
        .skip(declaration_index + 1)
    {
        let trimmed = line.trim();
        if !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && runtime_smart_context_leading_whitespace(line) <= base_indent
        {
            break;
        }
        end = index;
    }
    end
}

fn runtime_smart_context_leading_whitespace(line: &str) -> usize {
    line.chars().take_while(|ch| ch.is_whitespace()).count()
}

fn runtime_smart_context_has_rust_test_attribute(lines: &[&str], declaration_index: usize) -> bool {
    let lower_bound =
        declaration_index.saturating_sub(RUNTIME_SMART_CONTEXT_MAX_SYMBOL_PREFIX_LINES);
    for line in lines[lower_bound..declaration_index].iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if runtime_smart_context_is_rust_test_attribute(trimmed) {
            return true;
        }
        if !trimmed.starts_with("#[") {
            break;
        }
    }
    false
}

fn runtime_smart_context_is_rust_test_attribute(line: &str) -> bool {
    line == "#[test]"
        || line.starts_with("#[tokio::test")
        || line.starts_with("#[async_std::test")
        || line.starts_with("#[rstest")
}

fn runtime_smart_context_parse_python_symbol(line: &str) -> Option<String> {
    let line = line
        .strip_prefix("async def ")
        .or_else(|| line.strip_prefix("def "))?;
    runtime_smart_context_take_identifier(line)
}

fn runtime_smart_context_parse_js_function_symbol(line: &str) -> Option<String> {
    if let Some(symbol) = runtime_smart_context_identifier_after_keyword(line, "function") {
        return Some(symbol);
    }
    let Some((left, right)) = line.split_once('=') else {
        return None;
    };
    if !right.contains("=>") && !right.trim_start().starts_with("function") {
        return None;
    }
    let name = left
        .split_whitespace()
        .last()
        .map(|name| name.trim_matches(|ch: char| ch == ':' || ch == '?'))?;
    runtime_smart_context_take_identifier(name)
}

fn runtime_smart_context_parse_js_test_symbol(line: &str) -> Option<String> {
    for prefix in ["test(", "it("] {
        if let Some(rest) = line.strip_prefix(prefix) {
            let label = rest
                .trim_start()
                .strip_prefix('"')
                .and_then(|rest| rest.split_once('"').map(|(label, _)| label))
                .or_else(|| {
                    rest.trim_start()
                        .strip_prefix('\'')
                        .and_then(|rest| rest.split_once('\'').map(|(label, _)| label))
                })
                .unwrap_or(prefix.trim_end_matches('('));
            return runtime_smart_context_bounded_string(label);
        }
    }
    None
}

fn runtime_smart_context_identifier_after_keyword(line: &str, keyword: &str) -> Option<String> {
    let (_, rest) = line.split_once(&format!("{keyword} "))?;
    runtime_smart_context_take_identifier(rest.trim_start_matches('*').trim_start())
}

fn runtime_smart_context_take_identifier(value: &str) -> Option<String> {
    let identifier = value
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(*ch, '_' | '$' | '#'))
        .collect::<String>();
    runtime_smart_context_bounded_string(identifier.trim_start_matches("r#"))
}

fn runtime_smart_context_parse_file_location(
    line: &str,
) -> Option<RuntimeSmartContextParsedFileLocation> {
    line.split_whitespace()
        .filter_map(runtime_smart_context_parse_file_location_token)
        .next()
}

fn runtime_smart_context_parse_file_location_token(
    token: &str,
) -> Option<RuntimeSmartContextParsedFileLocation> {
    let token = token
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
        })
        .trim_end_matches(':');
    let last_colon = token.rfind(':')?;
    let last_number = token[last_colon + 1..].parse::<usize>().ok()?;
    let prefix = &token[..last_colon];
    let (path, line, column) = if let Some(second_colon) = prefix.rfind(':') {
        if let Ok(line) = prefix[second_colon + 1..].parse::<usize>() {
            (&prefix[..second_colon], line, Some(last_number))
        } else {
            (prefix, last_number, None)
        }
    } else {
        (prefix, last_number, None)
    };
    let path = path
        .trim_start_matches("file://")
        .trim_start_matches("a/")
        .trim_start_matches("b/");
    if !runtime_smart_context_path_looks_like_file(path) {
        return None;
    }
    let path = runtime_smart_context_bounded_string(path)?;
    Some(RuntimeSmartContextParsedFileLocation { path, line, column })
}

fn runtime_smart_context_path_looks_like_file(path: &str) -> bool {
    if path.is_empty() || path.len() > RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_FIELD_BYTES {
        return false;
    }
    path.contains('/')
        || path.contains('\\')
        || path.rsplit_once('.').is_some_and(|(_, ext)| {
            matches!(
                ext,
                "rs" | "toml"
                    | "json"
                    | "md"
                    | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "py"
                    | "go"
                    | "java"
                    | "kt"
                    | "swift"
                    | "c"
                    | "cc"
                    | "cpp"
                    | "h"
                    | "hpp"
                    | "css"
                    | "scss"
                    | "html"
                    | "yml"
                    | "yaml"
                    | "sh"
                    | "bash"
                    | "zsh"
                    | "sql"
                    | "lock"
            )
        })
}

fn runtime_smart_context_parse_diff_file_path(line: &str) -> Option<String> {
    let path = line
        .strip_prefix("+++ ")
        .or_else(|| line.strip_prefix("--- "))?
        .split_whitespace()
        .next()?;
    if path == "/dev/null" {
        return None;
    }
    let path = path
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_matches('"');
    runtime_smart_context_bounded_string(path)
}

fn runtime_smart_context_parse_diff_hunk(line: &str) -> Option<RuntimeSmartContextParsedDiffHunk> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "@@" {
        return None;
    }
    let (old_start, old_count) = runtime_smart_context_parse_diff_span(parts.next()?, '-')?;
    let (new_start, new_count) = runtime_smart_context_parse_diff_span(parts.next()?, '+')?;
    Some(RuntimeSmartContextParsedDiffHunk {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

fn runtime_smart_context_parse_diff_span(span: &str, prefix: char) -> Option<(usize, usize)> {
    let span = span.strip_prefix(prefix)?;
    let mut parts = span.splitn(2, ',');
    let start = parts.next()?.parse::<usize>().ok()?;
    let count = parts
        .next()
        .map(|count| count.parse::<usize>().ok())
        .unwrap_or(Some(1))?;
    Some((start, count))
}

fn runtime_smart_context_diff_hunk_end(lines: &[&str], start_index: usize) -> usize {
    let max_end = (start_index + 24).min(lines.len().saturating_sub(1));
    for (index, line) in lines
        .iter()
        .enumerate()
        .take(max_end + 1)
        .skip(start_index + 1)
    {
        if line.starts_with("@@ ") || line.starts_with("diff --git ") {
            return index;
        }
        if !(line.starts_with(' ')
            || line.starts_with('+')
            || line.starts_with('-')
            || line.starts_with("\\ No newline"))
        {
            return index;
        }
    }
    max_end + 1
}

fn runtime_smart_context_is_test_failure_line(line: &str) -> bool {
    line.contains("test result: FAILED")
        || line == "failures:"
        || line.starts_with("failures:")
        || line.starts_with("FAIL ")
        || line.starts_with("FAILED ")
        || line.contains(" panicked at ")
        || (line.starts_with("---- ") && line.ends_with(" stdout ----"))
}

fn runtime_smart_context_parse_test_symbol(line: &str) -> Option<String> {
    if let Some(symbol) = line
        .strip_prefix("---- ")
        .and_then(|line| line.strip_suffix(" stdout ----"))
    {
        return runtime_smart_context_bounded_string(symbol);
    }
    if let Some(rest) = line.strip_prefix("thread '")
        && let Some((symbol, _)) = rest.split_once("' panicked at ")
    {
        return runtime_smart_context_bounded_string(symbol);
    }
    None
}

fn runtime_smart_context_parse_error_code(line: &str) -> Option<String> {
    if let Some(code) = runtime_smart_context_parse_bracketed_error_code(line) {
        return Some(code);
    }
    if line.contains("error:") || line.contains("Error:") || line.contains("ERROR") {
        return Some("error".to_string());
    }
    if let Some((_, rest)) = line.split_once("exit code ") {
        let code = rest.split_whitespace().next()?;
        return runtime_smart_context_bounded_string(&format!("exit_code_{code}"));
    }
    if let Some((_, rest)) = line.split_once("status code ") {
        let code = rest.split_whitespace().next()?;
        return runtime_smart_context_bounded_string(&format!("status_code_{code}"));
    }
    None
}

fn runtime_smart_context_parse_bracketed_error_code(line: &str) -> Option<String> {
    let start = line.find("error[")? + "error[".len();
    let rest = &line[start..];
    let end = rest.find(']')?;
    runtime_smart_context_bounded_string(&rest[..end])
}

fn runtime_smart_context_infer_command_kind(lines: &[&str]) -> Option<String> {
    let mut saw_diff = false;
    let mut saw_cargo_test = false;
    let mut saw_cargo_error = false;
    let mut saw_npm_test = false;
    for line in lines {
        if line.starts_with("diff --git ") || line.starts_with("@@ ") {
            saw_diff = true;
        }
        if line.contains("test result:") || line.starts_with("running ") && line.ends_with(" tests")
        {
            saw_cargo_test = true;
        }
        if line.contains("error: could not compile") {
            saw_cargo_error = true;
        }
        if line.starts_with("npm ERR!") || line.starts_with("FAIL ") {
            saw_npm_test = true;
        }
        if *line == "Traceback (most recent call last):" {
            return Some("python".to_string());
        }
    }
    if saw_cargo_test {
        Some("cargo-test".to_string())
    } else if saw_npm_test {
        Some("npm-test".to_string())
    } else if saw_cargo_error {
        Some("cargo-build".to_string())
    } else {
        saw_diff.then(|| "diff".to_string())
    }
}

fn runtime_smart_context_bounded_string(value: &str) -> Option<String> {
    (!value.is_empty() && value.len() <= RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_FIELD_BYTES)
        .then(|| value.to_string())
}

fn runtime_smart_context_semantic_index_complete_default() -> bool {
    true
}

fn runtime_smart_context_bool_is_true(value: &bool) -> bool {
    *value
}

fn runtime_smart_context_u8_is_zero(value: &u8) -> bool {
    *value == 0
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
    fn runtime_smart_context_artifact_save_persists_static_fingerprints() {
        let path = smart_context_artifact_temp_path("static-fingerprints");
        remove_smart_context_artifact_temp_files(&path);

        let mut store = RuntimeSmartContextArtifactStore::default();
        store.set_static_context_fingerprints(
            Some("scpc:1234".to_string()),
            vec![runtime_proxy_crate::SmartContextFingerprint {
                id: "instructions".to_string(),
                kind: runtime_proxy_crate::SmartContextFingerprintKind::StaticContext,
                content_hash: "hash-a".to_string(),
                byte_len: 42,
            }],
        );
        store.save_to_path(&path).expect("store saved");

        let loaded = RuntimeSmartContextArtifactStore::load_from_path(&path);
        assert_eq!(loaded.static_context_prompt_cache_hash(), Some("scpc:1234"));
        assert_eq!(loaded.static_context_fingerprints().len(), 1);
        assert_eq!(loaded.static_context_fingerprints()[0].id, "instructions");

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
    fn runtime_smart_context_artifact_insert_stores_semantic_line_index() {
        let text = "\
running 1 test
---- tests::keeps_failure_metadata stdout ----
thread 'tests::keeps_failure_metadata' panicked at src/main.rs:22:5:
error[E0277]: trait bound failed
 --> src/main.rs:22:5
--- a/src/main.rs
+++ b/src/main.rs
@@ -20,2 +20,3 @@ fn demo()
-old
+new
test result: FAILED. 0 passed; 1 failed";
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(1, text).expect("artifact inserted");

        let index = store
            .line_index(&artifact.id)
            .expect("new artifacts should carry a line index");
        assert_eq!(index.command_kind.as_deref(), Some("cargo-test"));
        assert!(
            index
                .file_location_ranges
                .iter()
                .any(|range| range.path.as_deref() == Some("src/main.rs")
                    && range.line == Some(22)
                    && range.column == Some(5))
        );
        assert!(index.diff_hunk_ranges.iter().any(|range| {
            range.path.as_deref() == Some("src/main.rs")
                && range.old_start == Some(20)
                && range.old_count == Some(2)
                && range.new_start == Some(20)
                && range.new_count == Some(3)
                && range.text.contains("+new")
        }));
        assert!(index.test_failure_ranges.iter().any(|range| {
            range.symbol.as_deref() == Some("tests::keeps_failure_metadata")
                || range.text.contains("test result: FAILED")
        }));
        assert!(
            index
                .error_ranges
                .iter()
                .any(|range| range.code.as_deref() == Some("E0277"))
        );
        for range in index
            .file_location_ranges
            .iter()
            .chain(index.diff_hunk_ranges.iter())
            .chain(index.test_failure_ranges.iter())
            .chain(index.error_ranges.iter())
        {
            assert_eq!(range.byte_len, range.text.len());
            assert_eq!(
                range.content_hash,
                runtime_proxy_crate::smart_context_hash_text(&range.text)
            );
            assert!(range.byte_len <= RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES);
        }
    }

    #[test]
    fn runtime_smart_context_artifact_semantic_line_index_is_bounded() {
        let text = (0..400)
            .map(|index| format!("src/file{index}.rs:{}:1: error[E0001]: failure", index + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(1, &text).expect("artifact inserted");

        let index = store
            .line_index(&artifact.id)
            .expect("new artifacts should carry a line index");
        let semantic_range_count = index.file_location_ranges.len()
            + index.diff_hunk_ranges.len()
            + index.test_failure_ranges.len()
            + index.error_ranges.len();
        assert!(semantic_range_count <= RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES);
        assert_eq!(
            semantic_range_count,
            RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES
        );
        assert!(!index.semantic_complete);
    }

    #[test]
    fn runtime_smart_context_artifact_insert_stores_chunk_fingerprints() {
        let text = "\
running 1 test
---- tests::stores_chunk_fingerprints stdout ----
thread 'tests::stores_chunk_fingerprints' panicked at src/main.rs:22:5:
error[E0277]: trait bound failed
 --> src/main.rs:22:5
--- a/src/main.rs
+++ b/src/main.rs
@@ -20,2 +20,3 @@ fn demo()
-old
+new
test result: FAILED. 0 passed; 1 failed";
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(1, text).expect("artifact inserted");

        let chunk_index = store
            .chunk_index(&artifact.id)
            .expect("new artifacts should carry a chunk index");
        assert!(chunk_index.complete);
        assert!(
            chunk_index
                .chunks
                .iter()
                .any(|chunk| chunk.kind == "file" && chunk.path.as_deref() == Some("src/main.rs"))
        );
        assert!(
            chunk_index
                .chunks
                .iter()
                .any(|chunk| chunk.kind == "diff" && chunk.path.as_deref() == Some("src/main.rs"))
        );
        assert!(chunk_index.chunks.iter().any(|chunk| {
            chunk.kind == "test"
                && chunk
                    .symbol
                    .as_deref()
                    .is_some_and(|symbol| symbol.contains("stores_chunk_fingerprints"))
        }));
        assert!(
            chunk_index
                .chunks
                .iter()
                .any(|chunk| chunk.kind == "error" && chunk.code.as_deref() == Some("E0277"))
        );

        let lines = text.lines().collect::<Vec<_>>();
        for chunk in &chunk_index.chunks {
            let excerpt = runtime_smart_context_line_excerpt(&lines, chunk.start, chunk.end)
                .expect("chunk excerpt should resolve");
            assert_eq!(chunk.byte_len, excerpt.len());
            assert_eq!(
                chunk.content_hash,
                runtime_proxy_crate::smart_context_hash_text(&excerpt)
            );
            assert!(chunk.byte_len <= RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES);
        }
    }

    #[test]
    fn runtime_smart_context_artifact_chunk_fingerprints_are_bounded() {
        let text = (0..400)
            .map(|index| format!("src/file{index}.rs:{}:1: error[E0001]: failure", index + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(1, &text).expect("artifact inserted");

        let chunk_index = store
            .chunk_index(&artifact.id)
            .expect("new artifacts should carry a chunk index");
        assert!(chunk_index.chunks.len() <= RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS);
        assert_eq!(
            chunk_index.chunks.len(),
            RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS
        );
        assert!(!chunk_index.complete);
    }

    #[test]
    fn runtime_smart_context_artifact_chunk_fingerprints_fall_back_to_line_windows() {
        let text = (1..=70)
            .map(|line| format!("plain output line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(1, &text).expect("artifact inserted");

        let chunk_index = store
            .chunk_index(&artifact.id)
            .expect("new artifacts should carry a chunk index");
        assert!(chunk_index.complete);
        assert_eq!(chunk_index.chunks.len(), 3);
        assert!(
            chunk_index
                .chunks
                .iter()
                .all(|chunk| chunk.kind == "window")
        );

        let lines = text.lines().collect::<Vec<_>>();
        let first = chunk_index.chunks.first().expect("first window chunk");
        assert_eq!(first.start, 1);
        assert_eq!(first.end, RUNTIME_SMART_CONTEXT_CHUNK_WINDOW_LINES);
        let excerpt = runtime_smart_context_line_excerpt(&lines, first.start, first.end)
            .expect("first window excerpt should resolve");
        assert_eq!(
            first.content_hash,
            runtime_proxy_crate::smart_context_hash_text(&excerpt)
        );
    }

    #[test]
    fn runtime_smart_context_artifact_duplicate_chunk_metadata_is_recorded() {
        let text = "\
error[E0001]: repeated failure
ok
error[E0001]: repeated failure
ok
error[E0001]: repeated failure";
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(1, text).expect("artifact inserted");

        let chunk_index = store
            .chunk_index(&artifact.id)
            .expect("new artifacts should carry a chunk index");
        let repeated_hash =
            runtime_proxy_crate::smart_context_hash_text("error[E0001]: repeated failure");
        let duplicate = chunk_index
            .duplicate_chunks
            .iter()
            .find(|duplicate| duplicate.content_hash == repeated_hash)
            .expect("repeated semantic chunks should be summarized");
        assert_eq!(duplicate.occurrence_count, 3);
        assert_eq!(duplicate.byte_len, "error[E0001]: repeated failure".len());
        assert_eq!(duplicate.occurrences.len(), 3);
        assert!(
            duplicate
                .occurrences
                .iter()
                .all(|occurrence| occurrence.kind == "error")
        );
        assert_eq!(
            duplicate
                .occurrences
                .iter()
                .map(|occurrence| occurrence.start)
                .collect::<Vec<_>>(),
            vec![1, 3, 5]
        );
    }

    #[test]
    fn runtime_smart_context_artifact_line_index_json_without_semantic_fields_still_loads() {
        let raw = serde_json::json!({
            "complete": true,
            "critical_ranges": [{
                "start": 1,
                "end": 1,
                "byte_len": 12,
                "content_hash": runtime_proxy_crate::smart_context_hash_text("error: old"),
                "text": "error: old"
            }]
        });

        let index: RuntimeSmartContextArtifactLineIndex =
            serde_json::from_value(raw).expect("legacy line index should deserialize");

        assert!(index.complete);
        assert!(index.semantic_complete);
        assert_eq!(index.critical_ranges.len(), 1);
        assert!(index.file_location_ranges.is_empty());
        assert!(index.diff_hunk_ranges.is_empty());
        assert!(index.test_failure_ranges.is_empty());
        assert!(index.error_ranges.is_empty());
        assert!(index.command_kind.is_none());

        let serialized = serde_json::to_value(&index).expect("line index should serialize");
        assert!(serialized.get("file_location_ranges").is_none());
        assert!(serialized.get("diff_hunk_ranges").is_none());
        assert!(serialized.get("test_failure_ranges").is_none());
        assert!(serialized.get("error_ranges").is_none());
        assert!(serialized.get("command_kind").is_none());
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
        assert!(store.chunk_index(&content_hash).is_none());

        store
            .insert_text(2, text)
            .expect("matching legacy artifact should refresh metadata");

        assert!(
            store
                .line_index(&content_hash)
                .is_some_and(|index| index.complete)
        );
        assert!(
            store
                .chunk_index(&content_hash)
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

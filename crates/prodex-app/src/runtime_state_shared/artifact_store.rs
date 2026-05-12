use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

static RUNTIME_SMART_CONTEXT_ARTIFACT_PROCESS_LOCKS: OnceLock<
    Mutex<BTreeMap<PathBuf, Arc<Mutex<()>>>>,
> = OnceLock::new();

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactRepoMap {
    #[serde(default)]
    pub(crate) complete: bool,
    #[serde(default)]
    pub(crate) entries: Vec<RuntimeSmartContextArtifactRepoMapEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextArtifactRepoMapEntry {
    pub(crate) kind: RuntimeSmartContextArtifactRepoMapEntryKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) line: Option<usize>,
    pub(crate) artifact_id: String,
    pub(crate) sequence: u64,
    pub(crate) range_start: usize,
    pub(crate) range_end: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeSmartContextArtifactRepoMapEntryKind {
    Path,
    Module,
    Symbol,
    Test,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeSmartContextArtifactProjectionKind {
    Repo,
    Symbol,
}

impl RuntimeSmartContextArtifactProjectionKind {
    fn includes_paths(self) -> bool {
        self == Self::Repo
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RuntimeSmartContextArtifactStore {
    pub(super) artifacts: BTreeMap<String, RuntimeSmartContextArtifact>,
    pub(super) total_bytes: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) static_context_fingerprints: Vec<RuntimeSmartContextStaticFingerprintMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) static_context_prompt_cache_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) repo_map_prewarm: Option<RuntimeSmartContextArtifactProjectionPrewarm>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) symbol_map_prewarm: Option<RuntimeSmartContextArtifactProjectionPrewarm>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct RuntimeSmartContextArtifactProjectionPrewarm {
    #[serde(default, skip_serializing_if = "runtime_smart_context_u8_is_zero")]
    pub(super) schema_version: u8,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(super) source_hash: String,
    #[serde(default)]
    pub(super) limit: usize,
    #[serde(default)]
    pub(super) projection: RuntimeSmartContextArtifactRepoMap,
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
        store.refresh_prewarmed_projections();
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

    #[allow(dead_code)]
    pub(crate) fn repo_map_projection(&self, limit: usize) -> RuntimeSmartContextArtifactRepoMap {
        self.map_projection_from_prewarm_or_build(
            limit,
            self.repo_map_prewarm.as_ref(),
            RuntimeSmartContextArtifactProjectionKind::Repo,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn symbol_map_projection(&self, limit: usize) -> RuntimeSmartContextArtifactRepoMap {
        self.map_projection_from_prewarm_or_build(
            limit,
            self.symbol_map_prewarm.as_ref(),
            RuntimeSmartContextArtifactProjectionKind::Symbol,
        )
    }

    fn map_projection_from_prewarm_or_build(
        &self,
        limit: usize,
        prewarm: Option<&RuntimeSmartContextArtifactProjectionPrewarm>,
        projection_kind: RuntimeSmartContextArtifactProjectionKind,
    ) -> RuntimeSmartContextArtifactRepoMap {
        let limit = limit.min(RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES);
        if limit == 0 {
            return RuntimeSmartContextArtifactRepoMap {
                complete: self.artifacts.is_empty(),
                entries: Vec::new(),
            };
        }

        if let Some(prewarm) = prewarm
            && self.projection_prewarm_valid(prewarm)
        {
            return runtime_smart_context_limited_repo_map(prewarm.projection.clone(), limit);
        }

        self.build_map_projection(limit, projection_kind)
    }

    fn build_map_projection(
        &self,
        limit: usize,
        projection_kind: RuntimeSmartContextArtifactProjectionKind,
    ) -> RuntimeSmartContextArtifactRepoMap {
        let mut entries = BTreeMap::<
            RuntimeSmartContextArtifactRepoMapKey,
            RuntimeSmartContextArtifactRepoMapEntry,
        >::new();
        let mut complete = true;
        let mut artifacts = self.artifacts.values().collect::<Vec<_>>();
        artifacts.sort_by(|left, right| {
            right
                .sequence
                .cmp(&left.sequence)
                .then_with(|| left.id.cmp(&right.id))
        });

        for artifact in artifacts {
            let Some(line_index) = artifact.line_index.as_ref() else {
                complete = false;
                continue;
            };
            complete &= line_index.semantic_complete && line_index.symbol_complete;

            let paths = runtime_smart_context_repo_map_paths(line_index);
            let primary_path = (paths.len() == 1).then(|| paths[0].clone());
            if projection_kind.includes_paths() {
                for path in paths {
                    let first_path_range =
                        runtime_smart_context_repo_map_first_path_range(line_index, &path);
                    runtime_smart_context_insert_repo_map_entry(
                        &mut entries,
                        RuntimeSmartContextArtifactRepoMapEntry {
                            kind: RuntimeSmartContextArtifactRepoMapEntryKind::Path,
                            module: runtime_smart_context_repo_map_module_from_path(&path),
                            symbol: None,
                            code: None,
                            line: first_path_range.and_then(|range| {
                                range.line.or(range.new_start).or(range.old_start)
                            }),
                            path: Some(path),
                            artifact_id: artifact.id.clone(),
                            sequence: artifact.sequence,
                            range_start: first_path_range.map_or(0, |range| range.start),
                            range_end: first_path_range.map_or(0, |range| range.end),
                        },
                    );
                }
            }

            for range in &line_index.symbol_ranges {
                let Some(symbol) = range.symbol.clone() else {
                    continue;
                };
                let kind = runtime_smart_context_repo_map_symbol_kind(range);
                let path = runtime_smart_context_repo_map_nearest_path(line_index, range.start)
                    .or_else(|| primary_path.clone());
                runtime_smart_context_insert_repo_map_entry(
                    &mut entries,
                    RuntimeSmartContextArtifactRepoMapEntry {
                        kind,
                        module: runtime_smart_context_repo_map_symbol_module(
                            kind,
                            path.as_deref(),
                            &symbol,
                        ),
                        symbol: Some(symbol),
                        code: None,
                        line: range.line,
                        path,
                        artifact_id: artifact.id.clone(),
                        sequence: artifact.sequence,
                        range_start: range.start,
                        range_end: range.end,
                    },
                );
            }

            for range in &line_index.error_ranges {
                let Some(code) = range.code.clone() else {
                    continue;
                };
                let path = runtime_smart_context_repo_map_nearest_path(line_index, range.start)
                    .or_else(|| primary_path.clone());
                runtime_smart_context_insert_repo_map_entry(
                    &mut entries,
                    RuntimeSmartContextArtifactRepoMapEntry {
                        kind: RuntimeSmartContextArtifactRepoMapEntryKind::Error,
                        module: path
                            .as_deref()
                            .and_then(runtime_smart_context_repo_map_module_from_path),
                        symbol: None,
                        code: Some(code),
                        line: Some(range.start),
                        path,
                        artifact_id: artifact.id.clone(),
                        sequence: artifact.sequence,
                        range_start: range.start,
                        range_end: range.end,
                    },
                );
            }
        }

        let mut entries = entries.into_values().collect::<Vec<_>>();
        if entries.len() > limit {
            entries.truncate(limit);
            complete = false;
        }

        RuntimeSmartContextArtifactRepoMap { complete, entries }
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
        if self.artifacts.contains_key(&id) {
            let (artifact_ref, projection_dirty) = {
                let existing = self.artifacts.get_mut(&id)?;
                if !Self::artifact_matches_text(existing, text, &content_hash) {
                    return None;
                }
                let mut projection_dirty = existing.sequence != sequence;
                existing.sequence = sequence;
                let refresh_line_index = runtime_smart_context_artifact_line_index_needs_refresh(
                    existing.line_index.as_ref(),
                );
                if refresh_line_index || existing.chunk_index.is_none() {
                    let line_index = if refresh_line_index {
                        runtime_smart_context_artifact_line_index(text)
                    } else if let Some(line_index) = existing.line_index.clone() {
                        line_index
                    } else {
                        projection_dirty = true;
                        runtime_smart_context_artifact_line_index(text)
                    };
                    if refresh_line_index || existing.line_index.is_none() {
                        existing.line_index = Some(line_index.clone());
                        projection_dirty = true;
                    }
                    if refresh_line_index || existing.chunk_index.is_none() {
                        existing.chunk_index = Some(runtime_smart_context_artifact_chunk_index(
                            text,
                            &line_index,
                        ));
                    }
                }
                (Self::artifact_ref(existing), projection_dirty)
            };
            if projection_dirty {
                self.invalidate_prewarmed_projections();
            }
            return Some(artifact_ref);
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
        self.invalidate_prewarmed_projections();
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
        self.refresh_prewarmed_projections();
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
                    artifact.line_index.clone().unwrap_or_else(|| {
                        runtime_smart_context_artifact_line_index(&artifact.text)
                    })
                };
                if refresh_line_index || artifact.line_index.is_none() {
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

    fn invalidate_prewarmed_projections(&mut self) {
        self.repo_map_prewarm = None;
        self.symbol_map_prewarm = None;
    }

    fn refresh_prewarmed_projections(&mut self) {
        let source_hash = self.projection_source_hash();
        let repo_valid = self
            .repo_map_prewarm
            .as_ref()
            .is_some_and(|prewarm| self.projection_prewarm_valid_for_source(prewarm, &source_hash));
        if !repo_valid {
            self.repo_map_prewarm = Some(RuntimeSmartContextArtifactProjectionPrewarm {
                schema_version: RUNTIME_SMART_CONTEXT_REPO_MAP_PREWARM_SCHEMA_VERSION,
                source_hash: source_hash.clone(),
                limit: RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES,
                projection: self.build_map_projection(
                    RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES,
                    RuntimeSmartContextArtifactProjectionKind::Repo,
                ),
            });
        }

        let symbol_valid = self
            .symbol_map_prewarm
            .as_ref()
            .is_some_and(|prewarm| self.projection_prewarm_valid_for_source(prewarm, &source_hash));
        if !symbol_valid {
            self.symbol_map_prewarm = Some(RuntimeSmartContextArtifactProjectionPrewarm {
                schema_version: RUNTIME_SMART_CONTEXT_REPO_MAP_PREWARM_SCHEMA_VERSION,
                source_hash,
                limit: RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES,
                projection: self.build_map_projection(
                    RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES,
                    RuntimeSmartContextArtifactProjectionKind::Symbol,
                ),
            });
        }
    }

    fn projection_prewarm_valid(
        &self,
        prewarm: &RuntimeSmartContextArtifactProjectionPrewarm,
    ) -> bool {
        let source_hash = self.projection_source_hash();
        self.projection_prewarm_valid_for_source(prewarm, &source_hash)
    }

    fn projection_prewarm_valid_for_source(
        &self,
        prewarm: &RuntimeSmartContextArtifactProjectionPrewarm,
        source_hash: &str,
    ) -> bool {
        prewarm.schema_version == RUNTIME_SMART_CONTEXT_REPO_MAP_PREWARM_SCHEMA_VERSION
            && prewarm.limit >= RUNTIME_SMART_CONTEXT_MAX_REPO_MAP_ENTRIES
            && prewarm.source_hash == source_hash
    }

    fn projection_source_hash(&self) -> String {
        let mut source = String::new();
        let _ = writeln!(
            source,
            "schema={}",
            RUNTIME_SMART_CONTEXT_REPO_MAP_PREWARM_SCHEMA_VERSION
        );
        for artifact in self.artifacts.values() {
            let _ = writeln!(
                source,
                "artifact\t{}\t{}\t{}\t{}",
                artifact.id, artifact.content_hash, artifact.byte_len, artifact.sequence
            );
            let Some(line_index) = artifact.line_index.as_ref() else {
                source.push_str("line_index\tmissing\n");
                continue;
            };
            let _ = writeln!(
                source,
                "line_index\t{}\t{}\t{}\t{}\t{}",
                line_index.semantic_schema_version,
                line_index.complete,
                line_index.semantic_complete,
                line_index.symbol_complete,
                line_index.command_kind.as_deref().unwrap_or_default()
            );
            runtime_smart_context_push_projection_source_ranges(
                &mut source,
                "file",
                &line_index.file_location_ranges,
            );
            runtime_smart_context_push_projection_source_ranges(
                &mut source,
                "diff",
                &line_index.diff_hunk_ranges,
            );
            runtime_smart_context_push_projection_source_ranges(
                &mut source,
                "test",
                &line_index.test_failure_ranges,
            );
            runtime_smart_context_push_projection_source_ranges(
                &mut source,
                "error",
                &line_index.error_ranges,
            );
            runtime_smart_context_push_projection_source_ranges(
                &mut source,
                "symbol",
                &line_index.symbol_ranges,
            );
        }
        runtime_proxy_crate::smart_context_hash_text(&source)
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

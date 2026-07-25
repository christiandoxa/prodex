use super::super::{
    RUNTIME_SMART_CONTEXT_MAX_ARTIFACTS, RUNTIME_SMART_CONTEXT_MAX_TOTAL_BYTES,
    runtime_smart_context_artifact_chunk_index, runtime_smart_context_artifact_line_index,
    runtime_smart_context_artifact_line_index_needs_refresh,
};
use super::RuntimeSmartContextArtifactStore;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read as _, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

const RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

static RUNTIME_SMART_CONTEXT_ARTIFACT_PROCESS_LOCKS: OnceLock<
    Mutex<BTreeMap<PathBuf, Weak<Mutex<()>>>>,
> = OnceLock::new();

impl RuntimeSmartContextArtifactStore {
    pub(crate) fn load_from_path(path: &Path) -> Self {
        let Some(raw) = runtime_smart_context_read_artifact_store(path) else {
            return Self::default();
        };
        let Ok(mut store) = serde_json::from_str::<Self>(&raw) else {
            return Self::default();
        };
        if !matches!(store.schema_version, 1 | 2) {
            return Self::default();
        }
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

    fn merge_from(&mut self, incoming: &Self) {
        self.next_artifact_order = self.next_artifact_order.max(
            self.artifacts
                .values()
                .map(|artifact| artifact.order)
                .max()
                .unwrap_or(0),
        );
        for (id, incoming_artifact) in incoming
            .artifacts
            .iter()
            .filter(|(_, artifact)| !artifact.pending_order)
        {
            self.artifacts
                .entry(id.clone())
                .and_modify(|current| {
                    if incoming_artifact.order >= current.order {
                        *current = incoming_artifact.clone();
                    }
                })
                .or_insert_with(|| incoming_artifact.clone());
        }
        let mut pending = incoming
            .artifacts
            .values()
            .filter(|artifact| artifact.pending_order)
            .cloned()
            .collect::<Vec<_>>();
        pending.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.id.cmp(&right.id))
        });
        for mut artifact in pending {
            self.next_artifact_order = self.next_artifact_order.saturating_add(1);
            artifact.order = self.next_artifact_order;
            artifact.pending_order = false;
            self.artifacts.insert(artifact.id.clone(), artifact);
        }
        self.legacy_artifact_ids
            .extend(incoming.legacy_artifact_ids.clone());
        self.legacy_artifact_ids
            .retain(|_, id| self.artifacts.contains_key(id));
        self.schema_version = 2;
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

    pub(crate) fn apply_persisted_artifact_ordering(
        &mut self,
        submitted: &Self,
        persisted: &Self,
    ) -> Vec<String> {
        let mut projection_dirty = false;
        let mut durable_ids = Vec::new();
        for (id, persisted_artifact) in &persisted.artifacts {
            let Some(current) = self.artifacts.get_mut(id) else {
                continue;
            };
            if current.content_hash != persisted_artifact.content_hash
                || current.byte_len != persisted_artifact.byte_len
                || current.text != persisted_artifact.text
            {
                continue;
            }
            durable_ids.push(id.clone());
            let Some(submitted_artifact) = submitted.artifacts.get(id) else {
                continue;
            };
            if current.pending_order
                && submitted_artifact.pending_order
                && current.order == submitted_artifact.order
            {
                projection_dirty |= current.order != persisted_artifact.order;
                current.order = persisted_artifact.order;
                current.pending_order = false;
            }
        }
        self.next_artifact_order = self.next_artifact_order.max(persisted.next_artifact_order);
        if projection_dirty {
            self.invalidate_prewarmed_projections();
        }
        durable_ids
    }

    fn write_to_path_unlocked(&self, path: &Path) -> anyhow::Result<()> {
        let raw = serde_json::to_vec(self)?;
        let temp_path = crate::runtime_store::unique_state_temp_file_path(path);
        runtime_smart_context_write_private_file(&temp_path, &raw)?;
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
        let mut validated = BTreeMap::new();
        for (stored_id, mut artifact) in std::mem::take(&mut self.artifacts) {
            if stored_id != artifact.id
                || artifact.id != artifact.content_hash
                || artifact.byte_len != artifact.text.len()
                || !runtime_proxy_crate::smart_context_hash_matches_text(
                    &artifact.content_hash,
                    &artifact.text,
                )
            {
                continue;
            }
            let strong_id = runtime_proxy_crate::smart_context_hash_text(&artifact.text);
            if artifact.id != strong_id {
                self.legacy_artifact_ids
                    .insert(artifact.id.clone(), strong_id.clone());
                artifact.id = strong_id.clone();
                artifact.content_hash = strong_id.clone();
                artifact.line_index = None;
                artifact.chunk_index = None;
            }
            validated
                .entry(strong_id)
                .and_modify(|current: &mut super::super::RuntimeSmartContextArtifact| {
                    if artifact.order >= current.order {
                        *current = artifact.clone();
                    }
                })
                .or_insert(artifact);
        }
        self.artifacts = validated;
        self.schema_version = 2;

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
            !fingerprint.id.trim().is_empty() && fingerprint.content_hash.starts_with("sc2:")
        });
        self.legacy_artifact_ids
            .retain(|legacy, id| legacy.starts_with("sc:") && self.artifacts.contains_key(id));
        self.next_artifact_order = self.next_artifact_order.max(
            self.artifacts
                .values()
                .map(|artifact| artifact.order)
                .max()
                .unwrap_or(0),
        );
    }

    pub(in crate::runtime_state_shared::artifact_store) fn enforce_limits(&mut self) {
        while self.artifacts.len() > RUNTIME_SMART_CONTEXT_MAX_ARTIFACTS
            || self.total_bytes > RUNTIME_SMART_CONTEXT_MAX_TOTAL_BYTES
        {
            let Some(oldest_id) = self
                .artifacts
                .values()
                .min_by(|left, right| {
                    left.order
                        .cmp(&right.order)
                        .then_with(|| left.id.cmp(&right.id))
                })
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

fn runtime_smart_context_read_artifact_store(path: &Path) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file()
        || metadata.len() > RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES
    {
        return None;
    }

    let file = fs::File::open(path).ok()?;
    let opened_metadata = file.metadata().ok()?;
    if !runtime_smart_context_same_artifact_store_file(&metadata, &opened_metadata) {
        return None;
    }

    let mut raw = String::new();
    if file
        .take(RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES.saturating_add(1))
        .read_to_string(&mut raw)
        .is_err()
    {
        return None;
    }
    if raw.len() as u64 > RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES {
        return None;
    }
    Some(raw)
}

#[cfg(unix)]
fn runtime_smart_context_same_artifact_store_file(
    before: &fs::Metadata,
    after: &fs::Metadata,
) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn runtime_smart_context_same_artifact_store_file(
    _before: &fs::Metadata,
    _after: &fs::Metadata,
) -> bool {
    true
}

fn runtime_smart_context_write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = runtime_smart_context_open_private_file(path)?;
    file.write_all(bytes)
}

#[cfg(unix)]
fn runtime_smart_context_open_private_file(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn runtime_smart_context_open_private_file(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

fn runtime_smart_context_artifact_process_lock(path: &Path) -> Arc<Mutex<()>> {
    let locks =
        RUNTIME_SMART_CONTEXT_ARTIFACT_PROCESS_LOCKS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut locks = locks
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(path).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(path.to_path_buf(), Arc::downgrade(&lock));
    lock
}

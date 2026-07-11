use super::super::{
    RUNTIME_SMART_CONTEXT_MAX_ARTIFACT_BYTES, RuntimeSmartContextArtifact,
    RuntimeSmartContextArtifactChunkIndex, RuntimeSmartContextArtifactLineIndex,
    RuntimeSmartContextArtifactManifestEntry, runtime_smart_context_artifact_chunk_index,
    runtime_smart_context_artifact_line_index,
    runtime_smart_context_artifact_line_index_needs_refresh,
};
use super::{RuntimeSmartContextArtifactStore, RuntimeSmartContextStaticFingerprintMetadata};

impl RuntimeSmartContextArtifactStore {
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

    pub(in crate::runtime_state_shared::artifact_store) fn artifact_matches_text(
        artifact: &RuntimeSmartContextArtifact,
        text: &str,
        content_hash: &str,
    ) -> bool {
        artifact.content_hash == content_hash
            && artifact.byte_len == text.len()
            && artifact.text == text
    }

    pub(in crate::runtime_state_shared::artifact_store) fn artifact_ref(
        artifact: &RuntimeSmartContextArtifact,
    ) -> runtime_proxy_crate::SmartContextArtifactRef {
        runtime_proxy_crate::SmartContextArtifactRef {
            id: artifact.id.clone(),
            byte_len: artifact.byte_len,
            content_hash: artifact.content_hash.clone(),
        }
    }
}

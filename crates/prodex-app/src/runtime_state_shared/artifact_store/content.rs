#[cfg(any(test, feature = "bench-support"))]
use super::super::{
    RuntimeSmartContextArtifact, RuntimeSmartContextArtifactChunkIndex,
    RuntimeSmartContextArtifactLineIndex, runtime_smart_context_artifact_chunk_index,
    runtime_smart_context_artifact_line_index,
};
use super::RuntimeSmartContextArtifactStore;
#[cfg(test)]
use super::RuntimeSmartContextStaticFingerprintMetadata;
#[cfg(not(test))]
use super::types::RuntimeSmartContextStaticFingerprintMetadata;

impl RuntimeSmartContextArtifactStore {
    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn insert_text(
        &mut self,
        text: &str,
    ) -> Option<runtime_proxy_crate::SmartContextArtifactRef> {
        let content_hash = runtime_proxy_crate::smart_context_hash_text(text);
        let id = content_hash.clone();
        if let Some(existing) = self.artifacts.get_mut(&id) {
            if existing.line_index.is_none() {
                existing.line_index = Some(runtime_smart_context_artifact_line_index(text));
            }
            if existing.chunk_index.is_none() {
                let line_index = existing
                    .line_index
                    .clone()
                    .unwrap_or_else(|| runtime_smart_context_artifact_line_index(text));
                existing.chunk_index = Some(runtime_smart_context_artifact_chunk_index(
                    text,
                    &line_index,
                ));
            }
            return Some(runtime_proxy_crate::SmartContextArtifactRef {
                id: existing.id.clone(),
                byte_len: existing.byte_len,
                content_hash: existing.content_hash.clone(),
            });
        }
        let order = self
            .artifacts
            .values()
            .map(|artifact| artifact.order)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let line_index = runtime_smart_context_artifact_line_index(text);
        let chunk_index = runtime_smart_context_artifact_chunk_index(text, &line_index);
        let byte_len = text.len();
        self.artifacts.insert(
            id.clone(),
            RuntimeSmartContextArtifact {
                id: id.clone(),
                byte_len,
                content_hash: content_hash.clone(),
                text: text.to_string(),
                order,
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

    #[cfg(test)]
    pub(crate) fn artifact_ref_for_exact_text(
        &self,
        text: &str,
    ) -> Option<runtime_proxy_crate::SmartContextArtifactRef> {
        let content_hash = runtime_proxy_crate::smart_context_hash_text(text);
        let artifact = self.artifacts.get(&content_hash)?;
        (artifact.content_hash == content_hash
            && artifact.byte_len == text.len()
            && artifact.text == text)
            .then(|| runtime_proxy_crate::SmartContextArtifactRef {
                id: artifact.id.clone(),
                byte_len: artifact.byte_len,
                content_hash: artifact.content_hash.clone(),
            })
    }

    #[cfg(test)]
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

    pub(crate) fn static_context_prompt_cache_hash(&self) -> Option<&str> {
        self.static_context_prompt_cache_hash.as_deref()
    }

    pub(crate) fn get_text(&self, id: &str) -> Option<String> {
        self.artifacts
            .get(self.resolve_artifact_id(id))
            .map(|artifact| artifact.text.clone())
    }

    #[cfg(test)]
    pub(crate) fn line_index(&self, id: &str) -> Option<&RuntimeSmartContextArtifactLineIndex> {
        self.artifacts
            .get(self.resolve_artifact_id(id))
            .and_then(|artifact| artifact.line_index.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn chunk_index(&self, id: &str) -> Option<&RuntimeSmartContextArtifactChunkIndex> {
        self.artifacts
            .get(self.resolve_artifact_id(id))
            .and_then(|artifact| artifact.chunk_index.as_ref())
    }

    pub(crate) fn contains(&self, id: &str) -> bool {
        self.artifacts.contains_key(self.resolve_artifact_id(id))
    }

    fn resolve_artifact_id<'a>(&'a self, id: &'a str) -> &'a str {
        self.legacy_artifact_ids
            .get(id)
            .map(String::as_str)
            .unwrap_or(id)
    }
}

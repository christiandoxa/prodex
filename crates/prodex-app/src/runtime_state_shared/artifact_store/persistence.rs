use super::super::{
    RUNTIME_SMART_CONTEXT_MAX_ARTIFACTS, RUNTIME_SMART_CONTEXT_MAX_TOTAL_BYTES,
    runtime_smart_context_artifact_chunk_index, runtime_smart_context_artifact_line_index,
    runtime_smart_context_artifact_line_index_needs_refresh,
};
use super::RuntimeSmartContextArtifactStore;
use anyhow::{Context, bail};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

const RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const RUNTIME_SMART_CONTEXT_ARTIFACT_KEY_BYTES: usize = 32;
const RUNTIME_SMART_CONTEXT_ARTIFACT_ENCRYPTED_MAGIC: &[u8] = b"PSCA1\0";

impl RuntimeSmartContextArtifactStore {
    pub(crate) fn for_scope(scope: runtime_proxy_crate::ContextScopeId) -> Self {
        Self {
            scope_id: Some(scope),
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn load_from_path(path: &Path) -> Self {
        Self::load_validated_from_path(path, None).unwrap_or_default()
    }

    pub(crate) fn load_scoped_from_path(
        path: &Path,
        scope: &runtime_proxy_crate::ContextScopeId,
    ) -> anyhow::Result<Self> {
        Self::load_validated_from_path(path, Some(scope))
    }

    fn load_validated_from_path(
        path: &Path,
        expected_scope: Option<&runtime_proxy_crate::ContextScopeId>,
    ) -> anyhow::Result<Self> {
        let Some(raw) = runtime_smart_context_read_artifact_store(path, expected_scope)? else {
            return Ok(expected_scope
                .cloned()
                .map(Self::for_scope)
                .unwrap_or_default());
        };
        let mut store = serde_json::from_slice::<Self>(&raw).with_context(|| {
            format!("invalid Smart Context artifact JSON at {}", path.display())
        })?;
        if !matches!(store.schema_version, 1..=3) {
            bail!(
                "unsupported Smart Context artifact schema at {}",
                path.display()
            );
        }
        if store.schema_version == 3
            && expected_scope.is_some()
            && store.scope_id.as_ref() != expected_scope
        {
            bail!(
                "Smart Context artifact scope mismatch at {}",
                path.display()
            );
        }
        if !store.validate_loaded_metadata() {
            bail!(
                "invalid Smart Context artifact metadata at {}",
                path.display()
            );
        }
        store.scope_id = expected_scope.cloned().or(store.scope_id);
        store.schema_version = 3;
        store.recompute_total_bytes();
        store.enforce_limits();
        store.refresh_prewarmed_projections();
        Ok(store)
    }

    fn recompute_total_bytes(&mut self) {
        self.total_bytes = self
            .artifacts
            .values()
            .map(|artifact| artifact.byte_len)
            .sum();
    }

    fn validate_loaded_metadata(&mut self) -> bool {
        let mut valid = self.rebuild_validated_artifacts();
        self.refresh_loaded_artifact_indexes();
        self.schema_version = 3;

        let fingerprint_count = self.static_context_fingerprints.len();
        self.static_context_fingerprints.retain(|fingerprint| {
            !fingerprint.id.trim().is_empty() && fingerprint.content_hash.starts_with("sc2:")
        });
        valid &= self.static_context_fingerprints.len() == fingerprint_count;
        self.legacy_artifact_ids
            .retain(|legacy, id| legacy.starts_with("sc:") && self.artifacts.contains_key(id));
        valid
    }

    fn rebuild_validated_artifacts(&mut self) -> bool {
        let mut valid = true;
        let mut validated = BTreeMap::new();
        for (stored_id, mut artifact) in std::mem::take(&mut self.artifacts) {
            if !valid_loaded_artifact(&stored_id, &artifact) {
                valid = false;
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
        valid
    }

    fn refresh_loaded_artifact_indexes(&mut self) {
        for artifact in self.artifacts.values_mut() {
            refresh_loaded_artifact_index(artifact);
        }
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

fn refresh_loaded_artifact_index(artifact: &mut super::super::RuntimeSmartContextArtifact) {
    let refresh_line_index =
        runtime_smart_context_artifact_line_index_needs_refresh(artifact.line_index.as_ref());
    if !refresh_line_index && artifact.chunk_index.is_some() {
        return;
    }
    let line_index = if refresh_line_index {
        runtime_smart_context_artifact_line_index(&artifact.text)
    } else {
        artifact
            .line_index
            .clone()
            .unwrap_or_else(|| runtime_smart_context_artifact_line_index(&artifact.text))
    };
    if refresh_line_index || artifact.line_index.is_none() {
        artifact.line_index = Some(line_index.clone());
    }
    artifact.chunk_index = Some(runtime_smart_context_artifact_chunk_index(
        &artifact.text,
        &line_index,
    ));
}

fn valid_loaded_artifact(
    stored_id: &str,
    artifact: &super::super::RuntimeSmartContextArtifact,
) -> bool {
    stored_id == artifact.id
        && artifact.id == artifact.content_hash
        && artifact.byte_len == artifact.text.len()
        && runtime_proxy_crate::smart_context_hash_matches_text(
            &artifact.content_hash,
            &artifact.text,
        )
}

fn runtime_smart_context_read_artifact_store(
    path: &Path,
    expected_scope: Option<&runtime_proxy_crate::ContextScopeId>,
) -> anyhow::Result<Option<Zeroizing<Vec<u8>>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_file()
        || metadata.len() > RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES
    {
        bail!(
            "unsafe Smart Context artifact store path {}",
            path.display()
        );
    }

    let file = prodex_core::open_regular_file_no_follow(path)?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file)? {
        bail!(
            "Smart Context artifact store changed while opening {}",
            path.display()
        );
    }

    let mut raw = Zeroizing::new(Vec::with_capacity(metadata.len() as usize));
    file.take(RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut raw)
        .with_context(|| {
            format!(
                "failed to read Smart Context artifacts at {}",
                path.display()
            )
        })?;
    if raw.len() as u64 > RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES {
        bail!(
            "Smart Context artifact store exceeds size limit at {}",
            path.display()
        );
    }
    if raw.starts_with(RUNTIME_SMART_CONTEXT_ARTIFACT_ENCRYPTED_MAGIC) {
        return runtime_smart_context_decrypt_artifact_store(path, expected_scope, &raw).map(Some);
    }
    Ok(Some(raw))
}

fn runtime_smart_context_decrypt_artifact_store(
    path: &Path,
    scope: Option<&runtime_proxy_crate::ContextScopeId>,
    encoded: &[u8],
) -> anyhow::Result<Zeroizing<Vec<u8>>> {
    let key_path = runtime_smart_context_artifact_key_path(path)
        .context("encrypted Smart Context artifact path is outside the scoped store")?;
    let scope = scope.context("encrypted Smart Context artifact store has no expected scope")?;
    let ciphertext_start = RUNTIME_SMART_CONTEXT_ARTIFACT_ENCRYPTED_MAGIC.len();
    let key = runtime_smart_context_artifact_key(&key_path, false)?;
    secret_store::decrypt_private_payload(
        key.as_slice(),
        scope.as_str().as_bytes(),
        &encoded[ciphertext_start..],
    )
    .map_err(Into::into)
}

fn runtime_smart_context_artifact_key_path(path: &Path) -> Option<PathBuf> {
    let scope_dir = path.parent()?;
    let scopes_dir = scope_dir.parent()?;
    let smart_context_dir = scopes_dir.parent()?;
    (path.file_name()?.to_str()? == "artifacts.json"
        && scopes_dir.file_name()?.to_str()? == "scopes"
        && smart_context_dir.file_name()?.to_str()? == "smart-context")
        .then(|| smart_context_dir.join("artifact-store.key"))
}

fn runtime_smart_context_artifact_key(
    path: &Path,
    create: bool,
) -> anyhow::Result<Zeroizing<Vec<u8>>> {
    if let Some(parent) = path.parent() {
        secret_store::ensure_private_directory(parent)?;
    }
    let _lock = crate::runtime_store::acquire_json_file_lock(path)?;
    if let Some(key) = secret_store::read_private_file_bounded(
        path,
        RUNTIME_SMART_CONTEXT_ARTIFACT_KEY_BYTES as u64,
    )? {
        if key.len() != RUNTIME_SMART_CONTEXT_ARTIFACT_KEY_BYTES {
            bail!("invalid Smart Context artifact encryption key length");
        }
        return Ok(key);
    }
    if !create {
        bail!("Smart Context artifact encryption key is unavailable");
    }
    let mut key = Zeroizing::new(vec![0_u8; RUNTIME_SMART_CONTEXT_ARTIFACT_KEY_BYTES]);
    getrandom::fill(key.as_mut_slice())
        .map_err(|_| anyhow::anyhow!("failed to generate Smart Context artifact key"))?;
    secret_store::write_private_file_atomic(path, &key)?;
    Ok(key)
}

use super::super::{RuntimeSmartContextArtifact, runtime_smart_context_u8_is_zero};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    #[serde(alias = "sequence")]
    pub(crate) order: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeSmartContextArtifactStore {
    #[serde(default = "runtime_smart_context_legacy_artifact_store_schema_version")]
    pub(in crate::runtime_state_shared) schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::runtime_state_shared) scope_id: Option<runtime_proxy_crate::ContextScopeId>,
    #[serde(default)]
    pub(in crate::runtime_state_shared) next_artifact_order: u64,
    pub(in crate::runtime_state_shared) artifacts: BTreeMap<String, RuntimeSmartContextArtifact>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(in crate::runtime_state_shared) legacy_artifact_ids: BTreeMap<String, String>,
    pub(in crate::runtime_state_shared) total_bytes: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(in crate::runtime_state_shared) static_context_fingerprints:
        Vec<RuntimeSmartContextStaticFingerprintMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::runtime_state_shared) static_context_prompt_cache_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::runtime_state_shared) repo_map_prewarm:
        Option<RuntimeSmartContextArtifactProjectionPrewarm>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::runtime_state_shared) symbol_map_prewarm:
        Option<RuntimeSmartContextArtifactProjectionPrewarm>,
}

impl Default for RuntimeSmartContextArtifactStore {
    fn default() -> Self {
        Self {
            schema_version: 3,
            scope_id: None,
            next_artifact_order: 0,
            artifacts: BTreeMap::new(),
            legacy_artifact_ids: BTreeMap::new(),
            total_bytes: 0,
            static_context_fingerprints: Vec::new(),
            static_context_prompt_cache_hash: None,
            repo_map_prewarm: None,
            symbol_map_prewarm: None,
        }
    }
}

fn runtime_smart_context_legacy_artifact_store_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::runtime_state_shared) struct RuntimeSmartContextArtifactProjectionPrewarm {
    #[serde(default, skip_serializing_if = "runtime_smart_context_u8_is_zero")]
    pub(in crate::runtime_state_shared) schema_version: u8,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(in crate::runtime_state_shared) source_hash: String,
    #[serde(default)]
    pub(in crate::runtime_state_shared) limit: usize,
    #[serde(default)]
    pub(in crate::runtime_state_shared) projection: RuntimeSmartContextArtifactRepoMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextStaticFingerprintMetadata {
    pub(crate) id: String,
    pub(crate) content_hash: String,
    pub(crate) byte_len: usize,
}

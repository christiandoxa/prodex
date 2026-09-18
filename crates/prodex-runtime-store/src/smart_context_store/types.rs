use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeSmartContextArtifact {
    pub key: String,
    pub content_hash: String,
    pub byte_len: usize,
    pub created_at: i64,
    pub last_accessed_at: i64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSmartContextArtifactStore {
    pub version: u32,
    pub artifacts: BTreeMap<String, RuntimeSmartContextArtifact>,
}

impl Default for RuntimeSmartContextArtifactStore {
    fn default() -> Self {
        Self {
            version: crate::RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION,
            artifacts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSmartContextLineRange {
    pub start_line: usize,
    pub end_line: usize,
}

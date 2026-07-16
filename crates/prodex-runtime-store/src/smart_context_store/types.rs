use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSmartContextArtifactStorePolicy {
    pub ttl_seconds: i64,
    pub max_entries: usize,
}

impl Default for RuntimeSmartContextArtifactStorePolicy {
    fn default() -> Self {
        Self {
            ttl_seconds: 24 * 60 * 60,
            max_entries: 1_024,
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSmartContextExtractedLineRange {
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSmartContextStaleContextSnapshot<'a> {
    pub hash: Option<&'a str>,
    pub byte_len: usize,
    pub token_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSmartContextStaleContextPruningInput<'a> {
    pub previous: Option<RuntimeSmartContextStaleContextSnapshot<'a>>,
    pub current: RuntimeSmartContextStaleContextSnapshot<'a>,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSmartContextStaleContextPruningKind {
    TooSmall,
    NoPrevious,
    ExactReuse,
    Changed,
}

impl RuntimeSmartContextStaleContextPruningKind {
    pub fn can_prune_payload(self) -> bool {
        matches!(self, Self::ExactReuse | Self::Changed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSmartContextStaleContextPruningDecision {
    pub kind: RuntimeSmartContextStaleContextPruningKind,
    pub summary: String,
    pub previous_hash: Option<String>,
    pub current_hash: Option<String>,
    pub previous_byte_len: Option<usize>,
    pub current_byte_len: usize,
    pub previous_token_len: Option<usize>,
    pub current_token_len: usize,
    pub reusable_byte_len: usize,
    pub reusable_token_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSmartContextArtifactStoreJsonError {
    pub message: String,
}

impl RuntimeSmartContextArtifactStoreJsonError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RuntimeSmartContextArtifactStoreJsonError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeSmartContextArtifactStoreJsonError {}

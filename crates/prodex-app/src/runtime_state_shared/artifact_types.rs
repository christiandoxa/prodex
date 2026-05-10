use super::*;

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

use super::artifact_refs::RuntimeSmartContextArtifactReference;
use super::repo_state::RuntimeSmartContextRepoStateFacts;
use super::rewrite_telemetry::RuntimeSmartContextRewriteTelemetryRecord;
use super::static_context::RuntimeSmartContextStaticSectionFingerprint;
use super::token_calibration::RuntimeSmartContextTokenCalibrationObservation;
use crate::runtime_state_shared::{
    RuntimeSmartContextArtifactChunkIndex, RuntimeSmartContextArtifactLineIndex,
    RuntimeSmartContextArtifactStore,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeSmartContextTransport {
    Http,
    Websocket,
}

impl RuntimeSmartContextTransport {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Websocket => "websocket",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextTransformStats {
    pub(super) artifacts_stored: usize,
    pub(super) tool_outputs_condensed: usize,
    pub(super) tool_call_args_condensed: usize,
    pub(super) duplicate_texts: usize,
    pub(super) cross_turn_duplicate_texts: usize,
    pub(super) repeat_tool_output_refs: usize,
    pub(super) blob_outputs_condensed: usize,
    pub(super) rehydrated_refs: usize,
    pub(super) rehydration_token_cost: usize,
    pub(super) static_context_deltas: usize,
    pub(super) repo_state_facts: usize,
    pub(super) candidate_count: usize,
    pub(super) selected_candidate_count: usize,
    pub(super) rejected_candidate_count: usize,
    pub(super) selected_candidate_utility_points: u64,
    pub(super) segment_rollback_count: usize,
    pub(super) full_request_fallback_count: usize,
    pub(super) artifact_hash_failures: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextTransformOutcome {
    pub(super) stats: RuntimeSmartContextTransformStats,
    pub(super) deferred_rehydrate_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRewriteSafetyObservation {
    pub(super) safe: bool,
    pub(super) saved_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRewriteSafetyRecord {
    pub(super) observation: RuntimeSmartContextRewriteSafetyObservation,
    pub(super) observed_at_unix_secs: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextSelectiveRehydrateTerms {
    pub(super) file_paths: BTreeSet<String>,
    pub(super) error_codes: BTreeSet<String>,
    pub(super) test_symbols: BTreeSet<String>,
    pub(super) command_kinds: BTreeSet<String>,
    pub(super) diff_hunks: Vec<RuntimeSmartContextSelectiveDiffHunkTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextSelectiveDiffHunkTerm {
    pub(super) path: Option<String>,
    pub(super) old_start: Option<usize>,
    pub(super) new_start: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextIntentSignals {
    pub(super) intent_terms: Vec<String>,
    pub(super) semantic_terms: RuntimeSmartContextSelectiveRehydrateTerms,
    pub(super) artifact_refs: Vec<RuntimeSmartContextArtifactReference>,
    pub(super) command_kind_hints: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextToolCallMetadata {
    pub(super) command: Option<String>,
    pub(super) exit_code: Option<i32>,
    pub(super) kind_hint: Option<prodex_context::CommandOutputKind>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextToolOutputCompactionMetadata {
    pub(super) kind_hint: Option<prodex_context::CommandOutputKind>,
    pub(super) command: Option<String>,
    pub(super) exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextToolArgumentCandidate {
    pub(super) artifact: runtime_proxy_crate::SmartContextArtifactRef,
    pub(super) text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextToolArgumentDelta {
    pub(super) base_artifact: runtime_proxy_crate::SmartContextArtifactRef,
    pub(super) prefix_bytes: usize,
    pub(super) suffix_bytes: usize,
    pub(super) removed_bytes: usize,
    pub(super) inserted_bytes: usize,
    pub(super) inserted_hash: String,
    pub(super) inserted_preview: String,
    pub(super) common_bytes: usize,
    pub(super) changed_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextDuplicateChunkSummaryPlan {
    pub(super) text: String,
    pub(super) content_hash: String,
    pub(super) byte_len: usize,
    pub(super) occurrence_count: usize,
    pub(super) refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextExactAppendixRange {
    pub(super) reference: String,
    pub(super) body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextScoredExactAppendixRange {
    pub(super) range: RuntimeSmartContextExactAppendixRange,
    pub(super) score: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextArtifactAlias {
    pub(super) id: String,
    pub(super) alias: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RuntimeSmartContextArtifactIndexes<'a> {
    pub(super) line_index: Option<&'a RuntimeSmartContextArtifactLineIndex>,
    pub(super) chunk_index: Option<&'a RuntimeSmartContextArtifactChunkIndex>,
}

#[derive(Debug, Default)]
pub(super) struct RuntimeSmartContextProxyState {
    pub(super) enabled: bool,
    pub(super) model_context_window_tokens: Option<u64>,
    pub(super) artifacts: RuntimeSmartContextArtifactStore,
    pub(super) artifact_path: Option<PathBuf>,
    pub(super) last_token_usage: Option<runtime_proxy_crate::RuntimeTokenUsage>,
    pub(super) token_usage_history: Vec<runtime_proxy_crate::RuntimeTokenUsage>,
    pub(super) token_calibration_history: Vec<RuntimeSmartContextTokenCalibrationObservation>,
    pub(super) rewrite_telemetry_history: Vec<RuntimeSmartContextRewriteTelemetryRecord>,
    pub(super) rewrite_safety_history: Vec<RuntimeSmartContextRewriteSafetyRecord>,
    pub(super) last_static_context_fingerprints: Vec<runtime_proxy_crate::SmartContextFingerprint>,
    pub(super) last_static_context_prompt_cache_hash: Option<String>,
    pub(super) last_artifact_manifest_ids: BTreeSet<String>,
    pub(super) last_artifact_manifest_emitted_at: Option<Instant>,
    pub(super) artifact_aliases: BTreeMap<String, String>,
    pub(super) next_artifact_alias_index: usize,
    pub(super) static_section_fingerprints:
        BTreeMap<String, RuntimeSmartContextStaticSectionFingerprint>,
    pub(super) repo_state_facts: RuntimeSmartContextRepoStateFacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextBudget {
    pub(super) tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    pub(super) policy: runtime_proxy_crate::SmartContextAdaptiveBudgetPolicy,
    pub(super) model_context_window_tokens: u64,
    pub(super) model_context_window_source: &'static str,
    pub(super) available_tokens: usize,
    pub(super) observed_context_tokens: Option<usize>,
    pub(super) token_usage_source: &'static str,
    pub(super) pressure: runtime_proxy_crate::SmartContextPressureSnapshot,
}

pub(super) type RuntimeSmartContextBudgetInputs = (
    Vec<runtime_proxy_crate::RuntimeTokenUsage>,
    Vec<runtime_proxy_crate::RuntimeTokenUsage>,
    Vec<runtime_proxy_crate::SmartContextTokenCalibrationSample>,
    Option<u64>,
    runtime_proxy_crate::SmartContextRecentRewriteSafety,
    Vec<runtime_proxy_crate::SmartContextRewriteTelemetrySample>,
);

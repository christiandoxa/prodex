use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::RuntimeTokenUsage;

const SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX: &str = "psc:";

pub fn smart_context_structural_minify_json_body(body: &[u8]) -> Cow<'_, [u8]> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return Cow::Borrowed(body);
    };
    smart_context_structural_minify_json_value_body(body, &value)
}

pub fn smart_context_structural_minify_json_value_body<'a>(
    original_body: &'a [u8],
    value: &serde_json::Value,
) -> Cow<'a, [u8]> {
    match serde_json::to_vec(value) {
        Ok(body) if body != original_body => Cow::Owned(body),
        _ => Cow::Borrowed(original_body),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextExactnessDecision {
    Allow,
    RequireExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextExactnessReason {
    ExplicitExactMode,
    PreviousResponseAffinity,
    TurnStateAffinity,
    SessionAffinity,
    ToolOutputWithoutArtifact,
    RehydrateRequired,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartContextExactnessInput {
    pub exact_mode: bool,
    pub previous_response_id: Option<String>,
    pub turn_state: Option<String>,
    pub session_id: Option<String>,
    pub tool_output_without_artifact: bool,
    pub missing_rehydrate_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextExactnessGuard {
    pub decision: SmartContextExactnessDecision,
    pub reasons: Vec<SmartContextExactnessReason>,
}

pub fn smart_context_exactness_guard(
    input: SmartContextExactnessInput,
) -> SmartContextExactnessGuard {
    let mut reasons = Vec::new();
    if input.exact_mode {
        reasons.push(SmartContextExactnessReason::ExplicitExactMode);
    }
    if input.previous_response_id.as_deref().is_some_and(non_empty) {
        reasons.push(SmartContextExactnessReason::PreviousResponseAffinity);
    }
    if input.turn_state.as_deref().is_some_and(non_empty) {
        reasons.push(SmartContextExactnessReason::TurnStateAffinity);
    }
    if input.session_id.as_deref().is_some_and(non_empty) {
        reasons.push(SmartContextExactnessReason::SessionAffinity);
    }
    if input.tool_output_without_artifact {
        reasons.push(SmartContextExactnessReason::ToolOutputWithoutArtifact);
    }
    if input
        .missing_rehydrate_refs
        .iter()
        .any(|value| non_empty(value))
    {
        reasons.push(SmartContextExactnessReason::RehydrateRequired);
    }

    SmartContextExactnessGuard {
        decision: if reasons.is_empty() {
            SmartContextExactnessDecision::Allow
        } else {
            SmartContextExactnessDecision::RequireExact
        },
        reasons,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextArtifactRef {
    pub id: String,
    pub byte_len: usize,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextToolOutput {
    pub call_id: String,
    pub text: String,
    pub artifact: Option<SmartContextArtifactRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextCondensedToolOutput {
    Inline {
        call_id: String,
        text: String,
        content_hash: String,
    },
    ArtifactBacked {
        call_id: String,
        artifact: SmartContextArtifactRef,
        content_hash: String,
        summary: String,
    },
}

pub const SMART_CONTEXT_COMMAND_OUTPUT_CACHE_MIN_BYTES: usize = 4 * 1024;
pub const SMART_CONTEXT_COMMAND_OUTPUT_CRITICAL_SAMPLE_LIMIT: usize = 3;
const SMART_CONTEXT_COMMAND_OUTPUT_CRITICAL_SAMPLE_BYTES: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCommandOutputCacheRecord {
    pub id: String,
    pub content_hash: String,
    pub byte_len: usize,
    pub estimated_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCommandOutputCacheInput {
    pub id: String,
    pub text: String,
    pub previous_records: Vec<SmartContextCommandOutputCacheRecord>,
    pub min_replacement_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextCommandOutputCacheKeepReason {
    BelowMinByteThreshold,
    NoMatchingPreviousOutput,
    ChangedSincePreviousOutput,
    SummaryWouldNotSaveTokens,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextCommandOutputCacheAction {
    KeepExact {
        reason: SmartContextCommandOutputCacheKeepReason,
        summary: Option<String>,
    },
    ReplaceWithUnchangedSummary {
        ref_id: String,
        saved_tokens: u64,
        critical_signal_count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCommandOutputCacheRewrite {
    pub record: SmartContextCommandOutputCacheRecord,
    pub output: String,
    pub action: SmartContextCommandOutputCacheAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCommandOutputCriticalSignals {
    pub count: usize,
    pub samples: Vec<String>,
}

pub fn smart_context_command_output_cache_record(
    id: impl Into<String>,
    text: &str,
) -> SmartContextCommandOutputCacheRecord {
    SmartContextCommandOutputCacheRecord {
        id: id.into(),
        content_hash: smart_context_normalized_command_output_hash_text(text),
        byte_len: text.len(),
        estimated_tokens: smart_context_estimate_tokens_from_body(text.as_bytes()),
    }
}

pub fn smart_context_command_output_cache_rewrite(
    input: SmartContextCommandOutputCacheInput,
) -> SmartContextCommandOutputCacheRewrite {
    let record = smart_context_command_output_cache_record(input.id, &input.text);
    let min_replacement_bytes = if input.min_replacement_bytes == 0 {
        SMART_CONTEXT_COMMAND_OUTPUT_CACHE_MIN_BYTES
    } else {
        input.min_replacement_bytes
    };

    if record.byte_len < min_replacement_bytes {
        return smart_context_command_output_keep_exact(
            record,
            input.text,
            SmartContextCommandOutputCacheKeepReason::BelowMinByteThreshold,
            None,
        );
    }

    let mut previous_records = input
        .previous_records
        .into_iter()
        .filter(smart_context_command_output_cache_record_valid)
        .collect::<Vec<_>>();
    previous_records.sort_by(|left, right| {
        (left.id != record.id)
            .cmp(&(right.id != record.id))
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.content_hash.cmp(&right.content_hash))
            .then_with(|| left.byte_len.cmp(&right.byte_len))
    });

    if let Some(previous) = previous_records
        .iter()
        .find(|previous| previous.content_hash == record.content_hash)
    {
        let summary =
            smart_context_command_output_unchanged_summary(&record, previous, &input.text);
        let summary_estimated_tokens = smart_context_estimate_tokens_from_body(summary.as_bytes());
        if summary.len() >= record.byte_len || summary_estimated_tokens >= record.estimated_tokens {
            return smart_context_command_output_keep_exact(
                record,
                input.text,
                SmartContextCommandOutputCacheKeepReason::SummaryWouldNotSaveTokens,
                None,
            );
        }

        return SmartContextCommandOutputCacheRewrite {
            output: summary,
            action: SmartContextCommandOutputCacheAction::ReplaceWithUnchangedSummary {
                ref_id: previous.id.clone(),
                saved_tokens: record
                    .estimated_tokens
                    .saturating_sub(summary_estimated_tokens),
                critical_signal_count: smart_context_command_output_critical_signals(&input.text)
                    .count,
            },
            record,
        };
    }

    let changed_summary = previous_records
        .iter()
        .find(|previous| previous.id == record.id)
        .map(|previous| smart_context_command_output_changed_summary(&record, previous));
    let reason = if changed_summary.is_some() {
        SmartContextCommandOutputCacheKeepReason::ChangedSincePreviousOutput
    } else {
        SmartContextCommandOutputCacheKeepReason::NoMatchingPreviousOutput
    };

    smart_context_command_output_keep_exact(record, input.text, reason, changed_summary)
}

pub fn smart_context_command_output_critical_signals(
    text: &str,
) -> SmartContextCommandOutputCriticalSignals {
    let mut count = 0usize;
    let mut samples = Vec::new();

    for line in text.lines() {
        if !smart_context_command_output_line_has_critical_signal(line) {
            continue;
        }
        count += 1;
        if samples.len() < SMART_CONTEXT_COMMAND_OUTPUT_CRITICAL_SAMPLE_LIMIT {
            samples.push(smart_context_command_output_signal_sample(line));
        }
    }

    SmartContextCommandOutputCriticalSignals { count, samples }
}

pub fn smart_context_artifact_marker(
    artifact: &SmartContextArtifactRef,
    compacted: &str,
) -> String {
    let marker = smart_context_artifact_marker_line("artifact", artifact);
    if compacted.is_empty() {
        return marker;
    }
    format!("{marker}\n{compacted}")
}

pub fn smart_context_artifact_reference_marker(artifact: &SmartContextArtifactRef) -> String {
    format!(
        "psc repeat {} b={}",
        smart_context_short_artifact_ref(&artifact.id),
        artifact.byte_len,
    )
}

pub fn smart_context_short_artifact_ref(id: &str) -> String {
    format!(
        "{SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX}{}",
        smart_context_short_artifact_label(id)
    )
}

pub fn smart_context_short_artifact_line_ref(id: &str, start: usize, end: usize) -> String {
    format!("{}#L{start}-L{end}", smart_context_short_artifact_ref(id))
}

pub fn smart_context_condense_tool_outputs(
    outputs: impl IntoIterator<Item = SmartContextToolOutput>,
    inline_byte_limit: usize,
) -> Vec<SmartContextCondensedToolOutput> {
    outputs
        .into_iter()
        .map(|output| {
            let content_hash = smart_context_hash_text(&output.text);
            if output.text.len() <= inline_byte_limit {
                return SmartContextCondensedToolOutput::Inline {
                    call_id: output.call_id,
                    text: output.text,
                    content_hash,
                };
            }

            match output.artifact {
                Some(artifact) if artifact.content_hash == content_hash => {
                    SmartContextCondensedToolOutput::ArtifactBacked {
                        call_id: output.call_id,
                        summary: smart_context_summary_prefix(&output.text, inline_byte_limit),
                        artifact,
                        content_hash,
                    }
                }
                _ => SmartContextCondensedToolOutput::Inline {
                    call_id: output.call_id,
                    text: output.text,
                    content_hash,
                },
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextConversationItem {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextHashRef {
    pub id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextDedupeItem {
    Keep {
        id: String,
        content_hash: String,
    },
    Duplicate {
        id: String,
        ref_id: String,
        content_hash: String,
    },
}

pub fn smart_context_conversation_dedupe(
    items: impl IntoIterator<Item = SmartContextConversationItem>,
) -> Vec<SmartContextDedupeItem> {
    let mut seen = BTreeMap::<String, String>::new();
    items
        .into_iter()
        .map(|item| {
            let content_hash = smart_context_normalized_command_output_hash_text(&item.text);
            if let Some(ref_id) = seen.get(&content_hash) {
                SmartContextDedupeItem::Duplicate {
                    id: item.id,
                    ref_id: ref_id.clone(),
                    content_hash,
                }
            } else {
                seen.insert(content_hash.clone(), item.id.clone());
                SmartContextDedupeItem::Keep {
                    id: item.id,
                    content_hash,
                }
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextCrossTurnDuplicateKeepReason {
    ExactnessRequired,
    BelowMinByteThreshold,
    MissingArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextCrossTurnDuplicateRefAction {
    Keep {
        id: String,
        content_hash: String,
        byte_len: usize,
        reason: SmartContextCrossTurnDuplicateKeepReason,
    },
    ReplaceWithArtifactRef {
        id: String,
        artifact: SmartContextArtifactRef,
        content_hash: String,
        byte_len: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCrossTurnDuplicateRefPlan {
    pub actions: Vec<SmartContextCrossTurnDuplicateRefAction>,
    pub replaced_items: usize,
    pub replaced_bytes: usize,
}

pub fn smart_context_cross_turn_duplicate_ref_plan(
    items: impl IntoIterator<Item = SmartContextConversationItem>,
    available_artifacts: impl IntoIterator<Item = SmartContextArtifactRef>,
    min_replacement_bytes: usize,
    exactness_guard: &SmartContextExactnessGuard,
) -> SmartContextCrossTurnDuplicateRefPlan {
    let artifacts = smart_context_available_artifacts_by_hash_and_len(available_artifacts);
    let exactness_allows = exactness_guard.decision == SmartContextExactnessDecision::Allow;
    let mut actions = Vec::new();
    let mut replaced_items = 0usize;
    let mut replaced_bytes = 0usize;

    for item in items {
        let byte_len = item.text.len();
        let content_hash = smart_context_hash_text(&item.text);
        let action = if !exactness_allows {
            SmartContextCrossTurnDuplicateRefAction::Keep {
                id: item.id,
                content_hash,
                byte_len,
                reason: SmartContextCrossTurnDuplicateKeepReason::ExactnessRequired,
            }
        } else if byte_len < min_replacement_bytes {
            SmartContextCrossTurnDuplicateRefAction::Keep {
                id: item.id,
                content_hash,
                byte_len,
                reason: SmartContextCrossTurnDuplicateKeepReason::BelowMinByteThreshold,
            }
        } else if let Some(artifact) = artifacts.get(&(content_hash.clone(), byte_len)) {
            replaced_items += 1;
            replaced_bytes = replaced_bytes.saturating_add(byte_len);
            SmartContextCrossTurnDuplicateRefAction::ReplaceWithArtifactRef {
                id: item.id,
                artifact: artifact.clone(),
                content_hash,
                byte_len,
            }
        } else {
            SmartContextCrossTurnDuplicateRefAction::Keep {
                id: item.id,
                content_hash,
                byte_len,
                reason: SmartContextCrossTurnDuplicateKeepReason::MissingArtifact,
            }
        };
        actions.push(action);
    }

    SmartContextCrossTurnDuplicateRefPlan {
        actions,
        replaced_items,
        replaced_bytes,
    }
}

pub fn smart_context_hash_refs(
    items: impl IntoIterator<Item = SmartContextConversationItem>,
) -> Vec<SmartContextHashRef> {
    items
        .into_iter()
        .map(|item| SmartContextHashRef {
            id: item.id,
            content_hash: smart_context_hash_text(&item.text),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextTokenBudgetTier {
    Exact,
    Large,
    Condensed,
    Minimal,
}

pub fn smart_context_token_budget_tier(available_tokens: usize) -> SmartContextTokenBudgetTier {
    match available_tokens {
        16_000.. => SmartContextTokenBudgetTier::Exact,
        8_000..=15_999 => SmartContextTokenBudgetTier::Large,
        2_000..=7_999 => SmartContextTokenBudgetTier::Condensed,
        _ => SmartContextTokenBudgetTier::Minimal,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmartContextMemoryCapsule {
    pub id: String,
    pub token_cost: usize,
    pub relevance: f32,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextMemoryCapsuleSelection {
    pub selected_ids: Vec<String>,
    pub omitted_ids: Vec<String>,
    pub used_tokens: usize,
}

pub const SMART_CONTEXT_MEMORY_CAPSULE_MINIMAL_TOKEN_BUDGET: usize = 256;
pub const SMART_CONTEXT_MEMORY_CAPSULE_CONDENSED_TOKEN_BUDGET: usize = 1_024;
pub const SMART_CONTEXT_MEMORY_CAPSULE_LARGE_TOKEN_BUDGET: usize = 4_096;

pub fn smart_context_select_memory_capsules_for_policy(
    capsules: impl IntoIterator<Item = SmartContextMemoryCapsule>,
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> SmartContextMemoryCapsuleSelection {
    smart_context_select_memory_capsules(
        capsules,
        smart_context_memory_capsule_token_budget(accounting, policy),
    )
}

pub fn smart_context_memory_capsule_token_budget(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> usize {
    if smart_context_memory_capsule_policy_allows_unbounded_budget(accounting, policy) {
        return usize::MAX;
    }
    if !smart_context_memory_capsule_policy_allows_bounded_budget(accounting, policy) {
        return 0;
    }

    let Some(available_context_tokens) = accounting.available_context_tokens else {
        return 0;
    };

    let mode_budget = match policy.mode {
        SmartContextBudgetMode::MinimalRefsOnly => {
            SMART_CONTEXT_MEMORY_CAPSULE_MINIMAL_TOKEN_BUDGET
        }
        SmartContextBudgetMode::ArtifactCondensed => {
            SMART_CONTEXT_MEMORY_CAPSULE_CONDENSED_TOKEN_BUDGET
        }
        SmartContextBudgetMode::LargeLossless => SMART_CONTEXT_MEMORY_CAPSULE_LARGE_TOKEN_BUDGET,
        SmartContextBudgetMode::ExactPassThrough => match policy.tier {
            SmartContextTokenBudgetTier::Exact | SmartContextTokenBudgetTier::Large => {
                SMART_CONTEXT_MEMORY_CAPSULE_LARGE_TOKEN_BUDGET
            }
            SmartContextTokenBudgetTier::Condensed => {
                SMART_CONTEXT_MEMORY_CAPSULE_CONDENSED_TOKEN_BUDGET
            }
            SmartContextTokenBudgetTier::Minimal => {
                SMART_CONTEXT_MEMORY_CAPSULE_MINIMAL_TOKEN_BUDGET
            }
        },
    };

    mode_budget
        .min(smart_context_u64_saturating_usize(
            policy.max_rehydrate_tokens,
        ))
        .min(smart_context_u64_saturating_usize(available_context_tokens))
}

pub fn smart_context_select_memory_capsules(
    capsules: impl IntoIterator<Item = SmartContextMemoryCapsule>,
    token_budget: usize,
) -> SmartContextMemoryCapsuleSelection {
    let mut required = Vec::new();
    let mut optional = Vec::new();
    for capsule in capsules {
        if capsule.required {
            required.push(capsule);
        } else {
            optional.push(capsule);
        }
    }

    required.sort_by(|left, right| left.id.cmp(&right.id));
    optional.sort_by(smart_context_capsule_order);

    let mut selected_ids = Vec::new();
    let mut omitted_ids = Vec::new();
    let mut used_tokens = 0usize;

    for capsule in required.into_iter().chain(optional) {
        if used_tokens.saturating_add(capsule.token_cost) <= token_budget {
            used_tokens += capsule.token_cost;
            selected_ids.push(capsule.id);
        } else {
            omitted_ids.push(capsule.id);
        }
    }

    SmartContextMemoryCapsuleSelection {
        selected_ids,
        omitted_ids,
        used_tokens,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRehydrateRef {
    pub id: String,
    pub token_cost: usize,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextRehydrateAction {
    Rehydrate {
        id: String,
        token_cost: usize,
    },
    Defer {
        id: String,
        reason: SmartContextRehydrateDeferReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextRehydrateDeferReason {
    MissingArtifact,
    TokenBudgetExceeded,
    MinimalBudgetTier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRehydratePlan {
    pub actions: Vec<SmartContextRehydrateAction>,
    pub used_tokens: usize,
}

pub fn smart_context_auto_rehydrate_plan(
    refs: impl IntoIterator<Item = SmartContextRehydrateRef>,
    available_artifact_ids: impl IntoIterator<Item = String>,
    token_budget: usize,
    tier: SmartContextTokenBudgetTier,
) -> SmartContextRehydratePlan {
    let available = available_artifact_ids.into_iter().collect::<BTreeSet<_>>();
    let mut refs = refs.into_iter().collect::<Vec<_>>();
    refs.sort_by(|left, right| {
        right
            .required
            .cmp(&left.required)
            .then_with(|| left.token_cost.cmp(&right.token_cost))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut actions = Vec::new();
    let mut used_tokens = 0usize;
    for item in refs {
        if !available.contains(&item.id) {
            actions.push(SmartContextRehydrateAction::Defer {
                id: item.id,
                reason: SmartContextRehydrateDeferReason::MissingArtifact,
            });
        } else if tier == SmartContextTokenBudgetTier::Minimal && !item.required {
            actions.push(SmartContextRehydrateAction::Defer {
                id: item.id,
                reason: SmartContextRehydrateDeferReason::MinimalBudgetTier,
            });
        } else if used_tokens.saturating_add(item.token_cost) <= token_budget {
            used_tokens += item.token_cost;
            actions.push(SmartContextRehydrateAction::Rehydrate {
                id: item.id,
                token_cost: item.token_cost,
            });
        } else {
            actions.push(SmartContextRehydrateAction::Defer {
                id: item.id,
                reason: SmartContextRehydrateDeferReason::TokenBudgetExceeded,
            });
        }
    }

    SmartContextRehydratePlan {
        actions,
        used_tokens,
    }
}

pub const SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN: u64 = 4;
const SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_NUMERATOR: u64 = 9;
const SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_DENOMINATOR: u64 = 8;
const SMART_CONTEXT_ADAPTIVE_ESTIMATE_MIN_MARGIN_TOKENS: u64 = 64;
const SMART_CONTEXT_ADAPTIVE_ESTIMATE_RECENT_USAGE_LIMIT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextTokenAccountingSource {
    CurrentRequestTokens,
    CurrentRequestBodyEstimate,
    ObservedHistory,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextTokenAccountingRisk {
    UnknownTokenWindow,
    ZeroContextWindow,
    ReservedOutputConsumesWindow,
    UnknownCurrentRequestAccounting,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartContextObservedTokenAccountingInput {
    pub model_context_window_tokens: Option<u64>,
    pub reserved_output_tokens: u64,
    pub current_input_tokens: u64,
    pub current_request_body_bytes: usize,
    pub current_request_estimated_tokens: Option<u64>,
    pub observed_usage: Vec<RuntimeTokenUsage>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SmartContextTokenCalibrationBucketKey {
    pub route: Option<String>,
    pub model: Option<String>,
    pub profile: Option<String>,
    pub transport: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTokenCalibrationSample {
    pub bucket_key: Option<SmartContextTokenCalibrationBucketKey>,
    pub usage: RuntimeTokenUsage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartContextObservedTokenAccountingCalibrationInput {
    pub accounting: SmartContextObservedTokenAccountingInput,
    pub calibration_bucket_key: Option<SmartContextTokenCalibrationBucketKey>,
    pub calibration_samples: Vec<SmartContextTokenCalibrationSample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextObservedTokenAccounting {
    pub model_context_window_tokens: Option<u64>,
    pub observed_turns: usize,
    pub observed_input_tokens: u64,
    pub observed_cached_input_tokens: u64,
    pub observed_uncached_input_tokens: u64,
    pub observed_output_tokens: u64,
    pub observed_reasoning_tokens: u64,
    pub observed_total_tokens: u64,
    pub observed_context_tokens: u64,
    pub last_input_tokens: u64,
    pub last_accounted_input_tokens: u64,
    pub last_observed_context_tokens: u64,
    pub current_request_body_bytes: usize,
    pub estimated_current_request_tokens: u64,
    pub current_request_accounted_tokens: u64,
    pub effective_input_tokens: u64,
    pub effective_input_source: SmartContextTokenAccountingSource,
    pub reserved_output_tokens: u64,
    pub available_context_tokens: Option<u64>,
    pub accounting_risks: Vec<SmartContextTokenAccountingRisk>,
}

pub fn smart_context_observed_token_accounting(
    input: SmartContextObservedTokenAccountingInput,
) -> SmartContextObservedTokenAccounting {
    smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: input,
            calibration_bucket_key: None,
            calibration_samples: Vec::new(),
        },
    )
}

pub fn smart_context_observed_token_accounting_with_calibration(
    input: SmartContextObservedTokenAccountingCalibrationInput,
) -> SmartContextObservedTokenAccounting {
    let SmartContextObservedTokenAccountingCalibrationInput {
        accounting: input,
        calibration_bucket_key,
        calibration_samples,
    } = input;
    let mut observed_input_tokens = 0u64;
    let mut observed_cached_input_tokens = 0u64;
    let mut observed_output_tokens = 0u64;
    let mut observed_reasoning_tokens = 0u64;
    let mut last_input_tokens = 0u64;
    let mut last_accounted_input_tokens = 0u64;
    let mut last_observed_context_tokens = 0u64;

    for usage in &input.observed_usage {
        observed_input_tokens = observed_input_tokens.saturating_add(usage.input_tokens);
        observed_cached_input_tokens =
            observed_cached_input_tokens.saturating_add(usage.cached_input_tokens);
        observed_output_tokens = observed_output_tokens.saturating_add(usage.output_tokens);
        observed_reasoning_tokens =
            observed_reasoning_tokens.saturating_add(usage.reasoning_tokens);
        last_input_tokens = usage.input_tokens;
        last_accounted_input_tokens = smart_context_accounted_input_tokens(*usage).unwrap_or(0);
        last_observed_context_tokens =
            smart_context_observed_usage_context_tokens(*usage).unwrap_or(0);
    }

    let observed_uncached_input_tokens =
        observed_input_tokens.saturating_sub(observed_cached_input_tokens);
    let observed_total_tokens = observed_input_tokens.saturating_add(observed_output_tokens);
    let observed_context_tokens = observed_total_tokens.saturating_add(observed_reasoning_tokens);
    let baseline_estimated_current_request_tokens =
        input.current_request_estimated_tokens.unwrap_or_else(|| {
            smart_context_estimate_tokens_from_body_bytes(input.current_request_body_bytes)
        });
    let estimated_current_request_tokens = smart_context_observed_calibrated_request_estimate(
        input.current_request_body_bytes,
        baseline_estimated_current_request_tokens,
        &input.observed_usage,
        calibration_bucket_key.as_ref(),
        &calibration_samples,
    );
    let current_request_accounted_tokens = input
        .current_input_tokens
        .max(estimated_current_request_tokens);
    let effective_input_tokens = current_request_accounted_tokens.max(last_accounted_input_tokens);
    let effective_input_source = smart_context_effective_input_source(
        input.current_input_tokens,
        estimated_current_request_tokens,
        current_request_accounted_tokens,
        last_accounted_input_tokens,
        effective_input_tokens,
    );
    let available_context_tokens = input.model_context_window_tokens.map(|window| {
        window
            .saturating_sub(effective_input_tokens)
            .saturating_sub(input.reserved_output_tokens)
    });
    let accounting_risks = smart_context_token_accounting_risks(
        input.model_context_window_tokens,
        input.reserved_output_tokens,
        effective_input_source,
    );

    SmartContextObservedTokenAccounting {
        model_context_window_tokens: input.model_context_window_tokens,
        observed_turns: input.observed_usage.len(),
        observed_input_tokens,
        observed_cached_input_tokens,
        observed_uncached_input_tokens,
        observed_output_tokens,
        observed_reasoning_tokens,
        observed_total_tokens,
        observed_context_tokens,
        last_input_tokens,
        last_accounted_input_tokens,
        last_observed_context_tokens,
        current_request_body_bytes: input.current_request_body_bytes,
        estimated_current_request_tokens,
        current_request_accounted_tokens,
        effective_input_tokens,
        effective_input_source,
        reserved_output_tokens: input.reserved_output_tokens,
        available_context_tokens,
        accounting_risks,
    }
}

pub fn smart_context_estimate_tokens_from_body_bytes(body_bytes: usize) -> u64 {
    let body_bytes = u64::try_from(body_bytes).unwrap_or(u64::MAX);
    body_bytes.saturating_add(SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN - 1)
        / SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN
}

fn smart_context_observed_calibrated_request_estimate(
    body_bytes: usize,
    baseline_estimate: u64,
    observed_usage: &[RuntimeTokenUsage],
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> u64 {
    if baseline_estimate == 0 {
        return 0;
    }
    let Some(recent_accounted_input) = smart_context_recent_accounted_input_calibration(
        observed_usage,
        calibration_bucket_key,
        calibration_samples,
    ) else {
        return baseline_estimate;
    };
    let raw_floor = smart_context_estimate_tokens_from_body_bytes(body_bytes)
        .saturating_add(1)
        .saturating_div(2)
        .max(1);
    let raw_floor_with_margin =
        raw_floor.saturating_add(SMART_CONTEXT_ADAPTIVE_ESTIMATE_MIN_MARGIN_TOKENS);
    let observed_with_margin = recent_accounted_input
        .saturating_mul(SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_NUMERATOR)
        .saturating_add(SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_DENOMINATOR - 1)
        / SMART_CONTEXT_ADAPTIVE_ESTIMATE_SAFETY_DENOMINATOR;
    let observed_with_margin =
        observed_with_margin.saturating_add(SMART_CONTEXT_ADAPTIVE_ESTIMATE_MIN_MARGIN_TOKENS);
    baseline_estimate.min(observed_with_margin.max(raw_floor_with_margin))
}

fn smart_context_recent_accounted_input_calibration(
    observed_usage: &[RuntimeTokenUsage],
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> Option<u64> {
    if !calibration_samples.is_empty() {
        return smart_context_recent_accounted_input_calibration_for_bucket(
            calibration_bucket_key,
            calibration_samples,
        );
    }

    observed_usage
        .iter()
        .rev()
        .filter_map(|usage| smart_context_accounted_input_tokens(*usage))
        .take(SMART_CONTEXT_ADAPTIVE_ESTIMATE_RECENT_USAGE_LIMIT)
        .max()
}

fn smart_context_recent_accounted_input_calibration_for_bucket(
    calibration_bucket_key: Option<&SmartContextTokenCalibrationBucketKey>,
    calibration_samples: &[SmartContextTokenCalibrationSample],
) -> Option<u64> {
    if let Some(calibration) = smart_context_recent_accounted_input_calibration_matching(
        calibration_samples,
        |sample_bucket_key| sample_bucket_key == calibration_bucket_key,
    ) {
        return Some(calibration);
    }

    let calibration_bucket_key = calibration_bucket_key?;

    for tier in [
        SmartContextTokenCalibrationFallbackTier::Model,
        SmartContextTokenCalibrationFallbackTier::ProfileRoute,
        SmartContextTokenCalibrationFallbackTier::RouteTransportGlobal,
    ] {
        if let Some(calibration) = smart_context_recent_accounted_input_calibration_matching(
            calibration_samples,
            |sample_bucket_key| {
                smart_context_token_calibration_bucket_fallback_matches(
                    calibration_bucket_key,
                    sample_bucket_key,
                    tier,
                )
            },
        ) {
            return Some(calibration);
        }
    }

    None
}

fn smart_context_recent_accounted_input_calibration_matching(
    calibration_samples: &[SmartContextTokenCalibrationSample],
    mut bucket_matches: impl FnMut(Option<&SmartContextTokenCalibrationBucketKey>) -> bool,
) -> Option<u64> {
    calibration_samples
        .iter()
        .rev()
        .filter(|sample| bucket_matches(sample.bucket_key.as_ref()))
        .filter_map(|sample| smart_context_accounted_input_tokens(sample.usage))
        .take(SMART_CONTEXT_ADAPTIVE_ESTIMATE_RECENT_USAGE_LIMIT)
        .max()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmartContextTokenCalibrationFallbackTier {
    Model,
    ProfileRoute,
    RouteTransportGlobal,
}

fn smart_context_token_calibration_bucket_fallback_matches(
    target: &SmartContextTokenCalibrationBucketKey,
    sample: Option<&SmartContextTokenCalibrationBucketKey>,
    tier: SmartContextTokenCalibrationFallbackTier,
) -> bool {
    match tier {
        SmartContextTokenCalibrationFallbackTier::Model => sample.is_some_and(|sample| {
            smart_context_token_calibration_model_matches(&target.model, &sample.model)
        }),
        SmartContextTokenCalibrationFallbackTier::ProfileRoute => sample.is_some_and(|sample| {
            smart_context_token_calibration_field_matches(&target.profile, &sample.profile)
                && smart_context_token_calibration_field_matches(&target.route, &sample.route)
        }),
        SmartContextTokenCalibrationFallbackTier::RouteTransportGlobal => sample
            .map(|sample| {
                (smart_context_token_calibration_field_matches(&target.route, &sample.route)
                    && smart_context_token_calibration_field_matches(
                        &target.transport,
                        &sample.transport,
                    ))
                    || smart_context_token_calibration_sample_is_global_compatible(target, sample)
            })
            .unwrap_or(true),
    }
}

fn smart_context_token_calibration_field_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    target.as_deref().is_some_and(non_empty) && target == sample
}

fn smart_context_token_calibration_model_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    let Some(target) = smart_context_token_calibration_normalized_model(target.as_deref()) else {
        return false;
    };
    let Some(sample) = smart_context_token_calibration_normalized_model(sample.as_deref()) else {
        return false;
    };
    target == sample
        || smart_context_token_calibration_model_family(&target)
            == smart_context_token_calibration_model_family(&sample)
}

fn smart_context_token_calibration_sample_is_global_compatible(
    target: &SmartContextTokenCalibrationBucketKey,
    sample: &SmartContextTokenCalibrationBucketKey,
) -> bool {
    smart_context_token_calibration_optional_field_compatible(&target.route, &sample.route)
        && smart_context_token_calibration_optional_model_compatible(&target.model, &sample.model)
        && smart_context_token_calibration_optional_field_compatible(
            &target.profile,
            &sample.profile,
        )
        && smart_context_token_calibration_optional_field_compatible(
            &target.transport,
            &sample.transport,
        )
}

fn smart_context_token_calibration_optional_field_compatible(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    sample
        .as_deref()
        .is_none_or(|sample| target.as_deref() == Some(sample))
}

fn smart_context_token_calibration_optional_model_compatible(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    sample
        .as_ref()
        .is_none_or(|_| smart_context_token_calibration_model_matches(target, sample))
}

fn smart_context_token_calibration_normalized_model(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    let mut normalized = String::with_capacity(value.len());
    let mut previous_separator = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        let ch = match ch {
            '_' | ' ' => '-',
            ch => ch,
        };
        if ch == '-' {
            if !previous_separator {
                normalized.push(ch);
                previous_separator = true;
            }
        } else {
            normalized.push(ch);
            previous_separator = false;
        }
    }
    let normalized = normalized.trim_matches('-');
    (!normalized.is_empty()).then(|| normalized.to_string())
}

fn smart_context_token_calibration_model_family(model: &str) -> &str {
    let model = model
        .strip_suffix("-latest")
        .or_else(|| model.strip_suffix("-preview"))
        .unwrap_or(model);

    if let Some(family) = model
        .strip_suffix("-mini")
        .or_else(|| model.strip_suffix("-nano"))
        .or_else(|| model.strip_suffix("-codex"))
    {
        return family;
    }

    if model.len() >= 11 {
        let suffix_start = model.len() - 11;
        let suffix = &model[suffix_start..];
        if suffix.as_bytes()[0] == b'-'
            && suffix.as_bytes()[1..5].iter().all(u8::is_ascii_digit)
            && suffix.as_bytes()[5] == b'-'
            && suffix.as_bytes()[6..8].iter().all(u8::is_ascii_digit)
            && suffix.as_bytes()[8] == b'-'
            && suffix.as_bytes()[9..11].iter().all(u8::is_ascii_digit)
        {
            return &model[..suffix_start];
        }
    }

    model
}

fn smart_context_accounted_input_tokens(usage: RuntimeTokenUsage) -> Option<u64> {
    let accounted = if usage.input_tokens == 0 {
        usage.cached_input_tokens
    } else {
        usage.input_tokens
    };
    (accounted > 0).then_some(accounted)
}

pub fn smart_context_estimate_tokens_from_body(body: &[u8]) -> u64 {
    let Ok(text) = std::str::from_utf8(body) else {
        return smart_context_estimate_tokens_from_body_bytes(body.len());
    };
    smart_context_estimate_tokens_from_text(text)
        .max(smart_context_estimate_tokens_from_body_bytes(body.len()).saturating_div(2))
}

fn smart_context_estimate_tokens_from_text(text: &str) -> u64 {
    let mut tokens = 0u64;
    let mut run = String::new();
    let mut run_kind = SmartContextEstimatorRunKind::Other;
    let mut structural = 0u64;
    let mut separators = 0u64;

    for ch in text.chars() {
        let kind = smart_context_estimator_run_kind(ch);
        if matches!(
            kind,
            SmartContextEstimatorRunKind::Word | SmartContextEstimatorRunKind::Number
        ) {
            if kind != run_kind {
                tokens = tokens.saturating_add(smart_context_estimate_run_tokens(&run, run_kind));
                run.clear();
                run_kind = kind;
            }
            run.push(ch);
            continue;
        }

        tokens = tokens.saturating_add(smart_context_estimate_run_tokens(&run, run_kind));
        run.clear();
        run_kind = SmartContextEstimatorRunKind::Other;

        if ch.is_whitespace() {
            if ch == '\n' {
                separators = separators.saturating_add(1);
            }
        } else if matches!(
            ch,
            '{' | '}' | '[' | ']' | ':' | ',' | '"' | '\'' | '`' | '(' | ')' | '<' | '>'
        ) {
            structural = structural.saturating_add(1);
        } else {
            tokens = tokens.saturating_add(1);
        }
    }

    tokens = tokens.saturating_add(smart_context_estimate_run_tokens(&run, run_kind));
    tokens
        .saturating_add(structural.saturating_add(3) / 4)
        .saturating_add(separators.saturating_add(7) / 8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmartContextEstimatorRunKind {
    Word,
    Number,
    Other,
}

fn smart_context_estimator_run_kind(ch: char) -> SmartContextEstimatorRunKind {
    if ch.is_ascii_alphabetic() || ch == '_' || ch == '-' {
        SmartContextEstimatorRunKind::Word
    } else if ch.is_ascii_digit() {
        SmartContextEstimatorRunKind::Number
    } else {
        SmartContextEstimatorRunKind::Other
    }
}

fn smart_context_estimate_run_tokens(run: &str, kind: SmartContextEstimatorRunKind) -> u64 {
    if run.is_empty() {
        return 0;
    }
    let chars = u64::try_from(run.chars().count()).unwrap_or(u64::MAX);
    match kind {
        SmartContextEstimatorRunKind::Word => chars.saturating_add(3) / 4,
        SmartContextEstimatorRunKind::Number => chars.saturating_add(2) / 3,
        SmartContextEstimatorRunKind::Other => chars,
    }
    .max(1)
}

pub fn smart_context_observed_usage_context_tokens(usage: RuntimeTokenUsage) -> Option<u64> {
    let observed = usage
        .input_tokens
        .saturating_add(usage.output_tokens)
        .saturating_add(usage.reasoning_tokens);
    let observed = if observed == 0 {
        usage.cached_input_tokens
    } else {
        observed
    };
    (observed > 0).then_some(observed)
}

pub fn smart_context_token_budget_tier_from_accounting(
    accounting: &SmartContextObservedTokenAccounting,
) -> SmartContextTokenBudgetTier {
    accounting
        .available_context_tokens
        .map(smart_context_u64_budget_tier)
        .unwrap_or(SmartContextTokenBudgetTier::Exact)
}

pub fn smart_context_accounting_safe_for_adaptive_policy(
    accounting: &SmartContextObservedTokenAccounting,
) -> bool {
    accounting.accounting_risks.is_empty()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextArtifactLineRangeRef {
    pub artifact_id: String,
    pub artifact_content_hash: String,
    pub artifact_byte_len: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub excerpt_hash: String,
    pub excerpt_byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextArtifactLineRange {
    pub reference: SmartContextArtifactLineRangeRef,
    pub excerpt: String,
}

pub fn smart_context_artifact_line_range(
    artifact: &SmartContextArtifactRef,
    artifact_text: &str,
    start_line: usize,
    end_line: usize,
) -> Option<SmartContextArtifactLineRange> {
    if artifact.content_hash != smart_context_hash_text(artifact_text) {
        return None;
    }

    let excerpt = smart_context_extract_line_range(artifact_text, start_line, end_line)?;
    let reference = SmartContextArtifactLineRangeRef {
        artifact_id: artifact.id.clone(),
        artifact_content_hash: artifact.content_hash.clone(),
        artifact_byte_len: artifact.byte_len,
        start_line,
        end_line,
        excerpt_hash: smart_context_hash_text(&excerpt),
        excerpt_byte_len: excerpt.len(),
    };

    Some(SmartContextArtifactLineRange { reference, excerpt })
}

pub fn smart_context_extract_line_range(
    text: &str,
    start_line: usize,
    end_line: usize,
) -> Option<String> {
    if start_line == 0 || end_line < start_line {
        return None;
    }

    let mut selected = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if line_number > end_line {
            break;
        }
        if line_number >= start_line {
            selected.push(line);
        }
    }

    (!selected.is_empty()).then(|| selected.join("\n"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextFingerprintKind {
    StaticContext,
    ConversationTurn,
    ToolOutput,
    Artifact,
    MemoryCapsule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextFingerprintInput {
    pub id: String,
    pub kind: SmartContextFingerprintKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextFingerprint {
    pub id: String,
    pub kind: SmartContextFingerprintKind,
    pub content_hash: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStaticContextItem {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStableStaticContextItem {
    pub id: String,
    pub canonical_text: String,
    pub content_hash: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStaticContextPromptCacheFingerprint {
    pub content_hash: String,
    pub items: Vec<SmartContextStableStaticContextItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextFingerprintChange {
    Added {
        fingerprint: SmartContextFingerprint,
    },
    Removed {
        fingerprint: SmartContextFingerprint,
    },
    Unchanged {
        fingerprint: SmartContextFingerprint,
    },
    Changed {
        before: SmartContextFingerprint,
        after: SmartContextFingerprint,
    },
}

pub fn smart_context_fingerprint(input: SmartContextFingerprintInput) -> SmartContextFingerprint {
    SmartContextFingerprint {
        id: input.id,
        kind: input.kind,
        content_hash: smart_context_hash_text(&input.text),
        byte_len: input.text.len(),
    }
}

pub fn smart_context_fingerprints(
    inputs: impl IntoIterator<Item = SmartContextFingerprintInput>,
) -> Vec<SmartContextFingerprint> {
    inputs.into_iter().map(smart_context_fingerprint).collect()
}

pub fn smart_context_stabilize_static_context_text(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines = text
        .lines()
        .map(|line| line.trim_end().to_string())
        .filter(|line| !smart_context_static_context_noise_line(line))
        .collect::<Vec<_>>();

    let Some(start) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return String::new();
    };
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .unwrap_or(start);

    lines[start..=end].join("\n")
}

pub fn smart_context_stabilize_static_context_items(
    items: impl IntoIterator<Item = SmartContextStaticContextItem>,
) -> Vec<SmartContextStableStaticContextItem> {
    let mut items = items
        .into_iter()
        .filter_map(|item| {
            let id = smart_context_stabilize_static_context_id(&item.id);
            let canonical_text = smart_context_stabilize_static_context_text(&item.text);
            if id.is_empty() && canonical_text.is_empty() {
                return None;
            }
            let content_hash = smart_context_hash_text(&canonical_text);
            Some(SmartContextStableStaticContextItem {
                id,
                byte_len: canonical_text.len(),
                canonical_text,
                content_hash,
            })
        })
        .collect::<Vec<_>>();

    items.sort_by(smart_context_static_context_item_order);
    items
}

pub fn smart_context_static_context_prompt_cache_fingerprint(
    items: impl IntoIterator<Item = SmartContextStaticContextItem>,
) -> SmartContextStaticContextPromptCacheFingerprint {
    let items = smart_context_stabilize_static_context_items(items);
    let payload = smart_context_static_context_prompt_cache_payload(&items);

    SmartContextStaticContextPromptCacheFingerprint {
        content_hash: format!("scpc:{:016x}", smart_context_fnv1a64(payload.as_bytes())),
        items,
    }
}

pub fn smart_context_fingerprint_delta(
    previous: impl IntoIterator<Item = SmartContextFingerprint>,
    current: impl IntoIterator<Item = SmartContextFingerprint>,
) -> Vec<SmartContextFingerprintChange> {
    let previous = smart_context_fingerprint_map(previous);
    let current = smart_context_fingerprint_map(current);
    let mut keys = BTreeSet::new();
    keys.extend(previous.keys().cloned());
    keys.extend(current.keys().cloned());

    keys.into_iter()
        .filter_map(|key| match (previous.get(&key), current.get(&key)) {
            (None, Some(after)) => Some(SmartContextFingerprintChange::Added {
                fingerprint: after.clone(),
            }),
            (Some(before), None) => Some(SmartContextFingerprintChange::Removed {
                fingerprint: before.clone(),
            }),
            (Some(before), Some(after)) if before.content_hash == after.content_hash => {
                Some(SmartContextFingerprintChange::Unchanged {
                    fingerprint: after.clone(),
                })
            }
            (Some(before), Some(after)) => Some(SmartContextFingerprintChange::Changed {
                before: before.clone(),
                after: after.clone(),
            }),
            (None, None) => None,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextBudgetMode {
    ExactPassThrough,
    LargeLossless,
    ArtifactCondensed,
    MinimalRefsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextBudgetPolicyReason {
    ExactnessRequired,
    StaticContextChanged,
    MissingRehydrateRefs,
    UnknownTokenWindow,
    UnsafeAccounting,
    RecentRewriteSavingsSafe,
    PlentyOfBudget,
    ModerateBudget,
    TightBudget,
    CriticalBudget,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmartContextRecentRewriteSafety {
    pub safe_rewrites: usize,
    pub fallback_rewrites: usize,
    pub saved_tokens: u64,
}

pub const SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS: u64 = 256;
pub const SMART_CONTEXT_REWRITE_TELEMETRY_RECENT_LIMIT: usize = 4;
pub const SMART_CONTEXT_REWRITE_TELEMETRY_MIN_SAMPLE_COUNT: usize = 2;
pub const SMART_CONTEXT_REWRITE_TELEMETRY_RELAX_MAX_AVERAGE_BODY_RATIO_PERCENT: usize = 70;
pub const SMART_CONTEXT_REWRITE_TELEMETRY_TIGHTEN_MIN_AVERAGE_BODY_RATIO_PERCENT: usize = 85;
pub const SMART_CONTEXT_REWRITE_BUDGET_RELAX_NUMERATOR: u64 = 5;
pub const SMART_CONTEXT_REWRITE_BUDGET_RELAX_DENOMINATOR: u64 = 4;
pub const SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_NUMERATOR: u64 = 9;
pub const SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_DENOMINATOR: u64 = 10;
pub const SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_INLINE_BYTES: usize = 256;
pub const SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_REHYDRATE_TOKENS: u64 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SmartContextRewritePolicyBucketKey {
    pub route: Option<String>,
    pub model: Option<String>,
    pub profile: Option<String>,
}

impl From<SmartContextTokenCalibrationBucketKey> for SmartContextRewritePolicyBucketKey {
    fn from(value: SmartContextTokenCalibrationBucketKey) -> Self {
        Self {
            route: value.route,
            model: value.model,
            profile: value.profile,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SmartContextRewriteBudgetDecision {
    #[default]
    NoChange,
    Relax,
    Tighten,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmartContextRewriteTelemetrySample {
    pub body_bytes_before: usize,
    pub body_bytes_after: usize,
    pub estimated_tokens_before: u64,
    pub estimated_tokens_after: u64,
    pub safe: bool,
    pub fallback: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartContextRewriteTelemetryBudgetInput {
    pub recent_rewrite_safety: SmartContextRecentRewriteSafety,
    pub telemetry_samples: Vec<SmartContextRewriteTelemetrySample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextBucketedRewriteTelemetrySample {
    pub bucket_key: Option<SmartContextRewritePolicyBucketKey>,
    pub sample: SmartContextRewriteTelemetrySample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextLearnedRewritePolicyReason {
    BasePolicyExactPassThrough,
    MissingBucketKey,
    MissingBucketConfidence,
    UnsafeRewriteSample,
    RecentSafetyFallback,
    LearnedNoChange,
    LearnedRelax,
    LearnedTighten,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextLearnedRewritePolicyInput {
    pub adaptive_policy_input: SmartContextAdaptiveBudgetPolicyInput,
    pub bucket_key: Option<SmartContextRewritePolicyBucketKey>,
    pub telemetry_samples: Vec<SmartContextBucketedRewriteTelemetrySample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextLearnedRewritePolicy {
    pub policy: SmartContextAdaptiveBudgetPolicy,
    pub decision: SmartContextRewriteBudgetDecision,
    pub reasons: Vec<SmartContextLearnedRewritePolicyReason>,
    pub matching_telemetry_samples: usize,
    pub safety_samples: usize,
}

pub fn smart_context_recent_rewrite_safety_allows_larger_preview(
    safety: &SmartContextRecentRewriteSafety,
) -> bool {
    smart_context_recent_rewrite_safety_budget_decision(safety)
        == SmartContextRewriteBudgetDecision::Relax
}

pub fn smart_context_recent_rewrite_safety_budget_decision(
    safety: &SmartContextRecentRewriteSafety,
) -> SmartContextRewriteBudgetDecision {
    if safety.fallback_rewrites > 0 {
        return SmartContextRewriteBudgetDecision::Tighten;
    }
    if safety.safe_rewrites == 0 {
        return SmartContextRewriteBudgetDecision::NoChange;
    }

    if safety.saved_tokens >= smart_context_recent_rewrite_min_saved_tokens(safety.safe_rewrites) {
        SmartContextRewriteBudgetDecision::Relax
    } else {
        SmartContextRewriteBudgetDecision::Tighten
    }
}

pub fn smart_context_rewrite_telemetry_budget_decision(
    input: SmartContextRewriteTelemetryBudgetInput,
) -> SmartContextRewriteBudgetDecision {
    let recent = input
        .telemetry_samples
        .iter()
        .rev()
        .take(SMART_CONTEXT_REWRITE_TELEMETRY_RECENT_LIMIT)
        .copied()
        .collect::<Vec<_>>();

    if recent.is_empty() {
        return smart_context_recent_rewrite_safety_budget_decision(&input.recent_rewrite_safety);
    }
    if recent
        .iter()
        .any(|sample| sample.fallback || !smart_context_rewrite_telemetry_sample_safe_saved(sample))
    {
        return SmartContextRewriteBudgetDecision::Tighten;
    }
    if recent.len() < SMART_CONTEXT_REWRITE_TELEMETRY_MIN_SAMPLE_COUNT {
        return smart_context_recent_rewrite_safety_budget_decision(&input.recent_rewrite_safety);
    }

    let saved_tokens = recent.iter().fold(0u64, |total, sample| {
        total.saturating_add(
            sample
                .estimated_tokens_before
                .saturating_sub(sample.estimated_tokens_after),
        )
    });
    let average_body_ratio_percent = recent.iter().fold(0usize, |total, sample| {
        total.saturating_add(smart_context_rewrite_body_ratio_percent(
            sample.body_bytes_before,
            sample.body_bytes_after,
        ))
    }) / recent.len();
    let required_saved_tokens = smart_context_recent_rewrite_min_saved_tokens(recent.len());

    if saved_tokens >= required_saved_tokens
        && average_body_ratio_percent
            <= SMART_CONTEXT_REWRITE_TELEMETRY_RELAX_MAX_AVERAGE_BODY_RATIO_PERCENT
    {
        SmartContextRewriteBudgetDecision::Relax
    } else if saved_tokens < required_saved_tokens
        || average_body_ratio_percent
            >= SMART_CONTEXT_REWRITE_TELEMETRY_TIGHTEN_MIN_AVERAGE_BODY_RATIO_PERCENT
    {
        SmartContextRewriteBudgetDecision::Tighten
    } else {
        SmartContextRewriteBudgetDecision::NoChange
    }
}

pub fn smart_context_learned_rewrite_policy(
    input: SmartContextLearnedRewritePolicyInput,
) -> SmartContextLearnedRewritePolicy {
    let SmartContextLearnedRewritePolicyInput {
        mut adaptive_policy_input,
        bucket_key,
        telemetry_samples,
    } = input;
    let recent_rewrite_safety = adaptive_policy_input.recent_rewrite_safety;
    let safety_samples = recent_rewrite_safety
        .safe_rewrites
        .saturating_add(recent_rewrite_safety.fallback_rewrites);
    let available_context_tokens = adaptive_policy_input.accounting.available_context_tokens;

    adaptive_policy_input.recent_rewrite_safety = SmartContextRecentRewriteSafety::default();
    let base_policy = smart_context_adaptive_budget_policy(adaptive_policy_input);
    if base_policy.mode == SmartContextBudgetMode::ExactPassThrough {
        return SmartContextLearnedRewritePolicy {
            policy: base_policy,
            decision: SmartContextRewriteBudgetDecision::NoChange,
            reasons: vec![SmartContextLearnedRewritePolicyReason::BasePolicyExactPassThrough],
            matching_telemetry_samples: 0,
            safety_samples,
        };
    }

    let Some(bucket_key) = bucket_key
        .as_ref()
        .filter(|bucket_key| smart_context_rewrite_policy_bucket_key_complete(bucket_key))
    else {
        return smart_context_learned_rewrite_policy_exact(
            base_policy,
            SmartContextLearnedRewritePolicyReason::MissingBucketKey,
            0,
            safety_samples,
            available_context_tokens,
        );
    };

    let matching_telemetry_samples = telemetry_samples
        .into_iter()
        .filter(|sample| {
            smart_context_rewrite_policy_bucket_key_matches(bucket_key, sample.bucket_key.as_ref())
        })
        .map(|sample| sample.sample)
        .collect::<Vec<_>>();
    let matching_sample_count = matching_telemetry_samples.len();
    let telemetry_confident =
        matching_sample_count >= SMART_CONTEXT_REWRITE_TELEMETRY_MIN_SAMPLE_COUNT;
    let safety_confident = matching_sample_count == 0
        && safety_samples >= SMART_CONTEXT_REWRITE_TELEMETRY_MIN_SAMPLE_COUNT;

    if matching_telemetry_samples
        .iter()
        .any(|sample| sample.fallback || !smart_context_rewrite_telemetry_sample_safe_saved(sample))
        || recent_rewrite_safety.fallback_rewrites > 0
    {
        return smart_context_learned_rewrite_policy_exact(
            base_policy,
            SmartContextLearnedRewritePolicyReason::UnsafeRewriteSample,
            matching_sample_count,
            safety_samples,
            available_context_tokens,
        );
    }
    if !telemetry_confident && !safety_confident {
        return smart_context_learned_rewrite_policy_exact(
            base_policy,
            SmartContextLearnedRewritePolicyReason::MissingBucketConfidence,
            matching_sample_count,
            safety_samples,
            available_context_tokens,
        );
    }

    let decision = if telemetry_confident {
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            recent_rewrite_safety,
            telemetry_samples: matching_telemetry_samples,
        })
    } else {
        smart_context_recent_rewrite_safety_budget_decision(&recent_rewrite_safety)
    };
    let policy = smart_context_apply_rewrite_budget_decision(
        base_policy,
        decision,
        available_context_tokens,
    );
    let mut reasons = Vec::new();
    if safety_confident {
        reasons.push(SmartContextLearnedRewritePolicyReason::RecentSafetyFallback);
    }
    reasons.push(match decision {
        SmartContextRewriteBudgetDecision::NoChange => {
            SmartContextLearnedRewritePolicyReason::LearnedNoChange
        }
        SmartContextRewriteBudgetDecision::Relax => {
            SmartContextLearnedRewritePolicyReason::LearnedRelax
        }
        SmartContextRewriteBudgetDecision::Tighten => {
            SmartContextLearnedRewritePolicyReason::LearnedTighten
        }
    });

    SmartContextLearnedRewritePolicy {
        policy,
        decision,
        reasons,
        matching_telemetry_samples: matching_sample_count,
        safety_samples,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextAdaptiveBudgetPolicyInput {
    pub exactness_guard: SmartContextExactnessGuard,
    pub accounting: SmartContextObservedTokenAccounting,
    pub recent_rewrite_safety: SmartContextRecentRewriteSafety,
    pub static_context_changed: bool,
    pub missing_rehydrate_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextAdaptiveBudgetPolicy {
    pub tier: SmartContextTokenBudgetTier,
    pub mode: SmartContextBudgetMode,
    pub max_inline_bytes: usize,
    pub max_inline_tool_output_bytes: usize,
    pub max_rehydrate_tokens: u64,
    pub reasons: Vec<SmartContextBudgetPolicyReason>,
}

pub fn smart_context_adaptive_budget_policy(
    input: SmartContextAdaptiveBudgetPolicyInput,
) -> SmartContextAdaptiveBudgetPolicy {
    let tier = smart_context_token_budget_tier_from_accounting(&input.accounting);
    let mut reasons = Vec::new();
    let available_context_tokens = input.accounting.available_context_tokens;

    if input.exactness_guard.decision == SmartContextExactnessDecision::RequireExact {
        reasons.push(SmartContextBudgetPolicyReason::ExactnessRequired);
    }
    if input.static_context_changed {
        reasons.push(SmartContextBudgetPolicyReason::StaticContextChanged);
    }
    if input
        .missing_rehydrate_refs
        .iter()
        .any(|value| non_empty(value))
    {
        reasons.push(SmartContextBudgetPolicyReason::MissingRehydrateRefs);
    }
    if input.accounting.available_context_tokens.is_none() {
        reasons.push(SmartContextBudgetPolicyReason::UnknownTokenWindow);
    }
    if input
        .accounting
        .accounting_risks
        .iter()
        .any(|risk| *risk != SmartContextTokenAccountingRisk::UnknownTokenWindow)
    {
        reasons.push(SmartContextBudgetPolicyReason::UnsafeAccounting);
    }

    if reasons.iter().any(|reason| {
        matches!(
            reason,
            SmartContextBudgetPolicyReason::ExactnessRequired
                | SmartContextBudgetPolicyReason::StaticContextChanged
                | SmartContextBudgetPolicyReason::MissingRehydrateRefs
                | SmartContextBudgetPolicyReason::UnknownTokenWindow
                | SmartContextBudgetPolicyReason::UnsafeAccounting
        )
    }) {
        return SmartContextAdaptiveBudgetPolicy {
            tier,
            mode: SmartContextBudgetMode::ExactPassThrough,
            max_inline_bytes: usize::MAX,
            max_inline_tool_output_bytes: usize::MAX,
            max_rehydrate_tokens: available_context_tokens.unwrap_or(u64::MAX),
            reasons,
        };
    }

    let rewrite_budget_decision =
        smart_context_recent_rewrite_safety_budget_decision(&input.recent_rewrite_safety);
    let larger_preview_safe = rewrite_budget_decision == SmartContextRewriteBudgetDecision::Relax;
    let (mode, max_inline_tool_output_bytes, max_rehydrate_tokens, tier_reason) = match tier {
        SmartContextTokenBudgetTier::Exact => (
            SmartContextBudgetMode::ExactPassThrough,
            usize::MAX,
            input
                .accounting
                .available_context_tokens
                .unwrap_or(u64::MAX),
            SmartContextBudgetPolicyReason::PlentyOfBudget,
        ),
        SmartContextTokenBudgetTier::Large => (
            SmartContextBudgetMode::LargeLossless,
            if larger_preview_safe {
                64 * 1024
            } else {
                32 * 1024
            },
            12_000,
            SmartContextBudgetPolicyReason::ModerateBudget,
        ),
        SmartContextTokenBudgetTier::Condensed => (
            SmartContextBudgetMode::ArtifactCondensed,
            8 * 1024,
            4_000,
            SmartContextBudgetPolicyReason::TightBudget,
        ),
        SmartContextTokenBudgetTier::Minimal => (
            SmartContextBudgetMode::MinimalRefsOnly,
            1024,
            1_000,
            SmartContextBudgetPolicyReason::CriticalBudget,
        ),
    };
    reasons.push(tier_reason);
    if larger_preview_safe && matches!(tier, SmartContextTokenBudgetTier::Large) {
        reasons.push(SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe);
    }
    let max_rehydrate_tokens = available_context_tokens
        .map(|available| max_rehydrate_tokens.min(available))
        .unwrap_or(max_rehydrate_tokens);

    let policy = SmartContextAdaptiveBudgetPolicy {
        tier,
        mode,
        max_inline_bytes: max_inline_tool_output_bytes,
        max_inline_tool_output_bytes,
        max_rehydrate_tokens,
        reasons,
    };
    smart_context_apply_rewrite_budget_decision(
        policy,
        rewrite_budget_decision,
        available_context_tokens,
    )
}

pub fn smart_context_apply_rewrite_budget_decision(
    mut policy: SmartContextAdaptiveBudgetPolicy,
    decision: SmartContextRewriteBudgetDecision,
    available_context_tokens: Option<u64>,
) -> SmartContextAdaptiveBudgetPolicy {
    if policy.mode == SmartContextBudgetMode::ExactPassThrough {
        return policy;
    }

    match decision {
        SmartContextRewriteBudgetDecision::NoChange => {}
        SmartContextRewriteBudgetDecision::Relax => {
            policy.max_inline_tool_output_bytes = smart_context_relaxed_inline_budget(
                policy.tier,
                policy.max_inline_tool_output_bytes,
            );
            policy.max_inline_bytes = policy.max_inline_tool_output_bytes;
            policy.max_rehydrate_tokens =
                smart_context_relaxed_rehydrate_budget(policy.max_rehydrate_tokens);
        }
        SmartContextRewriteBudgetDecision::Tighten => {
            policy.max_inline_tool_output_bytes =
                smart_context_tightened_inline_budget(policy.max_inline_tool_output_bytes);
            policy.max_inline_bytes = policy.max_inline_tool_output_bytes;
            policy.max_rehydrate_tokens =
                smart_context_tightened_rehydrate_budget(policy.max_rehydrate_tokens);
        }
    }

    if let Some(available) = available_context_tokens {
        policy.max_rehydrate_tokens = policy.max_rehydrate_tokens.min(available);
    }
    policy
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextRegressionSelfCheckDecision {
    Pass,
    FallbackExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextRegressionSelfCheckReason {
    ExactnessRequiredButPayloadChanged,
    TokenBudgetDidNotImprove,
    CriticalSignalDropped,
    MissingRehydrateRefs,
    EmptyAfterPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRegressionSelfCheckInput {
    pub exactness_guard: SmartContextExactnessGuard,
    pub before_hash: String,
    pub after_hash: String,
    pub before_estimated_tokens: u64,
    pub after_estimated_tokens: u64,
    pub before_critical_signal_count: usize,
    pub after_critical_signal_count: usize,
    pub missing_rehydrate_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRegressionSelfCheck {
    pub decision: SmartContextRegressionSelfCheckDecision,
    pub reasons: Vec<SmartContextRegressionSelfCheckReason>,
    pub saved_tokens: u64,
    pub before_hash: String,
    pub after_hash: String,
}

pub fn smart_context_regression_self_check(
    input: SmartContextRegressionSelfCheckInput,
) -> SmartContextRegressionSelfCheck {
    let mut reasons = Vec::new();
    let payload_changed = input.before_hash != input.after_hash;

    if input.exactness_guard.decision == SmartContextExactnessDecision::RequireExact
        && payload_changed
    {
        reasons.push(SmartContextRegressionSelfCheckReason::ExactnessRequiredButPayloadChanged);
    }
    if payload_changed && input.after_estimated_tokens >= input.before_estimated_tokens {
        reasons.push(SmartContextRegressionSelfCheckReason::TokenBudgetDidNotImprove);
    }
    if input.after_critical_signal_count < input.before_critical_signal_count {
        reasons.push(SmartContextRegressionSelfCheckReason::CriticalSignalDropped);
    }
    if input
        .missing_rehydrate_refs
        .iter()
        .any(|value| non_empty(value))
    {
        reasons.push(SmartContextRegressionSelfCheckReason::MissingRehydrateRefs);
    }
    if input.before_estimated_tokens > 0 && input.after_estimated_tokens == 0 {
        reasons.push(SmartContextRegressionSelfCheckReason::EmptyAfterPayload);
    }

    SmartContextRegressionSelfCheck {
        decision: if reasons.is_empty() {
            SmartContextRegressionSelfCheckDecision::Pass
        } else {
            SmartContextRegressionSelfCheckDecision::FallbackExact
        },
        reasons,
        saved_tokens: input
            .before_estimated_tokens
            .saturating_sub(input.after_estimated_tokens),
        before_hash: input.before_hash,
        after_hash: input.after_hash,
    }
}

pub fn smart_context_hash_text(text: &str) -> String {
    format!("sc:{:016x}", smart_context_fnv1a64(text.as_bytes()))
}

pub fn smart_context_normalized_command_output_hash_text(text: &str) -> String {
    let normalized = smart_context_normalize_volatile_command_output(text);
    format!(
        "scv:{:016x}",
        smart_context_fnv1a64(normalized.as_ref().as_bytes())
    )
}

pub fn smart_context_normalize_volatile_command_output(text: &str) -> Cow<'_, str> {
    let mut normalized = String::with_capacity(text.len());
    let mut changed = false;
    let mut index = 0usize;

    while index < text.len() {
        let rest = &text[index..];
        let previous = smart_context_previous_char(text, index);

        if let Some(len) = smart_context_ansi_escape_len(rest) {
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_temp_path_len(rest) {
            normalized.push_str("<tmp-path>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_timestamp_len(rest, previous) {
            normalized.push_str("<timestamp>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_progress_counter_len(rest, previous) {
            normalized.push_str("<progress>");
            index += len;
            changed = true;
            continue;
        }
        if let Some((len, replacement)) =
            smart_context_labeled_random_id_replacement(rest, previous)
        {
            normalized.push_str(&replacement);
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_uuid_len(rest, previous) {
            normalized.push_str("<id>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_duration_len(rest, previous) {
            normalized.push_str("<duration>");
            index += len;
            changed = true;
            continue;
        }

        let ch = rest.chars().next().expect("index is within string");
        normalized.push(ch);
        index += ch.len_utf8();
    }

    if changed {
        Cow::Owned(normalized)
    } else {
        Cow::Borrowed(text)
    }
}

fn smart_context_effective_input_source(
    current_input_tokens: u64,
    estimated_current_request_tokens: u64,
    current_request_accounted_tokens: u64,
    last_accounted_input_tokens: u64,
    effective_input_tokens: u64,
) -> SmartContextTokenAccountingSource {
    if effective_input_tokens == 0 {
        SmartContextTokenAccountingSource::Unknown
    } else if last_accounted_input_tokens > current_request_accounted_tokens {
        SmartContextTokenAccountingSource::ObservedHistory
    } else if current_input_tokens >= estimated_current_request_tokens && current_input_tokens > 0 {
        SmartContextTokenAccountingSource::CurrentRequestTokens
    } else if estimated_current_request_tokens > 0 {
        SmartContextTokenAccountingSource::CurrentRequestBodyEstimate
    } else if last_accounted_input_tokens > 0 {
        SmartContextTokenAccountingSource::ObservedHistory
    } else {
        SmartContextTokenAccountingSource::Unknown
    }
}

fn smart_context_token_accounting_risks(
    model_context_window_tokens: Option<u64>,
    reserved_output_tokens: u64,
    effective_input_source: SmartContextTokenAccountingSource,
) -> Vec<SmartContextTokenAccountingRisk> {
    let mut risks = Vec::new();

    match model_context_window_tokens {
        Some(0) => risks.push(SmartContextTokenAccountingRisk::ZeroContextWindow),
        Some(window) if reserved_output_tokens >= window => {
            risks.push(SmartContextTokenAccountingRisk::ReservedOutputConsumesWindow);
        }
        Some(_) => {}
        None => risks.push(SmartContextTokenAccountingRisk::UnknownTokenWindow),
    }
    if effective_input_source == SmartContextTokenAccountingSource::Unknown {
        risks.push(SmartContextTokenAccountingRisk::UnknownCurrentRequestAccounting);
    }

    risks
}

fn smart_context_u64_budget_tier(available_tokens: u64) -> SmartContextTokenBudgetTier {
    if available_tokens > usize::MAX as u64 {
        SmartContextTokenBudgetTier::Exact
    } else {
        smart_context_token_budget_tier(available_tokens as usize)
    }
}

fn smart_context_u64_saturating_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

fn smart_context_memory_capsule_policy_allows_unbounded_budget(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> bool {
    smart_context_accounting_safe_for_adaptive_policy(accounting)
        && policy.mode == SmartContextBudgetMode::ExactPassThrough
        && policy.tier == SmartContextTokenBudgetTier::Exact
        && policy.reasons == [SmartContextBudgetPolicyReason::PlentyOfBudget]
}

fn smart_context_memory_capsule_policy_allows_bounded_budget(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> bool {
    smart_context_accounting_safe_for_adaptive_policy(accounting)
        && !policy.reasons.iter().any(|reason| {
            matches!(
                reason,
                SmartContextBudgetPolicyReason::ExactnessRequired
                    | SmartContextBudgetPolicyReason::StaticContextChanged
                    | SmartContextBudgetPolicyReason::MissingRehydrateRefs
                    | SmartContextBudgetPolicyReason::UnknownTokenWindow
                    | SmartContextBudgetPolicyReason::UnsafeAccounting
            )
        })
}

fn smart_context_recent_rewrite_min_saved_tokens(rewrite_count: usize) -> u64 {
    SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS
        .saturating_mul(u64::try_from(rewrite_count).unwrap_or(u64::MAX))
}

fn smart_context_learned_rewrite_policy_exact(
    base_policy: SmartContextAdaptiveBudgetPolicy,
    reason: SmartContextLearnedRewritePolicyReason,
    matching_telemetry_samples: usize,
    safety_samples: usize,
    available_context_tokens: Option<u64>,
) -> SmartContextLearnedRewritePolicy {
    SmartContextLearnedRewritePolicy {
        policy: SmartContextAdaptiveBudgetPolicy {
            tier: base_policy.tier,
            mode: SmartContextBudgetMode::ExactPassThrough,
            max_inline_bytes: usize::MAX,
            max_inline_tool_output_bytes: usize::MAX,
            max_rehydrate_tokens: available_context_tokens.unwrap_or(u64::MAX),
            reasons: base_policy.reasons,
        },
        decision: SmartContextRewriteBudgetDecision::NoChange,
        reasons: vec![reason],
        matching_telemetry_samples,
        safety_samples,
    }
}

fn smart_context_rewrite_policy_bucket_key_complete(
    bucket_key: &SmartContextRewritePolicyBucketKey,
) -> bool {
    bucket_key.route.as_deref().is_some_and(non_empty)
        && bucket_key
            .model
            .as_deref()
            .and_then(|model| smart_context_token_calibration_normalized_model(Some(model)))
            .is_some()
        && bucket_key.profile.as_deref().is_some_and(non_empty)
}

fn smart_context_rewrite_policy_bucket_key_matches(
    target: &SmartContextRewritePolicyBucketKey,
    sample: Option<&SmartContextRewritePolicyBucketKey>,
) -> bool {
    let Some(sample) = sample else {
        return false;
    };
    smart_context_rewrite_policy_field_matches(&target.route, &sample.route)
        && smart_context_rewrite_policy_model_matches(&target.model, &sample.model)
        && smart_context_rewrite_policy_field_matches(&target.profile, &sample.profile)
}

fn smart_context_rewrite_policy_field_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    target.as_deref().is_some_and(non_empty) && target == sample
}

fn smart_context_rewrite_policy_model_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    let Some(target) = smart_context_token_calibration_normalized_model(target.as_deref()) else {
        return false;
    };
    let Some(sample) = smart_context_token_calibration_normalized_model(sample.as_deref()) else {
        return false;
    };
    target == sample
}

fn smart_context_rewrite_telemetry_sample_safe_saved(
    sample: &SmartContextRewriteTelemetrySample,
) -> bool {
    sample.safe
        && sample.estimated_tokens_after < sample.estimated_tokens_before
        && sample.body_bytes_after < sample.body_bytes_before
}

fn smart_context_rewrite_body_ratio_percent(
    body_bytes_before: usize,
    body_bytes_after: usize,
) -> usize {
    if body_bytes_before == 0 {
        return 100;
    }
    body_bytes_after.saturating_mul(100) / body_bytes_before
}

fn smart_context_relaxed_inline_budget(tier: SmartContextTokenBudgetTier, value: usize) -> usize {
    if value == 0 || value == usize::MAX {
        return value;
    }

    if tier == SmartContextTokenBudgetTier::Large {
        let cap = 64 * 1024;
        if value >= cap {
            return value;
        }
        return value.saturating_mul(2).min(cap).max(value);
    }

    smart_context_scale_usize_ceil(
        value,
        SMART_CONTEXT_REWRITE_BUDGET_RELAX_NUMERATOR,
        SMART_CONTEXT_REWRITE_BUDGET_RELAX_DENOMINATOR,
    )
    .max(value)
}

fn smart_context_relaxed_rehydrate_budget(value: u64) -> u64 {
    if value == 0 || value == u64::MAX {
        return value;
    }
    smart_context_scale_u64_ceil(
        value,
        SMART_CONTEXT_REWRITE_BUDGET_RELAX_NUMERATOR,
        SMART_CONTEXT_REWRITE_BUDGET_RELAX_DENOMINATOR,
    )
    .max(value)
}

fn smart_context_tightened_inline_budget(value: usize) -> usize {
    if value <= SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_INLINE_BYTES {
        return value;
    }
    smart_context_scale_usize_floor(
        value,
        SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_NUMERATOR,
        SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_DENOMINATOR,
    )
    .max(SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_INLINE_BYTES)
    .min(value)
}

fn smart_context_tightened_rehydrate_budget(value: u64) -> u64 {
    if value <= SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_REHYDRATE_TOKENS {
        return value;
    }
    smart_context_scale_u64_floor(
        value,
        SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_NUMERATOR,
        SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_DENOMINATOR,
    )
    .max(SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_REHYDRATE_TOKENS)
    .min(value)
}

fn smart_context_scale_usize_ceil(value: usize, numerator: u64, denominator: u64) -> usize {
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    smart_context_u64_saturating_usize(smart_context_scale_u64_ceil(value, numerator, denominator))
}

fn smart_context_scale_usize_floor(value: usize, numerator: u64, denominator: u64) -> usize {
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    smart_context_u64_saturating_usize(smart_context_scale_u64_floor(value, numerator, denominator))
}

fn smart_context_scale_u64_ceil(value: u64, numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return value;
    }
    value
        .saturating_mul(numerator)
        .saturating_add(denominator - 1)
        / denominator
}

fn smart_context_scale_u64_floor(value: u64, numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return value;
    }
    value.saturating_mul(numerator) / denominator
}

fn smart_context_fingerprint_map(
    fingerprints: impl IntoIterator<Item = SmartContextFingerprint>,
) -> BTreeMap<(SmartContextFingerprintKind, String), SmartContextFingerprint> {
    fingerprints
        .into_iter()
        .map(|fingerprint| ((fingerprint.kind, fingerprint.id.clone()), fingerprint))
        .collect()
}

fn smart_context_available_artifacts_by_hash_and_len(
    artifacts: impl IntoIterator<Item = SmartContextArtifactRef>,
) -> BTreeMap<(String, usize), SmartContextArtifactRef> {
    let mut artifacts = artifacts
        .into_iter()
        .filter(|artifact| non_empty(&artifact.id) && non_empty(&artifact.content_hash))
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        left.content_hash
            .cmp(&right.content_hash)
            .then_with(|| left.byte_len.cmp(&right.byte_len))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut available = BTreeMap::new();
    for artifact in artifacts {
        available
            .entry((artifact.content_hash.clone(), artifact.byte_len))
            .or_insert(artifact);
    }

    available
}

fn smart_context_command_output_keep_exact(
    record: SmartContextCommandOutputCacheRecord,
    output: String,
    reason: SmartContextCommandOutputCacheKeepReason,
    summary: Option<String>,
) -> SmartContextCommandOutputCacheRewrite {
    SmartContextCommandOutputCacheRewrite {
        record,
        output,
        action: SmartContextCommandOutputCacheAction::KeepExact { reason, summary },
    }
}

fn smart_context_command_output_cache_record_valid(
    record: &SmartContextCommandOutputCacheRecord,
) -> bool {
    non_empty(&record.content_hash) && record.byte_len > 0
}

fn smart_context_command_output_unchanged_summary(
    current: &SmartContextCommandOutputCacheRecord,
    previous: &SmartContextCommandOutputCacheRecord,
    text: &str,
) -> String {
    let mut summary = format!(
        "psc cmdout unchanged id={} ref={} h={} b={} tok={}; volatile-normalized repeat omitted",
        smart_context_command_output_label(&current.id),
        smart_context_command_output_label(&previous.id),
        current.content_hash,
        current.byte_len,
        current.estimated_tokens,
    );

    let critical_signals = smart_context_command_output_critical_signals(text);
    if critical_signals.count > 0 {
        summary.push('\n');
        summary.push_str(&format!(
            "psc cmdout critical n={}; exact signals available via h={}",
            critical_signals.count, current.content_hash
        ));
        for sample in critical_signals.samples {
            summary.push('\n');
            summary.push_str("sig: ");
            summary.push_str(&sample);
        }
    }

    summary
}

fn smart_context_command_output_changed_summary(
    current: &SmartContextCommandOutputCacheRecord,
    previous: &SmartContextCommandOutputCacheRecord,
) -> String {
    let byte_delta = smart_context_signed_delta(current.byte_len, previous.byte_len);
    let token_delta =
        smart_context_signed_delta_u64(current.estimated_tokens, previous.estimated_tokens);
    format!(
        "psc cmdout changed id={} ref={} old_h={} new_h={} old_b={} new_b={} old_tok={} new_tok={} db={} dtok={}; exact output kept",
        smart_context_command_output_label(&current.id),
        smart_context_command_output_label(&previous.id),
        previous.content_hash,
        current.content_hash,
        previous.byte_len,
        current.byte_len,
        previous.estimated_tokens,
        current.estimated_tokens,
        byte_delta,
        token_delta,
    )
}

fn smart_context_command_output_label(value: &str) -> String {
    let mut label = String::new();
    for value in value.trim().chars() {
        if label.len() >= 96 {
            break;
        }
        let replacement = if value.is_ascii_alphanumeric()
            || matches!(value, ':' | '_' | '-' | '.' | '/' | '#' | '@')
        {
            value
        } else {
            '_'
        };
        label.push(replacement);
    }

    if label.is_empty() {
        "unknown".to_string()
    } else {
        label
    }
}

fn smart_context_command_output_line_has_critical_signal(line: &str) -> bool {
    let line = line.to_ascii_lowercase();
    [
        "error",
        "failed",
        "failure",
        "panic",
        "exception",
        "traceback",
        "fatal",
        "denied",
        "not found",
        "segmentation fault",
        "abort",
        "timeout",
    ]
    .iter()
    .any(|signal| line.contains(signal))
}

fn smart_context_command_output_signal_sample(line: &str) -> String {
    let line = line.trim();
    let mut sample =
        smart_context_summary_prefix(line, SMART_CONTEXT_COMMAND_OUTPUT_CRITICAL_SAMPLE_BYTES);
    if sample.len() < line.len() {
        sample.push_str("...");
    }
    sample
}

fn smart_context_signed_delta(current: usize, previous: usize) -> i128 {
    let current = i128::try_from(current).unwrap_or(i128::MAX);
    let previous = i128::try_from(previous).unwrap_or(i128::MAX);
    current.saturating_sub(previous)
}

fn smart_context_signed_delta_u64(current: u64, previous: u64) -> i128 {
    let current = i128::from(current);
    let previous = i128::from(previous);
    current.saturating_sub(previous)
}

fn smart_context_stabilize_static_context_id(id: &str) -> String {
    id.trim().replace('\\', "/")
}

fn smart_context_static_context_item_order(
    left: &SmartContextStableStaticContextItem,
    right: &SmartContextStableStaticContextItem,
) -> Ordering {
    smart_context_static_context_item_order_key(&left.id)
        .cmp(&smart_context_static_context_item_order_key(&right.id))
        .then_with(|| left.id.cmp(&right.id))
        .then_with(|| left.content_hash.cmp(&right.content_hash))
        .then_with(|| left.byte_len.cmp(&right.byte_len))
        .then_with(|| left.canonical_text.cmp(&right.canonical_text))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SmartContextStaticContextItemOrderKey {
    group: u8,
    input_index: usize,
    role_rank: u8,
    generic_id: String,
}

fn smart_context_static_context_item_order_key(id: &str) -> SmartContextStaticContextItemOrderKey {
    match id {
        "instructions" => smart_context_static_context_order_key(0, 0, 0, ""),
        "system" => smart_context_static_context_order_key(1, 0, 0, ""),
        "developer" => smart_context_static_context_order_key(2, 0, 0, ""),
        _ => smart_context_input_static_context_order_key(id)
            .unwrap_or_else(|| smart_context_static_context_order_key(100, 0, 0, id)),
    }
}

fn smart_context_input_static_context_order_key(
    id: &str,
) -> Option<SmartContextStaticContextItemOrderKey> {
    let rest = id.strip_prefix("input[")?;
    let (index, rest) = rest.split_once("].")?;
    let input_index = index.parse::<usize>().ok()?;
    let role_rank = match rest {
        "system" => 0,
        "developer" => 1,
        _ => return None,
    };
    Some(smart_context_static_context_order_key(
        3,
        input_index,
        role_rank,
        "",
    ))
}

fn smart_context_static_context_order_key(
    group: u8,
    input_index: usize,
    role_rank: u8,
    generic_id: &str,
) -> SmartContextStaticContextItemOrderKey {
    SmartContextStaticContextItemOrderKey {
        group,
        input_index,
        role_rank,
        generic_id: generic_id.to_string(),
    }
}

fn smart_context_static_context_prompt_cache_payload(
    items: &[SmartContextStableStaticContextItem],
) -> String {
    let mut payload = String::from("prodex-smart-context-static-prompt-cache-v1\n");
    for item in items {
        payload.push_str("id-bytes:");
        payload.push_str(&item.id.len().to_string());
        payload.push('\n');
        payload.push_str(&item.id);
        payload.push('\n');
        payload.push_str("text-bytes:");
        payload.push_str(&item.byte_len.to_string());
        payload.push('\n');
        payload.push_str(&item.canonical_text);
        payload.push('\n');
    }
    payload
}

fn smart_context_static_context_noise_line(line: &str) -> bool {
    let mut value = line.trim();
    if let Some(inner) = value
        .strip_prefix("<!--")
        .and_then(|value| value.strip_suffix("-->"))
    {
        value = inner.trim();
    }

    for prefix in ["//", "#", ";"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest.trim_start();
            break;
        }
    }

    let Some((key, noise_value)) = value.split_once(':').or_else(|| value.split_once('=')) else {
        return false;
    };
    let key = smart_context_static_context_noise_key(key);
    if !smart_context_static_context_noise_key_is_volatile(&key) {
        return false;
    }

    matches!(
        key.as_str(),
        "run id" | "request id" | "trace id" | "session id"
    ) || smart_context_static_context_noise_value_looks_volatile(noise_value)
}

fn smart_context_static_context_noise_key(key: &str) -> String {
    let lower = key.trim().to_ascii_lowercase();
    let mut normalized = String::new();
    let mut previous_space = false;
    for value in lower.chars() {
        let value = match value {
            '-' | '_' => ' ',
            value => value,
        };
        if value.is_whitespace() {
            if !previous_space {
                normalized.push(' ');
                previous_space = true;
            }
        } else {
            normalized.push(value);
            previous_space = false;
        }
    }

    normalized
        .trim()
        .strip_prefix("prodex ")
        .unwrap_or(normalized.trim())
        .to_string()
}

fn smart_context_static_context_noise_key_is_volatile(key: &str) -> bool {
    matches!(
        key,
        "generated"
            | "generated at"
            | "generated on"
            | "last generated"
            | "last generated at"
            | "timestamp"
            | "current date"
            | "current time"
            | "current datetime"
            | "as of"
            | "last updated"
            | "updated at"
            | "run id"
            | "request id"
            | "trace id"
            | "session id"
    )
}

fn smart_context_static_context_noise_value_looks_volatile(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().any(|value| value.is_ascii_digit()) {
        return true;
    }

    matches!(
        value.to_ascii_lowercase().as_str(),
        "now" | "today" | "yesterday" | "tomorrow"
    )
}

fn smart_context_previous_char(text: &str, index: usize) -> Option<char> {
    if index == 0 {
        None
    } else {
        text[..index].chars().next_back()
    }
}

fn smart_context_ansi_escape_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.first().copied()? != 0x1b {
        return None;
    }

    match bytes.get(1).copied() {
        Some(b'[') => bytes
            .iter()
            .enumerate()
            .skip(2)
            .find(|(_, byte)| (0x40u8..=0x7e).contains(*byte))
            .map(|(index, _)| index + 1)
            .or(Some(bytes.len())),
        Some(b']') => {
            let mut index = 2usize;
            while index < bytes.len() {
                if bytes[index] == 0x07 {
                    return Some(index + 1);
                }
                if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
                    return Some(index + 2);
                }
                index += 1;
            }
            Some(bytes.len())
        }
        Some(_) => Some(2.min(bytes.len())),
        None => Some(1),
    }
}

fn smart_context_temp_path_len(text: &str) -> Option<usize> {
    for prefix in [
        "/tmp/",
        "/var/tmp/",
        "/private/tmp/",
        "/var/folders/",
        "$TMPDIR/",
        "%TEMP%\\",
        "%TMP%\\",
    ] {
        if text.starts_with(prefix) {
            return Some(smart_context_path_token_len(text));
        }
    }

    let bytes = text.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
    {
        let token_len = smart_context_path_token_len(text);
        let token = text[..token_len].replace('/', "\\").to_ascii_lowercase();
        if token.contains("\\appdata\\local\\temp\\") || token.starts_with("c:\\temp\\") {
            return Some(token_len);
        }
    }

    None
}

fn smart_context_path_token_len(text: &str) -> usize {
    text.char_indices()
        .find(|(index, ch)| *index > 0 && smart_context_path_token_delimiter(*ch))
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

fn smart_context_path_token_delimiter(ch: char) -> bool {
    ch.is_whitespace()
        || ch.is_control()
        || matches!(
            ch,
            '"' | '\'' | '`' | '<' | '>' | '|' | '(' | ')' | '[' | ']' | '{' | '}'
        )
}

fn smart_context_timestamp_len(text: &str, previous: Option<char>) -> Option<usize> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    let bytes = text.as_bytes();
    if bytes.len() < 16
        || !smart_context_ascii_digits(bytes, 0, 4)
        || bytes[4] != b'-'
        || !smart_context_ascii_digits(bytes, 5, 2)
        || bytes[7] != b'-'
        || !smart_context_ascii_digits(bytes, 8, 2)
        || !matches!(bytes[10], b'T' | b' ')
        || !smart_context_ascii_digits(bytes, 11, 2)
        || bytes[13] != b':'
        || !smart_context_ascii_digits(bytes, 14, 2)
    {
        return None;
    }

    let mut index = 16usize;
    if bytes.get(index) == Some(&b':') {
        if !smart_context_ascii_digits(bytes, index + 1, 2) {
            return None;
        }
        index += 3;
        if bytes.get(index) == Some(&b'.') {
            let fraction_start = index + 1;
            index = fraction_start;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
            if index == fraction_start {
                return None;
            }
        }
    }

    if bytes.get(index) == Some(&b'Z') {
        index += 1;
    } else if matches!(bytes.get(index), Some(b'+') | Some(b'-')) {
        if !smart_context_ascii_digits(bytes, index + 1, 2) {
            return None;
        }
        index += 3;
        if bytes.get(index) == Some(&b':') {
            if !smart_context_ascii_digits(bytes, index + 1, 2) {
                return None;
            }
            index += 3;
        } else if smart_context_ascii_digits(bytes, index, 2) {
            index += 2;
        }
    }

    smart_context_after_token_boundary(text, index).then_some(index)
}

fn smart_context_progress_counter_len(text: &str, previous: Option<char>) -> Option<usize> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    smart_context_percent_progress_len(text)
        .or_else(|| smart_context_slash_progress_len(text))
        .or_else(|| smart_context_of_progress_len(text))
}

fn smart_context_percent_progress_len(text: &str) -> Option<usize> {
    let (mut index, _) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    if text.as_bytes().get(index) == Some(&b'.') {
        let fraction_start = index + 1;
        index = fraction_start;
        while text.as_bytes().get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    if text.as_bytes().get(index) != Some(&b'%') {
        return None;
    }
    index += 1;
    smart_context_after_token_boundary(text, index).then_some(index)
}

fn smart_context_slash_progress_len(text: &str) -> Option<usize> {
    let (left_end, left) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    if text.as_bytes().get(left_end) != Some(&b'/') {
        return None;
    }
    let (right_end, right) = smart_context_parse_unsigned_ascii_int(text, left_end + 1)?;
    if left > right || right == 0 {
        return None;
    }
    smart_context_after_token_boundary(text, right_end).then_some(right_end)
}

fn smart_context_of_progress_len(text: &str) -> Option<usize> {
    let (left_end, left) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    let mut index = smart_context_skip_ascii_spaces(text, left_end);
    if !text[index..].starts_with("of") {
        return None;
    }
    index += 2;
    if !text
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        return None;
    }
    index = smart_context_skip_ascii_spaces(text, index);
    let (right_end, right) = smart_context_parse_unsigned_ascii_int(text, index)?;
    if left > right || right == 0 {
        return None;
    }
    smart_context_after_token_boundary(text, right_end).then_some(right_end)
}

fn smart_context_labeled_random_id_replacement(
    text: &str,
    previous: Option<char>,
) -> Option<(usize, String)> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    let mut separator_index = None;
    for (index, ch) in text.char_indices().take(64) {
        if matches!(ch, ':' | '=') {
            separator_index = Some(index);
            break;
        }
        if !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ' ')) {
            return None;
        }
    }
    let separator_index = separator_index?;
    let key = smart_context_static_context_noise_key(&text[..separator_index]);
    if !smart_context_random_id_key_is_volatile(&key) {
        return None;
    }

    let mut value_start = smart_context_skip_ascii_spaces(text, separator_index + 1);
    if matches!(text.as_bytes().get(value_start), Some(b'"') | Some(b'\'')) {
        value_start += 1;
    }
    let value_len = smart_context_random_id_value_len(&text[value_start..])?;
    let value_end = value_start + value_len;
    let value = &text[value_start..value_end];
    if !smart_context_random_id_value_looks_volatile_for_key(&key, value) {
        return None;
    }

    let mut replacement = String::with_capacity(value_start + 4);
    replacement.push_str(&text[..value_start]);
    replacement.push_str("<id>");
    Some((value_end, replacement))
}

fn smart_context_uuid_len(text: &str, previous: Option<char>) -> Option<usize> {
    if !smart_context_token_boundary(previous) || text.len() < 36 {
        return None;
    }
    let candidate = &text[..36];
    if !smart_context_uuid_token_exact(candidate) || !smart_context_after_token_boundary(text, 36) {
        return None;
    }
    Some(36)
}

fn smart_context_duration_len(text: &str, previous: Option<char>) -> Option<usize> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    let (mut index, _) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    if text.as_bytes().get(index) == Some(&b'.') {
        let fraction_start = index + 1;
        index = fraction_start;
        while text.as_bytes().get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    index = smart_context_skip_ascii_spaces(text, index);
    let unit_len = smart_context_duration_unit_len(&text[index..])?;
    let end = index + unit_len;
    smart_context_after_token_boundary(text, end).then_some(end)
}

fn smart_context_duration_unit_len(text: &str) -> Option<usize> {
    for unit in [
        "milliseconds",
        "millisecond",
        "microseconds",
        "microsecond",
        "nanoseconds",
        "nanosecond",
        "seconds",
        "second",
        "minutes",
        "minute",
        "hours",
        "hour",
        "msecs",
        "msec",
        "usecs",
        "usec",
        "nsecs",
        "nsec",
        "secs",
        "sec",
        "mins",
        "min",
        "hrs",
        "hr",
        "ms",
        "us",
        "ns",
        "s",
        "m",
        "h",
    ] {
        if smart_context_ascii_case_prefix(text, unit) {
            return Some(unit.len());
        }
    }
    None
}

fn smart_context_random_id_key_is_volatile(key: &str) -> bool {
    matches!(
        key,
        "request id"
            | "trace id"
            | "run id"
            | "span id"
            | "correlation id"
            | "invocation id"
            | "execution id"
            | "operation id"
            | "job id"
            | "build id"
            | "uuid"
            | "id"
    )
}

fn smart_context_random_id_value_len(text: &str) -> Option<usize> {
    let len = text
        .char_indices()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':')))
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    (len > 0).then_some(len)
}

fn smart_context_random_id_value_looks_volatile_for_key(key: &str, value: &str) -> bool {
    if smart_context_uuid_token_exact(value) {
        return true;
    }
    if key == "id" && value.len() < 20 {
        return false;
    }
    if value.len() < 12 {
        return false;
    }

    let mut alpha = false;
    let mut digit = false;
    let mut hex_like = true;
    let mut entropy_marks = 0usize;
    for ch in value.chars() {
        if ch.is_ascii_alphabetic() {
            alpha = true;
            if !ch.is_ascii_hexdigit() {
                hex_like = false;
            }
        } else if ch.is_ascii_digit() {
            digit = true;
        } else if matches!(ch, '_' | '-' | '.' | ':') {
            entropy_marks += 1;
        } else {
            return false;
        }
    }

    (hex_like && value.len() >= 16) || (alpha && digit && (value.len() >= 16 || entropy_marks > 0))
}

fn smart_context_uuid_token_exact(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if *byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

fn smart_context_parse_unsigned_ascii_int(text: &str, start: usize) -> Option<(usize, u64)> {
    let bytes = text.as_bytes();
    let mut index = start;
    let mut value = 0u64;
    let mut digits = 0usize;
    while let Some(byte) = bytes.get(index).copied() {
        if !byte.is_ascii_digit() {
            break;
        }
        value = value
            .saturating_mul(10)
            .saturating_add(u64::from(byte - b'0'));
        index += 1;
        digits += 1;
    }
    (digits > 0).then_some((index, value))
}

fn smart_context_skip_ascii_spaces(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = start;
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

fn smart_context_ascii_digits(bytes: &[u8], start: usize, len: usize) -> bool {
    bytes
        .get(start..start.saturating_add(len))
        .is_some_and(|value| value.iter().all(u8::is_ascii_digit))
}

fn smart_context_ascii_case_prefix(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|value| value.eq_ignore_ascii_case(prefix))
}

fn smart_context_token_boundary(ch: Option<char>) -> bool {
    ch.is_none_or(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/')))
}

fn smart_context_after_token_boundary(text: &str, index: usize) -> bool {
    text[index..]
        .chars()
        .next()
        .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/')))
}

fn smart_context_summary_prefix(text: &str, byte_limit: usize) -> String {
    let mut summary = String::new();
    for value in text.chars() {
        let next_len = summary.len() + value.len_utf8();
        if next_len > byte_limit {
            break;
        }
        summary.push(value);
    }
    summary
}

fn smart_context_artifact_marker_line(kind: &str, artifact: &SmartContextArtifactRef) -> String {
    let reference = smart_context_short_artifact_ref(&artifact.id);
    let kind = match kind {
        "artifact" => "art",
        other => other,
    };
    format!(
        "psc {kind} {reference} b={}; ref {reference}[#Lx-Ly]",
        artifact.byte_len
    )
}

#[allow(dead_code)]
fn smart_context_legacy_artifact_ref(id: &str) -> String {
    format!("prodex-artifact:{id}")
}

fn smart_context_short_artifact_label(id: &str) -> &str {
    id.strip_prefix("sc:").unwrap_or(id)
}

fn smart_context_capsule_order(
    left: &SmartContextMemoryCapsule,
    right: &SmartContextMemoryCapsule,
) -> Ordering {
    right
        .relevance
        .partial_cmp(&left.relevance)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.token_cost.cmp(&right.token_cost))
        .then_with(|| left.id.cmp(&right.id))
}

fn smart_context_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}

#[cfg(test)]
#[path = "../tests/src/smart_context.rs"]
mod tests;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::RuntimeTokenUsage;

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
            let content_hash = smart_context_hash_text(&item.text);
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
    pub observed_usage: Vec<RuntimeTokenUsage>,
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
        last_accounted_input_tokens = if usage.input_tokens == 0 {
            usage.cached_input_tokens
        } else {
            usage.input_tokens
        };
        last_observed_context_tokens =
            smart_context_observed_usage_context_tokens(*usage).unwrap_or(0);
    }

    let observed_uncached_input_tokens =
        observed_input_tokens.saturating_sub(observed_cached_input_tokens);
    let observed_total_tokens = observed_input_tokens.saturating_add(observed_output_tokens);
    let observed_context_tokens = observed_total_tokens.saturating_add(observed_reasoning_tokens);
    let estimated_current_request_tokens =
        smart_context_estimate_tokens_from_body_bytes(input.current_request_body_bytes);
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
    PlentyOfBudget,
    ModerateBudget,
    TightBudget,
    CriticalBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextAdaptiveBudgetPolicyInput {
    pub exactness_guard: SmartContextExactnessGuard,
    pub accounting: SmartContextObservedTokenAccounting,
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
            max_rehydrate_tokens: input
                .accounting
                .available_context_tokens
                .unwrap_or(u64::MAX),
            reasons,
        };
    }

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
            32 * 1024,
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
            2 * 1024,
            1_000,
            SmartContextBudgetPolicyReason::CriticalBudget,
        ),
    };
    reasons.push(tier_reason);
    let max_rehydrate_tokens = input
        .accounting
        .available_context_tokens
        .map(|available| max_rehydrate_tokens.min(available))
        .unwrap_or(max_rehydrate_tokens);

    SmartContextAdaptiveBudgetPolicy {
        tier,
        mode,
        max_inline_bytes: max_inline_tool_output_bytes,
        max_inline_tool_output_bytes,
        max_rehydrate_tokens,
        reasons,
    }
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

fn smart_context_stabilize_static_context_id(id: &str) -> String {
    id.trim().replace('\\', "/")
}

fn smart_context_static_context_item_order(
    left: &SmartContextStableStaticContextItem,
    right: &SmartContextStableStaticContextItem,
) -> Ordering {
    left.id
        .cmp(&right.id)
        .then_with(|| left.content_hash.cmp(&right.content_hash))
        .then_with(|| left.byte_len.cmp(&right.byte_len))
        .then_with(|| left.canonical_text.cmp(&right.canonical_text))
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

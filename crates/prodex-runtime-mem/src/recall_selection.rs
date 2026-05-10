use crate::artifact_refs::{
    runtime_mem_artifact_recall_summary, runtime_mem_content_hash,
    runtime_mem_duplicate_recall_summary, runtime_mem_prodex_artifact_ref,
};
mod intent;
pub(crate) use intent::runtime_mem_prompt_term_is_path;
use intent::{
    RuntimeMemPreparedRecallIntent, RuntimeMemRecallIntentScore, runtime_mem_capsule_intent_score,
    runtime_mem_normalized_path, runtime_mem_recall_diet_capsule_order,
};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const RUNTIME_MEM_RECALL_PROMPT_SCAN_CHAR_LIMIT: usize = 4096;
const RUNTIME_MEM_RECALL_PROMPT_MAX_TERMS: usize = 32;
pub const RUNTIME_MEM_DEFAULT_RECENT_WINDOW_SECONDS: u64 = 7 * 24 * 60 * 60;
pub const RUNTIME_MEM_DEFAULT_CAPSULE_MINIMAL_TOKEN_BUDGET: usize = 128;
pub const RUNTIME_MEM_DEFAULT_CAPSULE_CONDENSED_TOKEN_BUDGET: usize = 512;
pub const RUNTIME_MEM_DEFAULT_CAPSULE_LARGE_TOKEN_BUDGET: usize = 2_048;
pub const RUNTIME_MEM_SUPER_CAPSULE_MINIMAL_TOKEN_BUDGET: usize = 256;
pub const RUNTIME_MEM_SUPER_CAPSULE_CONDENSED_TOKEN_BUDGET: usize = 1_024;
pub const RUNTIME_MEM_SUPER_CAPSULE_LARGE_TOKEN_BUDGET: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemCapsulePriority {
    Required,
    ProjectLocal,
    Recent,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemCapsuleBudgetTier {
    Exact,
    Large,
    Condensed,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemCapsuleBudgetMode {
    Default,
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemCapsuleBudget {
    Explicit(usize),
    Tier {
        available_tokens: usize,
        mode: RuntimeMemCapsuleBudgetMode,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeMemCapsuleMetadata {
    pub id: String,
    pub token_cost: usize,
    pub required: bool,
    pub project_path: Option<PathBuf>,
    pub updated_at_seconds: Option<i64>,
    pub relevance: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeMemRecallIntent {
    pub prompt: Option<String>,
    pub paths: Vec<PathBuf>,
    pub symbols: Vec<String>,
}

impl RuntimeMemRecallIntent {
    pub fn from_prompt(prompt: impl Into<String>) -> Self {
        Self {
            prompt: Some(prompt.into()),
            paths: Vec::new(),
            symbols: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
            && self.symbols.is_empty()
            && self
                .prompt
                .as_deref()
                .is_none_or(|prompt| prompt.trim().is_empty())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeMemRecallCapsuleMetadata {
    pub capsule: RuntimeMemCapsuleMetadata,
    pub paths: Vec<PathBuf>,
    pub symbols: Vec<String>,
}

impl RuntimeMemRecallCapsuleMetadata {
    pub fn new(capsule: RuntimeMemCapsuleMetadata) -> Self {
        Self {
            capsule,
            paths: Vec::new(),
            symbols: Vec::new(),
        }
    }
}

impl From<RuntimeMemCapsuleMetadata> for RuntimeMemRecallCapsuleMetadata {
    fn from(capsule: RuntimeMemCapsuleMetadata) -> Self {
        Self::new(capsule)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemRecallDedupeItem {
    pub id: String,
    pub content: String,
    pub required: bool,
    pub artifact_ref: Option<String>,
}

impl RuntimeMemRecallDedupeItem {
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            required: false,
            artifact_ref: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeMemRecallDedupeReason {
    Duplicate { original_id: String },
    ArtifactRef { artifact_ref: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemRecallDedupeEntry {
    pub id: String,
    pub content: String,
    pub content_hash: String,
    pub required: bool,
    pub replacement: Option<String>,
    pub reason: Option<RuntimeMemRecallDedupeReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemCapsuleSelectionContext {
    pub token_budget: usize,
    pub project_root: Option<PathBuf>,
    pub now_seconds: Option<i64>,
    pub recent_window_seconds: u64,
}

impl RuntimeMemCapsuleSelectionContext {
    pub fn new(token_budget: usize) -> Self {
        Self {
            token_budget,
            project_root: None,
            now_seconds: None,
            recent_window_seconds: RUNTIME_MEM_DEFAULT_RECENT_WINDOW_SECONDS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemAutoCapsuleSelectionContext {
    pub budget: RuntimeMemCapsuleBudget,
    pub project_root: Option<PathBuf>,
    pub now_seconds: Option<i64>,
    pub recent_window_seconds: u64,
}

impl RuntimeMemAutoCapsuleSelectionContext {
    pub fn new(budget: RuntimeMemCapsuleBudget) -> Self {
        Self {
            budget,
            project_root: None,
            now_seconds: None,
            recent_window_seconds: RUNTIME_MEM_DEFAULT_RECENT_WINDOW_SECONDS,
        }
    }

    pub fn super_mode(available_tokens: usize) -> Self {
        Self::new(RuntimeMemCapsuleBudget::Tier {
            available_tokens,
            mode: RuntimeMemCapsuleBudgetMode::Super,
        })
    }

    pub fn to_selection_context(&self) -> RuntimeMemCapsuleSelectionContext {
        RuntimeMemCapsuleSelectionContext {
            token_budget: runtime_mem_capsule_token_budget(self.budget),
            project_root: self.project_root.clone(),
            now_seconds: self.now_seconds,
            recent_window_seconds: self.recent_window_seconds,
        }
    }
}

impl Default for RuntimeMemCapsuleSelectionContext {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemCapsuleSelectionEntry {
    pub id: String,
    pub priority: RuntimeMemCapsulePriority,
    pub token_cost: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMemCapsuleSelection {
    pub selected: Vec<RuntimeMemCapsuleSelectionEntry>,
    pub omitted: Vec<RuntimeMemCapsuleSelectionEntry>,
    pub used_tokens: usize,
    pub token_budget: usize,
}

pub fn runtime_mem_classify_capsule(
    capsule: &RuntimeMemCapsuleMetadata,
    context: &RuntimeMemCapsuleSelectionContext,
) -> RuntimeMemCapsulePriority {
    if capsule.required {
        return RuntimeMemCapsulePriority::Required;
    }
    if runtime_mem_capsule_is_project_local(capsule, context) {
        return RuntimeMemCapsulePriority::ProjectLocal;
    }
    if runtime_mem_capsule_is_recent(capsule, context) {
        return RuntimeMemCapsulePriority::Recent;
    }
    RuntimeMemCapsulePriority::Optional
}

pub fn runtime_mem_capsule_budget_tier(available_tokens: usize) -> RuntimeMemCapsuleBudgetTier {
    match available_tokens {
        16_000.. => RuntimeMemCapsuleBudgetTier::Exact,
        8_000..=15_999 => RuntimeMemCapsuleBudgetTier::Large,
        2_000..=7_999 => RuntimeMemCapsuleBudgetTier::Condensed,
        _ => RuntimeMemCapsuleBudgetTier::Minimal,
    }
}

pub fn runtime_mem_capsule_token_budget(budget: RuntimeMemCapsuleBudget) -> usize {
    match budget {
        RuntimeMemCapsuleBudget::Explicit(token_budget) => token_budget,
        RuntimeMemCapsuleBudget::Tier {
            available_tokens,
            mode,
        } => runtime_mem_capsule_token_budget_for_tier(
            mode,
            runtime_mem_capsule_budget_tier(available_tokens),
        ),
    }
}

pub fn runtime_mem_capsule_token_budget_for_tier(
    mode: RuntimeMemCapsuleBudgetMode,
    tier: RuntimeMemCapsuleBudgetTier,
) -> usize {
    match mode {
        RuntimeMemCapsuleBudgetMode::Default => match tier {
            RuntimeMemCapsuleBudgetTier::Minimal => {
                RUNTIME_MEM_DEFAULT_CAPSULE_MINIMAL_TOKEN_BUDGET
            }
            RuntimeMemCapsuleBudgetTier::Condensed => {
                RUNTIME_MEM_DEFAULT_CAPSULE_CONDENSED_TOKEN_BUDGET
            }
            RuntimeMemCapsuleBudgetTier::Large | RuntimeMemCapsuleBudgetTier::Exact => {
                RUNTIME_MEM_DEFAULT_CAPSULE_LARGE_TOKEN_BUDGET
            }
        },
        RuntimeMemCapsuleBudgetMode::Super => match tier {
            RuntimeMemCapsuleBudgetTier::Minimal => RUNTIME_MEM_SUPER_CAPSULE_MINIMAL_TOKEN_BUDGET,
            RuntimeMemCapsuleBudgetTier::Condensed => {
                RUNTIME_MEM_SUPER_CAPSULE_CONDENSED_TOKEN_BUDGET
            }
            RuntimeMemCapsuleBudgetTier::Large | RuntimeMemCapsuleBudgetTier::Exact => {
                RUNTIME_MEM_SUPER_CAPSULE_LARGE_TOKEN_BUDGET
            }
        },
    }
}

pub fn runtime_mem_select_capsules_auto(
    capsules: impl IntoIterator<Item = RuntimeMemCapsuleMetadata>,
    context: RuntimeMemAutoCapsuleSelectionContext,
) -> RuntimeMemCapsuleSelection {
    runtime_mem_select_capsules(capsules, context.to_selection_context())
}

pub fn runtime_mem_select_capsules_for_recall_diet(
    capsules: impl IntoIterator<Item = RuntimeMemRecallCapsuleMetadata>,
    context: RuntimeMemAutoCapsuleSelectionContext,
    intent: RuntimeMemRecallIntent,
) -> RuntimeMemCapsuleSelection {
    runtime_mem_select_capsules_with_recall_intent(capsules, context.to_selection_context(), intent)
}

pub fn runtime_mem_select_capsules_with_recall_intent(
    capsules: impl IntoIterator<Item = RuntimeMemRecallCapsuleMetadata>,
    context: RuntimeMemCapsuleSelectionContext,
    intent: RuntimeMemRecallIntent,
) -> RuntimeMemCapsuleSelection {
    let prepared_intent = RuntimeMemPreparedRecallIntent::from_intent(&intent);
    if prepared_intent.is_empty() {
        return runtime_mem_select_capsules(
            capsules.into_iter().map(|candidate| candidate.capsule),
            context,
        );
    }

    let mut candidates = capsules
        .into_iter()
        .map(|capsule| {
            let priority = runtime_mem_classify_capsule(&capsule.capsule, &context);
            let intent_score =
                runtime_mem_capsule_intent_score(&capsule, &context, &prepared_intent);
            (capsule, priority, intent_score)
        })
        .collect::<Vec<_>>();
    candidates = runtime_mem_dedupe_recall_diet_candidates(candidates);
    candidates.sort_by(runtime_mem_recall_diet_capsule_order);
    let has_intent_matches = candidates
        .iter()
        .any(|(_, _, intent_score)| intent_score.is_match());

    let mut selected = Vec::new();
    let mut omitted = Vec::new();
    let mut used_tokens = 0usize;

    for (candidate, priority, intent_score) in candidates {
        let entry = RuntimeMemCapsuleSelectionEntry {
            id: candidate.capsule.id,
            priority,
            token_cost: candidate.capsule.token_cost,
        };
        if has_intent_matches
            && !intent_score.is_match()
            && matches!(priority, RuntimeMemCapsulePriority::Optional)
        {
            omitted.push(entry);
            continue;
        }
        if used_tokens.saturating_add(entry.token_cost) <= context.token_budget {
            used_tokens += entry.token_cost;
            selected.push(entry);
        } else {
            omitted.push(entry);
        }
    }

    RuntimeMemCapsuleSelection {
        selected,
        omitted,
        used_tokens,
        token_budget: context.token_budget,
    }
}

pub fn runtime_mem_select_capsules(
    capsules: impl IntoIterator<Item = RuntimeMemCapsuleMetadata>,
    context: RuntimeMemCapsuleSelectionContext,
) -> RuntimeMemCapsuleSelection {
    let mut candidates = capsules
        .into_iter()
        .map(|capsule| {
            let priority = runtime_mem_classify_capsule(&capsule, &context);
            (capsule, priority)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(runtime_mem_capsule_order);

    let mut selected = Vec::new();
    let mut omitted = Vec::new();
    let mut used_tokens = 0usize;

    for (capsule, priority) in candidates {
        let entry = RuntimeMemCapsuleSelectionEntry {
            id: capsule.id,
            priority,
            token_cost: capsule.token_cost,
        };
        if used_tokens.saturating_add(entry.token_cost) <= context.token_budget {
            used_tokens += entry.token_cost;
            selected.push(entry);
        } else {
            omitted.push(entry);
        }
    }

    RuntimeMemCapsuleSelection {
        selected,
        omitted,
        used_tokens,
        token_budget: context.token_budget,
    }
}

pub fn runtime_mem_dedupe_recall_content(
    items: impl IntoIterator<Item = RuntimeMemRecallDedupeItem>,
) -> Vec<RuntimeMemRecallDedupeEntry> {
    let items = items.into_iter().collect::<Vec<_>>();
    let mut first_by_content = HashMap::<String, String>::new();
    let mut first_required_by_content = HashMap::<String, String>::new();

    for item in &items {
        first_by_content
            .entry(item.content.clone())
            .or_insert_with(|| item.id.clone());
        if item.required {
            first_required_by_content
                .entry(item.content.clone())
                .or_insert_with(|| item.id.clone());
        }
    }

    items
        .into_iter()
        .map(|item| {
            let content_hash = runtime_mem_content_hash(&item.content);
            if item.required {
                return RuntimeMemRecallDedupeEntry {
                    id: item.id,
                    content: item.content,
                    content_hash,
                    required: true,
                    replacement: None,
                    reason: None,
                };
            }

            let artifact_ref =
                runtime_mem_prodex_artifact_ref(item.artifact_ref.as_deref(), &item.content);
            if let Some(artifact_ref) = artifact_ref {
                let original_bytes = item.content.len();
                return RuntimeMemRecallDedupeEntry {
                    id: item.id,
                    content: item.content,
                    replacement: Some(runtime_mem_artifact_recall_summary(
                        &artifact_ref,
                        &content_hash,
                        original_bytes,
                    )),
                    reason: Some(RuntimeMemRecallDedupeReason::ArtifactRef { artifact_ref }),
                    content_hash,
                    required: false,
                };
            }

            let original_id = first_required_by_content
                .get(&item.content)
                .or_else(|| first_by_content.get(&item.content))
                .cloned();
            if let Some(original_id) = original_id
                && original_id != item.id
            {
                let original_bytes = item.content.len();
                return RuntimeMemRecallDedupeEntry {
                    id: item.id,
                    content: item.content,
                    replacement: Some(runtime_mem_duplicate_recall_summary(
                        &original_id,
                        &content_hash,
                        original_bytes,
                    )),
                    reason: Some(RuntimeMemRecallDedupeReason::Duplicate { original_id }),
                    content_hash,
                    required: false,
                };
            }

            RuntimeMemRecallDedupeEntry {
                id: item.id,
                content: item.content,
                content_hash,
                required: false,
                replacement: None,
                reason: None,
            }
        })
        .collect()
}

fn runtime_mem_capsule_is_project_local(
    capsule: &RuntimeMemCapsuleMetadata,
    context: &RuntimeMemCapsuleSelectionContext,
) -> bool {
    let (Some(project_root), Some(capsule_path)) = (&context.project_root, &capsule.project_path)
    else {
        return false;
    };
    let project_root = runtime_mem_normalized_path(project_root);
    let capsule_path = runtime_mem_normalized_path(capsule_path);
    capsule_path == project_root || capsule_path.starts_with(project_root)
}

fn runtime_mem_capsule_is_recent(
    capsule: &RuntimeMemCapsuleMetadata,
    context: &RuntimeMemCapsuleSelectionContext,
) -> bool {
    let (Some(updated_at), Some(now)) = (capsule.updated_at_seconds, context.now_seconds) else {
        return false;
    };
    if updated_at >= now {
        return true;
    }
    now.checked_sub(updated_at)
        .is_some_and(|age| (age as u64) <= context.recent_window_seconds)
}

fn runtime_mem_capsule_order(
    left: &(RuntimeMemCapsuleMetadata, RuntimeMemCapsulePriority),
    right: &(RuntimeMemCapsuleMetadata, RuntimeMemCapsulePriority),
) -> Ordering {
    runtime_mem_capsule_priority_rank(left.1)
        .cmp(&runtime_mem_capsule_priority_rank(right.1))
        .then_with(|| runtime_mem_relevance_order(right.0.relevance, left.0.relevance))
        .then_with(|| {
            right
                .0
                .updated_at_seconds
                .unwrap_or(i64::MIN)
                .cmp(&left.0.updated_at_seconds.unwrap_or(i64::MIN))
        })
        .then_with(|| left.0.token_cost.cmp(&right.0.token_cost))
        .then_with(|| left.0.id.cmp(&right.0.id))
}

fn runtime_mem_dedupe_recall_diet_candidates(
    candidates: Vec<(
        RuntimeMemRecallCapsuleMetadata,
        RuntimeMemCapsulePriority,
        RuntimeMemRecallIntentScore,
    )>,
) -> Vec<(
    RuntimeMemRecallCapsuleMetadata,
    RuntimeMemCapsulePriority,
    RuntimeMemRecallIntentScore,
)> {
    let mut deduped: Vec<(
        RuntimeMemRecallCapsuleMetadata,
        RuntimeMemCapsulePriority,
        RuntimeMemRecallIntentScore,
    )> = Vec::new();
    for candidate in candidates {
        let Some(existing) = deduped
            .iter_mut()
            .find(|existing| existing.0.capsule.id == candidate.0.capsule.id)
        else {
            deduped.push(candidate);
            continue;
        };
        if runtime_mem_recall_diet_capsule_order(&candidate, existing).is_lt() {
            *existing = candidate;
        }
    }
    deduped
}

fn runtime_mem_capsule_priority_rank(priority: RuntimeMemCapsulePriority) -> u8 {
    match priority {
        RuntimeMemCapsulePriority::Required => 0,
        RuntimeMemCapsulePriority::ProjectLocal => 1,
        RuntimeMemCapsulePriority::Recent => 2,
        RuntimeMemCapsulePriority::Optional => 3,
    }
}

fn runtime_mem_relevance_order(left: f32, right: f32) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

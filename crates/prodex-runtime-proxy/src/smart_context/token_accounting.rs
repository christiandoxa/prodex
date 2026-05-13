mod calibration;
mod estimation;

pub(super) use calibration::*;
pub use estimation::{
    SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN, smart_context_estimate_tokens_from_body,
    smart_context_estimate_tokens_from_body_bytes,
};

use super::*;
use crate::RuntimeTokenUsage;
use std::collections::BTreeSet;

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

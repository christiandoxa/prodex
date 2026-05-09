use super::*;
use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextTransformCategory {
    StructuralMinifyJson,
    StaticContextPromptCache,
    CommandOutputCache,
    ToolOutputArtifact,
    CrossTurnDuplicateRef,
    AutoRehydrate,
    MemoryCapsuleSelection,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTransformRewriteTelemetrySample {
    pub category: SmartContextTransformCategory,
    pub sample: SmartContextRewriteTelemetrySample,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTransformRecentRewriteSafety {
    pub category: SmartContextTransformCategory,
    pub safety: SmartContextRecentRewriteSafety,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextTransformRewriteSafetyReason {
    NoTransformSamples,
    InsufficientTransformSamples,
    RecentSafetyFallback,
    FallbackObserved,
    UnsafeSample,
    WeakSavings,
    ModerateSavings,
    StableSavings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTransformRewriteSafetyScoreInput {
    pub category: SmartContextTransformCategory,
    pub recent_rewrite_safety: SmartContextRecentRewriteSafety,
    pub telemetry_samples: Vec<SmartContextTransformRewriteTelemetrySample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextPerTransformRewriteSafetyInput {
    pub recent_rewrite_safety: Vec<SmartContextTransformRecentRewriteSafety>,
    pub telemetry_samples: Vec<SmartContextTransformRewriteTelemetrySample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextTransformRewriteSafetyScore {
    pub category: SmartContextTransformCategory,
    pub decision: SmartContextRewriteBudgetDecision,
    pub safety_score: i32,
    pub telemetry_samples: usize,
    pub safety_samples: usize,
    pub safe_samples: usize,
    pub fallback_samples: usize,
    pub unsafe_samples: usize,
    pub weak_savings_samples: usize,
    pub saved_tokens: u64,
    pub average_body_ratio_percent: Option<usize>,
    pub reasons: Vec<SmartContextTransformRewriteSafetyReason>,
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

    let saved_tokens = smart_context_rewrite_telemetry_saved_tokens(&recent);
    let average_body_ratio_percent =
        smart_context_rewrite_telemetry_average_body_ratio_percent(&recent);
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

pub fn smart_context_transform_rewrite_budget_decision(
    input: SmartContextTransformRewriteSafetyScoreInput,
) -> SmartContextRewriteBudgetDecision {
    smart_context_transform_rewrite_safety_score(input).decision
}

pub fn smart_context_transform_rewrite_safety_score(
    input: SmartContextTransformRewriteSafetyScoreInput,
) -> SmartContextTransformRewriteSafetyScore {
    let SmartContextTransformRewriteSafetyScoreInput {
        category,
        recent_rewrite_safety,
        telemetry_samples,
    } = input;
    let recent = telemetry_samples
        .iter()
        .filter(|sample| sample.category == category)
        .map(|sample| sample.sample)
        .rev()
        .take(SMART_CONTEXT_REWRITE_TELEMETRY_RECENT_LIMIT)
        .collect::<Vec<_>>();
    let safety_samples = recent_rewrite_safety
        .safe_rewrites
        .saturating_add(recent_rewrite_safety.fallback_rewrites);

    if recent.is_empty() {
        let mut score =
            smart_context_transform_rewrite_safety_score_empty(category, &recent_rewrite_safety);
        score.safety_samples = safety_samples;
        return score;
    }

    let safe_samples = recent
        .iter()
        .filter(|sample| smart_context_rewrite_telemetry_sample_safe_saved(sample))
        .count();
    let fallback_samples = recent.iter().filter(|sample| sample.fallback).count();
    let unsafe_samples = recent.iter().filter(|sample| !sample.safe).count();
    let weak_savings_samples = recent
        .iter()
        .filter(|sample| {
            !sample.fallback
                && sample.safe
                && !smart_context_rewrite_telemetry_sample_safe_saved(sample)
        })
        .count();
    let saved_tokens = smart_context_rewrite_telemetry_saved_tokens(&recent);
    let average_body_ratio_percent_value =
        smart_context_rewrite_telemetry_average_body_ratio_percent(&recent);
    let average_body_ratio_percent = Some(average_body_ratio_percent_value);
    let mut reasons = Vec::new();

    if fallback_samples > 0 || unsafe_samples > 0 || weak_savings_samples > 0 {
        if fallback_samples > 0 {
            reasons.push(SmartContextTransformRewriteSafetyReason::FallbackObserved);
        }
        if unsafe_samples > 0 {
            reasons.push(SmartContextTransformRewriteSafetyReason::UnsafeSample);
        }
        if weak_savings_samples > 0 {
            reasons.push(SmartContextTransformRewriteSafetyReason::WeakSavings);
        }
        return SmartContextTransformRewriteSafetyScore {
            category,
            decision: SmartContextRewriteBudgetDecision::Tighten,
            safety_score: -100,
            telemetry_samples: recent.len(),
            safety_samples,
            safe_samples,
            fallback_samples,
            unsafe_samples,
            weak_savings_samples,
            saved_tokens,
            average_body_ratio_percent,
            reasons,
        };
    }

    if recent.len() < SMART_CONTEXT_REWRITE_TELEMETRY_MIN_SAMPLE_COUNT {
        let decision = smart_context_recent_rewrite_safety_budget_decision(&recent_rewrite_safety);
        reasons.push(SmartContextTransformRewriteSafetyReason::InsufficientTransformSamples);
        if safety_samples > 0 {
            reasons.push(SmartContextTransformRewriteSafetyReason::RecentSafetyFallback);
            reasons.extend(smart_context_transform_rewrite_safety_reasons_for_decision(
                decision,
            ));
        }
        return SmartContextTransformRewriteSafetyScore {
            category,
            decision,
            safety_score: smart_context_transform_rewrite_safety_score_value(decision),
            telemetry_samples: recent.len(),
            safety_samples,
            safe_samples,
            fallback_samples,
            unsafe_samples,
            weak_savings_samples,
            saved_tokens,
            average_body_ratio_percent,
            reasons,
        };
    }

    let required_saved_tokens = smart_context_recent_rewrite_min_saved_tokens(recent.len());
    let decision = if saved_tokens >= required_saved_tokens
        && average_body_ratio_percent_value
            <= SMART_CONTEXT_REWRITE_TELEMETRY_RELAX_MAX_AVERAGE_BODY_RATIO_PERCENT
    {
        SmartContextRewriteBudgetDecision::Relax
    } else if saved_tokens < required_saved_tokens
        || average_body_ratio_percent_value
            >= SMART_CONTEXT_REWRITE_TELEMETRY_TIGHTEN_MIN_AVERAGE_BODY_RATIO_PERCENT
    {
        SmartContextRewriteBudgetDecision::Tighten
    } else {
        SmartContextRewriteBudgetDecision::NoChange
    };
    reasons.extend(smart_context_transform_rewrite_safety_reasons_for_decision(
        decision,
    ));

    SmartContextTransformRewriteSafetyScore {
        category,
        decision,
        safety_score: smart_context_transform_rewrite_safety_score_value(decision),
        telemetry_samples: recent.len(),
        safety_samples,
        safe_samples,
        fallback_samples,
        unsafe_samples,
        weak_savings_samples,
        saved_tokens,
        average_body_ratio_percent,
        reasons,
    }
}

pub fn smart_context_per_transform_rewrite_safety_scores(
    input: SmartContextPerTransformRewriteSafetyInput,
) -> Vec<SmartContextTransformRewriteSafetyScore> {
    let mut categories = BTreeSet::new();
    let mut recent_safety =
        BTreeMap::<SmartContextTransformCategory, SmartContextRecentRewriteSafety>::new();

    for item in input.recent_rewrite_safety {
        categories.insert(item.category.clone());
        recent_safety
            .entry(item.category)
            .and_modify(|existing| {
                existing.safe_rewrites = existing
                    .safe_rewrites
                    .saturating_add(item.safety.safe_rewrites);
                existing.fallback_rewrites = existing
                    .fallback_rewrites
                    .saturating_add(item.safety.fallback_rewrites);
                existing.saved_tokens = existing
                    .saved_tokens
                    .saturating_add(item.safety.saved_tokens);
            })
            .or_insert(item.safety);
    }
    for sample in &input.telemetry_samples {
        categories.insert(sample.category.clone());
    }

    categories
        .into_iter()
        .map(|category| {
            smart_context_transform_rewrite_safety_score(
                SmartContextTransformRewriteSafetyScoreInput {
                    recent_rewrite_safety: recent_safety
                        .get(&category)
                        .copied()
                        .unwrap_or_default(),
                    telemetry_samples: input.telemetry_samples.clone(),
                    category,
                },
            )
        })
        .collect()
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

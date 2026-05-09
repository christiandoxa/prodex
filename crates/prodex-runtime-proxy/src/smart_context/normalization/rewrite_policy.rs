use super::*;

pub(in crate::smart_context) fn smart_context_recent_rewrite_min_saved_tokens(
    rewrite_count: usize,
) -> u64 {
    SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS
        .saturating_mul(u64::try_from(rewrite_count).unwrap_or(u64::MAX))
}

pub(in crate::smart_context) fn smart_context_learned_rewrite_policy_exact(
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

pub(in crate::smart_context) fn smart_context_rewrite_policy_bucket_key_complete(
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

pub(in crate::smart_context) fn smart_context_rewrite_policy_bucket_key_matches(
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

pub(in crate::smart_context) fn smart_context_rewrite_policy_field_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    target.as_deref().is_some_and(non_empty) && target == sample
}

pub(in crate::smart_context) fn smart_context_rewrite_policy_model_matches(
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

pub(in crate::smart_context) fn smart_context_rewrite_telemetry_sample_safe_saved(
    sample: &SmartContextRewriteTelemetrySample,
) -> bool {
    sample.safe
        && sample.estimated_tokens_after < sample.estimated_tokens_before
        && sample.body_bytes_after < sample.body_bytes_before
}

pub(in crate::smart_context) fn smart_context_rewrite_telemetry_saved_tokens(
    samples: &[SmartContextRewriteTelemetrySample],
) -> u64 {
    samples.iter().fold(0u64, |total, sample| {
        total.saturating_add(
            sample
                .estimated_tokens_before
                .saturating_sub(sample.estimated_tokens_after),
        )
    })
}

pub(in crate::smart_context) fn smart_context_rewrite_telemetry_average_body_ratio_percent(
    samples: &[SmartContextRewriteTelemetrySample],
) -> usize {
    if samples.is_empty() {
        return 100;
    }
    samples.iter().fold(0usize, |total, sample| {
        total.saturating_add(smart_context_rewrite_body_ratio_percent(
            sample.body_bytes_before,
            sample.body_bytes_after,
        ))
    }) / samples.len()
}

pub(in crate::smart_context) fn smart_context_transform_rewrite_safety_score_empty(
    category: SmartContextTransformCategory,
    safety: &SmartContextRecentRewriteSafety,
) -> SmartContextTransformRewriteSafetyScore {
    let safety_samples = safety
        .safe_rewrites
        .saturating_add(safety.fallback_rewrites);
    let decision = smart_context_recent_rewrite_safety_budget_decision(safety);
    let mut reasons = vec![SmartContextTransformRewriteSafetyReason::NoTransformSamples];
    if safety_samples > 0 {
        reasons.push(SmartContextTransformRewriteSafetyReason::RecentSafetyFallback);
        reasons.extend(smart_context_transform_rewrite_safety_reasons_for_decision(
            decision,
        ));
    }

    SmartContextTransformRewriteSafetyScore {
        category,
        decision,
        safety_score: smart_context_transform_rewrite_safety_score_value(decision),
        telemetry_samples: 0,
        safety_samples,
        safe_samples: 0,
        fallback_samples: 0,
        unsafe_samples: 0,
        weak_savings_samples: 0,
        saved_tokens: 0,
        average_body_ratio_percent: None,
        reasons,
    }
}

pub(in crate::smart_context) fn smart_context_transform_rewrite_safety_reasons_for_decision(
    decision: SmartContextRewriteBudgetDecision,
) -> Vec<SmartContextTransformRewriteSafetyReason> {
    match decision {
        SmartContextRewriteBudgetDecision::NoChange => {
            vec![SmartContextTransformRewriteSafetyReason::ModerateSavings]
        }
        SmartContextRewriteBudgetDecision::Relax => {
            vec![SmartContextTransformRewriteSafetyReason::StableSavings]
        }
        SmartContextRewriteBudgetDecision::Tighten => {
            vec![SmartContextTransformRewriteSafetyReason::WeakSavings]
        }
    }
}

pub(in crate::smart_context) fn smart_context_transform_rewrite_safety_score_value(
    decision: SmartContextRewriteBudgetDecision,
) -> i32 {
    match decision {
        SmartContextRewriteBudgetDecision::NoChange => 0,
        SmartContextRewriteBudgetDecision::Relax => 100,
        SmartContextRewriteBudgetDecision::Tighten => -100,
    }
}

pub(in crate::smart_context) fn smart_context_rewrite_body_ratio_percent(
    body_bytes_before: usize,
    body_bytes_after: usize,
) -> usize {
    if body_bytes_before == 0 {
        return 100;
    }
    body_bytes_after.saturating_mul(100) / body_bytes_before
}

pub(in crate::smart_context) fn smart_context_relaxed_inline_budget(
    tier: SmartContextTokenBudgetTier,
    value: usize,
) -> usize {
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

pub(in crate::smart_context) fn smart_context_relaxed_rehydrate_budget(value: u64) -> u64 {
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

pub(in crate::smart_context) fn smart_context_tightened_inline_budget(value: usize) -> usize {
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

pub(in crate::smart_context) fn smart_context_tightened_rehydrate_budget(value: u64) -> u64 {
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

pub(in crate::smart_context) fn smart_context_scale_usize_ceil(
    value: usize,
    numerator: u64,
    denominator: u64,
) -> usize {
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    smart_context_u64_saturating_usize(smart_context_scale_u64_ceil(value, numerator, denominator))
}

pub(in crate::smart_context) fn smart_context_scale_usize_floor(
    value: usize,
    numerator: u64,
    denominator: u64,
) -> usize {
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    smart_context_u64_saturating_usize(smart_context_scale_u64_floor(value, numerator, denominator))
}

pub(in crate::smart_context) fn smart_context_scale_u64_ceil(
    value: u64,
    numerator: u64,
    denominator: u64,
) -> u64 {
    if denominator == 0 {
        return value;
    }
    value
        .saturating_mul(numerator)
        .saturating_add(denominator - 1)
        / denominator
}

pub(in crate::smart_context) fn smart_context_scale_u64_floor(
    value: u64,
    numerator: u64,
    denominator: u64,
) -> u64 {
    if denominator == 0 {
        return value;
    }
    value.saturating_mul(numerator) / denominator
}

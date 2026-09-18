use super::*;

pub(in crate::smart_context) fn smart_context_recent_rewrite_min_saved_tokens(
    rewrite_count: usize,
) -> u64 {
    SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS
        .saturating_mul(u64::try_from(rewrite_count).unwrap_or(u64::MAX))
}

#[cfg(not(feature = "mojo"))]
pub(in crate::smart_context) fn smart_context_rewrite_telemetry_sample_safe_saved(
    sample: &SmartContextRewriteTelemetrySample,
) -> bool {
    sample.safe
        && sample.token_count_source == SmartContextTokenCountSource::TokenizerCounted
        && !smart_context_rewrite_telemetry_sample_quality_risk(sample)
        && sample.tokens_after < sample.tokens_before
        && sample.body_bytes_after < sample.body_bytes_before
}

pub(in crate::smart_context) fn smart_context_rewrite_telemetry_sample_quality_risk(
    sample: &SmartContextRewriteTelemetrySample,
) -> bool {
    sample.upstream_context_errors > 0
        || sample.previous_response_not_found
        || sample.invalid_tool_call_continuation
        || sample.missing_artifact_requests > 0
        || sample.repeated_tool_call_count > 0
        || sample.model_reread_requests > 0
        || sample.corrective_user_messages > 0
        || sample.test_or_build_failed_after_rewrite
        || sample.task_completed == Some(false)
}

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
pub(in crate::smart_context) fn smart_context_scale_usize_ceil(
    value: usize,
    numerator: u64,
    denominator: u64,
) -> usize {
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    smart_context_u64_saturating_usize(smart_context_scale_u64_ceil(value, numerator, denominator))
}

#[cfg(not(feature = "mojo"))]
pub(in crate::smart_context) fn smart_context_scale_usize_floor(
    value: usize,
    numerator: u64,
    denominator: u64,
) -> usize {
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    smart_context_u64_saturating_usize(smart_context_scale_u64_floor(value, numerator, denominator))
}

#[cfg(not(feature = "mojo"))]
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

#[cfg(not(feature = "mojo"))]
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

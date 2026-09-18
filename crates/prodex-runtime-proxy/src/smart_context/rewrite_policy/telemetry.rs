use super::*;
use crate::smart_context::smart_context_recent_rewrite_min_saved_tokens;
#[cfg(not(feature = "mojo"))]
use crate::smart_context::smart_context_rewrite_telemetry_sample_safe_saved;
#[cfg(feature = "mojo")]
use crate::smart_context::{
    SmartContextTokenCountSource, smart_context_rewrite_telemetry_sample_quality_risk,
};

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
    #[cfg(feature = "mojo")]
    {
        let recent = input
            .telemetry_samples
            .iter()
            .rev()
            .take(SMART_CONTEXT_REWRITE_TELEMETRY_RECENT_LIMIT)
            .map(
                |sample| prodex_mojo_core::runtime::SmartContextRewriteTelemetryInput {
                    body_bytes_before: u64::try_from(sample.body_bytes_before)
                        .expect("Smart Context body length fits u64"),
                    body_bytes_after: u64::try_from(sample.body_bytes_after)
                        .expect("Smart Context body length fits u64"),
                    tokens_before: sample.tokens_before,
                    tokens_after: sample.tokens_after,
                    token_count_source: i64::from(
                        sample.token_count_source == SmartContextTokenCountSource::TokenizerCounted,
                    ),
                    safe: sample.safe,
                    fallback: sample.fallback,
                    quality_risk: smart_context_rewrite_telemetry_sample_quality_risk(sample),
                },
            )
            .collect::<Vec<_>>();
        let decision = prodex_mojo_core::runtime::smart_context_rewrite_telemetry_budget_decision(
            &recent,
            input.recent_rewrite_safety.safe_rewrites,
            input.recent_rewrite_safety.fallback_rewrites,
            input.recent_rewrite_safety.saved_tokens,
        )
        .expect("Mojo Smart Context telemetry planner returned invalid output");
        match decision {
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_DECISION_NO_CHANGE => {
                SmartContextRewriteBudgetDecision::NoChange
            }
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_DECISION_RELAX => {
                SmartContextRewriteBudgetDecision::Relax
            }
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_DECISION_TIGHTEN => {
                SmartContextRewriteBudgetDecision::Tighten
            }
            _ => unreachable!("Mojo Smart Context telemetry decision was validated"),
        }
    }

    #[cfg(not(feature = "mojo"))]
    smart_context_rewrite_telemetry_budget_decision_rust(input)
}

#[cfg(not(feature = "mojo"))]
fn smart_context_rewrite_telemetry_budget_decision_rust(
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
        total.saturating_add(sample.tokens_before.saturating_sub(sample.tokens_after))
    });
    let average_body_ratio_percent = if recent.is_empty() {
        100
    } else {
        recent.iter().fold(0usize, |total, sample| {
            let ratio = if sample.body_bytes_before == 0 {
                100
            } else {
                sample.body_bytes_after.saturating_mul(100) / sample.body_bytes_before
            };
            total.saturating_add(ratio)
        }) / recent.len()
    };
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

use super::*;
use crate::smart_context::{
    SmartContextTokenCountSource, smart_context_rewrite_telemetry_sample_quality_risk,
};

pub fn smart_context_recent_rewrite_safety_budget_decision(
    safety: &SmartContextRecentRewriteSafety,
) -> SmartContextRewriteBudgetDecision {
    smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
        recent_rewrite_safety: *safety,
        ..SmartContextRewriteTelemetryBudgetInput::default()
    })
}

pub fn smart_context_rewrite_telemetry_budget_decision(
    input: SmartContextRewriteTelemetryBudgetInput,
) -> SmartContextRewriteBudgetDecision {
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

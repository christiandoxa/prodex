use super::adaptive::SmartContextAdaptiveBudgetPolicy;
use super::types::{SmartContextBudgetMode, SmartContextRewriteBudgetDecision};
use crate::smart_context::{SmartContextTokenBudgetTier, smart_context_u64_saturating_usize};

pub fn smart_context_apply_rewrite_budget_decision(
    mut policy: SmartContextAdaptiveBudgetPolicy,
    decision: SmartContextRewriteBudgetDecision,
    available_context_tokens: Option<u64>,
) -> SmartContextAdaptiveBudgetPolicy {
    let adjusted = prodex_mojo_core::runtime::smart_context_budget_adjustment(
        match policy.tier {
            SmartContextTokenBudgetTier::Exact => 0,
            SmartContextTokenBudgetTier::Large => 1,
            SmartContextTokenBudgetTier::Condensed => 2,
            SmartContextTokenBudgetTier::Minimal => 3,
        },
        match policy.mode {
            SmartContextBudgetMode::ExactPassThrough => 0,
            SmartContextBudgetMode::LargeLossless => 1,
            SmartContextBudgetMode::ArtifactCondensed => 2,
            SmartContextBudgetMode::MinimalRefsOnly => 3,
        },
        u64::try_from(policy.max_inline_bytes).unwrap_or(u64::MAX),
        u64::try_from(policy.max_inline_tool_output_bytes).unwrap_or(u64::MAX),
        policy.max_rehydrate_tokens,
        match decision {
            SmartContextRewriteBudgetDecision::NoChange => {
                prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_DECISION_NO_CHANGE
            }
            SmartContextRewriteBudgetDecision::Relax => {
                prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_DECISION_RELAX
            }
            SmartContextRewriteBudgetDecision::Tighten => {
                prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_DECISION_TIGHTEN
            }
        },
        available_context_tokens,
    )
    .expect("Mojo Smart Context budget adjustment returned invalid output");

    policy.max_inline_bytes = smart_context_u64_saturating_usize(adjusted.max_inline_bytes);
    policy.max_inline_tool_output_bytes =
        smart_context_u64_saturating_usize(adjusted.max_inline_tool_output_bytes);
    policy.max_rehydrate_tokens = adjusted.max_rehydrate_tokens;
    policy
}

use super::adaptive::SmartContextAdaptiveBudgetPolicy;
use super::types::{SmartContextBudgetMode, SmartContextRewriteBudgetDecision};
#[cfg(feature = "mojo")]
use crate::smart_context::SmartContextTokenBudgetTier;
#[cfg(not(feature = "mojo"))]
use crate::smart_context::{
    smart_context_relaxed_inline_budget, smart_context_relaxed_rehydrate_budget,
    smart_context_tightened_inline_budget, smart_context_tightened_rehydrate_budget,
};

pub fn smart_context_apply_rewrite_budget_decision(
    policy: SmartContextAdaptiveBudgetPolicy,
    decision: SmartContextRewriteBudgetDecision,
    available_context_tokens: Option<u64>,
) -> SmartContextAdaptiveBudgetPolicy {
    #[cfg(feature = "mojo")]
    {
        let mut policy = policy;
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
            u64::try_from(policy.max_inline_tool_output_bytes)
                .expect("Smart Context inline budget fits u64"),
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
        policy.max_inline_tool_output_bytes = usize::try_from(adjusted.max_inline_bytes)
            .expect("Mojo Smart Context inline budget fits usize");
        policy.max_inline_bytes = policy.max_inline_tool_output_bytes;
        policy.max_rehydrate_tokens = adjusted.max_rehydrate_tokens;
        policy
    }

    #[cfg(not(feature = "mojo"))]
    smart_context_apply_rewrite_budget_decision_rust(policy, decision, available_context_tokens)
}

#[cfg(not(feature = "mojo"))]
fn smart_context_apply_rewrite_budget_decision_rust(
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

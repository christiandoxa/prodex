use super::*;
use crate::smart_context::{
    SmartContextExactnessDecision, SmartContextExactnessGuard, SmartContextObservedTokenAccounting,
    SmartContextTokenAccountingRisk, SmartContextTokenBudgetTier, non_empty,
    smart_context_relaxed_inline_budget, smart_context_relaxed_rehydrate_budget,
    smart_context_tightened_inline_budget, smart_context_tightened_rehydrate_budget,
    smart_context_token_budget_tier_from_accounting,
};

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

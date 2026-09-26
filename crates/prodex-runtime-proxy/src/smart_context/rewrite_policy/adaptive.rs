use super::*;
use crate::smart_context::{
    SmartContextExactnessDecision, SmartContextTokenAccountingRisk, non_empty,
};
use crate::smart_context::{
    SmartContextExactnessGuard, SmartContextObservedTokenAccounting, SmartContextTokenBudgetTier,
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
) -> Option<SmartContextAdaptiveBudgetPolicy> {
    let plan = prodex_mojo_core::runtime::smart_context_adaptive_budget_plan(
        prodex_mojo_core::runtime::SmartContextAdaptiveBudgetPlanInput {
            available_context_tokens: input.accounting.available_context_tokens,
            exactness_required: input.exactness_guard.decision
                == SmartContextExactnessDecision::RequireExact,
            static_context_changed: input.static_context_changed,
            missing_rehydrate_refs: input
                .missing_rehydrate_refs
                .iter()
                .any(|value| non_empty(value)),
            unknown_token_window: input
                .accounting
                .accounting_risks
                .contains(&SmartContextTokenAccountingRisk::UnknownTokenWindow),
            unsafe_accounting: input
                .accounting
                .accounting_risks
                .iter()
                .any(|risk| *risk != SmartContextTokenAccountingRisk::UnknownTokenWindow),
            safe_rewrites: input.recent_rewrite_safety.safe_rewrites,
            fallback_rewrites: input.recent_rewrite_safety.fallback_rewrites,
            saved_tokens: input.recent_rewrite_safety.saved_tokens,
        },
    )
    .expect("Mojo Smart Context adaptive budget planner returned invalid output");
    Some(SmartContextAdaptiveBudgetPolicy {
        tier: match plan.tier {
            0 => SmartContextTokenBudgetTier::Exact,
            1 => SmartContextTokenBudgetTier::Large,
            2 => SmartContextTokenBudgetTier::Condensed,
            3 => SmartContextTokenBudgetTier::Minimal,
            _ => unreachable!("Mojo Smart Context tier was validated"),
        },
        mode: match plan.mode {
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_MODE_EXACT => {
                SmartContextBudgetMode::ExactPassThrough
            }
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_MODE_LARGE => {
                SmartContextBudgetMode::LargeLossless
            }
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_MODE_CONDENSED => {
                SmartContextBudgetMode::ArtifactCondensed
            }
            prodex_mojo_core::runtime::SMART_CONTEXT_BUDGET_MODE_MINIMAL => {
                SmartContextBudgetMode::MinimalRefsOnly
            }
            _ => unreachable!("Mojo Smart Context mode was validated"),
        },
        max_inline_bytes: usize::try_from(plan.max_inline_bytes)
            .expect("Mojo Smart Context inline budget fits usize"),
        max_inline_tool_output_bytes: usize::try_from(plan.max_inline_bytes)
            .expect("Mojo Smart Context inline budget fits usize"),
        max_rehydrate_tokens: plan.max_rehydrate_tokens,
        reasons: smart_context_budget_policy_reasons_from_bits(plan.reason_bits),
    })
}

fn smart_context_budget_policy_reasons_from_bits(bits: u64) -> Vec<SmartContextBudgetPolicyReason> {
    [
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_POLICY_REASON_EXACTNESS_REQUIRED,
            SmartContextBudgetPolicyReason::ExactnessRequired,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_POLICY_REASON_STATIC_CONTEXT_CHANGED,
            SmartContextBudgetPolicyReason::StaticContextChanged,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_POLICY_REASON_MISSING_REHYDRATE_REFS,
            SmartContextBudgetPolicyReason::MissingRehydrateRefs,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_POLICY_REASON_UNKNOWN_TOKEN_WINDOW,
            SmartContextBudgetPolicyReason::UnknownTokenWindow,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_POLICY_REASON_UNSAFE_ACCOUNTING,
            SmartContextBudgetPolicyReason::UnsafeAccounting,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_POLICY_REASON_MODERATE_BUDGET,
            SmartContextBudgetPolicyReason::ModerateBudget,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_POLICY_REASON_TIGHT_BUDGET,
            SmartContextBudgetPolicyReason::TightBudget,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_POLICY_REASON_CRITICAL_BUDGET,
            SmartContextBudgetPolicyReason::CriticalBudget,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_POLICY_REASON_RECENT_REWRITE_SAVINGS_SAFE,
            SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe,
        ),
        (
            prodex_mojo_core::runtime::SMART_CONTEXT_POLICY_REASON_PLENTY_OF_BUDGET,
            SmartContextBudgetPolicyReason::PlentyOfBudget,
        ),
    ]
    .into_iter()
    .filter_map(|(bit, reason)| (bits & bit != 0).then_some(reason))
    .collect()
}

#[cfg(test)]
#[test]
fn adaptive_budget_policy_uses_mojo_in_every_feature_mode() {
    let accounting = SmartContextObservedTokenAccounting {
        model_context_window_tokens: None,
        observed_turns: 0,
        observed_input_tokens: 0,
        observed_cached_input_tokens: 0,
        observed_uncached_input_tokens: 0,
        observed_output_tokens: 0,
        observed_reasoning_tokens: 0,
        observed_total_tokens: 0,
        observed_context_tokens: 0,
        last_input_tokens: 0,
        last_accounted_input_tokens: 0,
        last_observed_context_tokens: 0,
        current_request_body_bytes: 0,
        estimated_current_request_tokens: 0,
        current_request_accounted_tokens: 0,
        effective_input_tokens: 0,
        effective_input_source: crate::smart_context::SmartContextTokenAccountingSource::Unknown,
        reserved_output_tokens: 0,
        available_context_tokens: None,
        accounting_risks: Vec::new(),
        pressure: crate::smart_context::SmartContextPressureSnapshot {
            model_context_window_tokens: None,
            reserved_output_tokens: 0,
            effective_usable_context_tokens: None,
            effective_used_tokens: 0,
            pressure_basis_points: None,
            pressure_band: crate::smart_context::SmartContextPressureBand::Unknown,
            absolute_safety_floor_tokens: 0,
            available_context_tokens: None,
            estimator_confidence: crate::smart_context::SmartContextEstimatorConfidence::Low,
        },
    };
    let policy = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: SmartContextExactnessGuard {
            decision: crate::smart_context::SmartContextExactnessDecision::Allow,
            reasons: Vec::new(),
        },
        accounting,
        recent_rewrite_safety: Default::default(),
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    })
    .expect("Mojo adaptive budget policy is available in every feature mode");
    assert_eq!(policy.tier, SmartContextTokenBudgetTier::Exact);
    assert_eq!(policy.mode, SmartContextBudgetMode::ExactPassThrough);
    assert_eq!(
        policy.reasons,
        vec![SmartContextBudgetPolicyReason::UnknownTokenWindow]
    );
}

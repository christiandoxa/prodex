use super::*;
#[cfg(any(not(feature = "mojo"), test))]
use crate::smart_context::smart_context_token_budget_tier_from_accounting;
use crate::smart_context::{
    SmartContextExactnessDecision, SmartContextExactnessGuard, SmartContextObservedTokenAccounting,
    SmartContextTokenAccountingRisk, SmartContextTokenBudgetTier, non_empty,
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
    #[cfg(feature = "mojo")]
    {
        let plan = prodex_mojo_core::runtime::smart_context_adaptive_budget_plan(
            input.accounting.available_context_tokens,
            input.exactness_guard.decision == SmartContextExactnessDecision::RequireExact,
            input.static_context_changed,
            input
                .missing_rehydrate_refs
                .iter()
                .any(|value| non_empty(value)),
            input
                .accounting
                .accounting_risks
                .contains(&SmartContextTokenAccountingRisk::UnknownTokenWindow),
            input
                .accounting
                .accounting_risks
                .iter()
                .any(|risk| *risk != SmartContextTokenAccountingRisk::UnknownTokenWindow),
            input.recent_rewrite_safety.safe_rewrites,
            input.recent_rewrite_safety.fallback_rewrites,
            input.recent_rewrite_safety.saved_tokens,
        )
        .expect("Mojo Smart Context adaptive budget planner returned invalid output");
        return SmartContextAdaptiveBudgetPolicy {
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
        };
    }

    #[cfg(not(feature = "mojo"))]
    smart_context_adaptive_budget_policy_rust(input)
}

#[cfg(any(not(feature = "mojo"), test))]
fn smart_context_adaptive_budget_policy_rust(
    input: SmartContextAdaptiveBudgetPolicyInput,
) -> SmartContextAdaptiveBudgetPolicy {
    let tier = smart_context_token_budget_tier_from_accounting(&input.accounting);
    let mut reasons = smart_context_budget_policy_reasons(&input);
    let available_context_tokens = input.accounting.available_context_tokens;

    if reasons.iter().any(|reason| {
        matches!(
            reason,
            SmartContextBudgetPolicyReason::ExactnessRequired
                | SmartContextBudgetPolicyReason::StaticContextChanged
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

    let has_missing_rehydrate_refs =
        reasons.contains(&SmartContextBudgetPolicyReason::MissingRehydrateRefs);
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
            if has_missing_rehydrate_refs {
                SmartContextBudgetMode::ArtifactCondensed
            } else {
                SmartContextBudgetMode::LargeLossless
            },
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
    let (mode, max_inline_tool_output_bytes, max_rehydrate_tokens) = if has_missing_rehydrate_refs {
        (
            SmartContextBudgetMode::ArtifactCondensed,
            max_inline_tool_output_bytes.min(8 * 1024),
            max_rehydrate_tokens.min(4_000),
        )
    } else {
        (mode, max_inline_tool_output_bytes, max_rehydrate_tokens)
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

#[cfg(feature = "mojo")]
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

#[cfg(any(not(feature = "mojo"), test))]
fn smart_context_budget_policy_reasons(
    input: &SmartContextAdaptiveBudgetPolicyInput,
) -> Vec<SmartContextBudgetPolicyReason> {
    let mut reasons = Vec::new();
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
    reasons
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_tests {
    use super::*;

    fn accounting(
        available_context_tokens: Option<u64>,
        unsafe_accounting: bool,
    ) -> SmartContextObservedTokenAccounting {
        SmartContextObservedTokenAccounting {
            model_context_window_tokens: available_context_tokens,
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
            effective_input_source:
                crate::smart_context::SmartContextTokenAccountingSource::Unknown,
            reserved_output_tokens: 0,
            available_context_tokens,
            accounting_risks: if unsafe_accounting {
                vec![SmartContextTokenAccountingRisk::ReservedOutputConsumesWindow]
            } else {
                Vec::new()
            },
            pressure: crate::smart_context::SmartContextPressureSnapshot {
                model_context_window_tokens: available_context_tokens,
                reserved_output_tokens: 0,
                effective_usable_context_tokens: available_context_tokens,
                effective_used_tokens: 0,
                pressure_basis_points: Some(0),
                pressure_band: crate::smart_context::SmartContextPressureBand::Low,
                absolute_safety_floor_tokens: 1_000,
                available_context_tokens,
                estimator_confidence: crate::smart_context::SmartContextEstimatorConfidence::High,
            },
        }
    }

    #[test]
    fn adaptive_budget_planner_matches_rust_oracle_at_boundaries() {
        for available in [
            None,
            Some(1_000),
            Some(2_000),
            Some(7_999),
            Some(8_000),
            Some(15_999),
            Some(16_000),
        ] {
            for exactness_required in [false, true] {
                for missing in [false, true] {
                    for unsafe_accounting in [false, true] {
                        let input = SmartContextAdaptiveBudgetPolicyInput {
                            exactness_guard: SmartContextExactnessGuard {
                                decision: if exactness_required {
                                    SmartContextExactnessDecision::RequireExact
                                } else {
                                    SmartContextExactnessDecision::Allow
                                },
                                reasons: Vec::new(),
                            },
                            accounting: accounting(available, unsafe_accounting),
                            recent_rewrite_safety: SmartContextRecentRewriteSafety {
                                safe_rewrites: 2,
                                fallback_rewrites: 0,
                                saved_tokens: 512,
                            },
                            static_context_changed: false,
                            missing_rehydrate_refs: if missing {
                                vec!["artifact".to_string()]
                            } else {
                                Vec::new()
                            },
                        };
                        let expected = smart_context_adaptive_budget_policy_rust(input.clone());
                        let actual = smart_context_adaptive_budget_policy(input);
                        assert_eq!(
                            actual, expected,
                            "available={available:?} exact={exactness_required} missing={missing} unsafe={unsafe_accounting}"
                        );
                    }
                }
            }
        }
    }
}

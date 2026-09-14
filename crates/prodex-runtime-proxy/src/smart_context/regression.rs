use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextRegressionSelfCheckDecision {
    Pass,
    FallbackExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextRegressionSelfCheckReason {
    ExactnessRequiredButPayloadChanged,
    TokenizerEstimateNotEligible,
    TokenBudgetDidNotImprove,
    TokenSavingsBelowSafetyMargin,
    CriticalSignalDropped,
    MissingRehydrateRefs,
    EmptyAfterPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRegressionSelfCheckInput {
    pub exactness_guard: SmartContextExactnessGuard,
    pub before_hash: String,
    pub after_hash: String,
    pub before_tokens: u64,
    pub after_tokens: u64,
    pub token_count_source: SmartContextTokenCountSource,
    pub future_retrieval_overhead_tokens: u64,
    pub injected_protocol_overhead_tokens: u64,
    pub expected_recovery_overhead_tokens: u64,
    pub before_critical_signal_count: usize,
    pub after_critical_signal_count: usize,
    pub missing_rehydrate_refs: Vec<String>,
    pub unresolved_rehydrate_refs_are_segment_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRegressionSelfCheck {
    pub decision: SmartContextRegressionSelfCheckDecision,
    pub reasons: Vec<SmartContextRegressionSelfCheckReason>,
    pub saved_tokens: u64,
    pub before_tokens: u64,
    pub after_tokens: u64,
    pub token_count_source: SmartContextTokenCountSource,
    pub before_hash: String,
    pub after_hash: String,
}

pub fn smart_context_regression_self_check(
    input: SmartContextRegressionSelfCheckInput,
) -> SmartContextRegressionSelfCheck {
    #[cfg(feature = "mojo")]
    {
        let payload_changed = input.before_hash != input.after_hash;
        let missing_rehydrate_refs = input
            .missing_rehydrate_refs
            .iter()
            .any(|value| non_empty(value));
        let plan = prodex_mojo_core::runtime::smart_context_regression_plan(
            prodex_mojo_core::runtime::SmartContextRegressionPlanInput {
                exactness_required: input.exactness_guard.decision
                    == SmartContextExactnessDecision::RequireExact,
                payload_changed,
                before_tokens: input.before_tokens,
                after_tokens: input.after_tokens,
                tokenizer_counted: input.token_count_source
                    == SmartContextTokenCountSource::TokenizerCounted,
                future_retrieval_overhead_tokens: input.future_retrieval_overhead_tokens,
                injected_protocol_overhead_tokens: input.injected_protocol_overhead_tokens,
                expected_recovery_overhead_tokens: input.expected_recovery_overhead_tokens,
                before_critical_signal_count: input.before_critical_signal_count as u64,
                after_critical_signal_count: input.after_critical_signal_count as u64,
                missing_rehydrate_refs,
                unresolved_refs_segment_local: input.unresolved_rehydrate_refs_are_segment_local,
            },
        )
        .expect("Mojo Smart Context regression plan returned invalid output");
        SmartContextRegressionSelfCheck {
            decision: if plan.fallback_exact {
                SmartContextRegressionSelfCheckDecision::FallbackExact
            } else {
                SmartContextRegressionSelfCheckDecision::Pass
            },
            reasons: smart_context_regression_reasons_from_bits(plan.reason_bits),
            saved_tokens: plan.saved_tokens,
            before_tokens: input.before_tokens,
            after_tokens: input.after_tokens,
            token_count_source: input.token_count_source,
            before_hash: input.before_hash,
            after_hash: input.after_hash,
        }
    }

    #[cfg(not(feature = "mojo"))]
    smart_context_regression_self_check_rust(input)
}

#[cfg(feature = "mojo")]
fn smart_context_regression_reasons_from_bits(
    bits: u64,
) -> Vec<SmartContextRegressionSelfCheckReason> {
    [
        SmartContextRegressionSelfCheckReason::ExactnessRequiredButPayloadChanged,
        SmartContextRegressionSelfCheckReason::TokenizerEstimateNotEligible,
        SmartContextRegressionSelfCheckReason::TokenBudgetDidNotImprove,
        SmartContextRegressionSelfCheckReason::TokenSavingsBelowSafetyMargin,
        SmartContextRegressionSelfCheckReason::CriticalSignalDropped,
        SmartContextRegressionSelfCheckReason::MissingRehydrateRefs,
        SmartContextRegressionSelfCheckReason::EmptyAfterPayload,
    ]
    .into_iter()
    .enumerate()
    .filter_map(|(index, reason)| (bits & (1 << index) != 0).then_some(reason))
    .collect()
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_regression_self_check_rust(
    input: SmartContextRegressionSelfCheckInput,
) -> SmartContextRegressionSelfCheck {
    let mut reasons = Vec::new();
    let payload_changed = input.before_hash != input.after_hash;
    let gross_saved_tokens = input.before_tokens.saturating_sub(input.after_tokens);
    let overhead_tokens = input
        .future_retrieval_overhead_tokens
        .saturating_add(input.injected_protocol_overhead_tokens)
        .saturating_add(input.expected_recovery_overhead_tokens);
    let net_saved_tokens = gross_saved_tokens.saturating_sub(overhead_tokens);
    let required_saved_tokens =
        128.max(input.before_tokens.saturating_mul(3).saturating_add(99) / 100);

    if input.exactness_guard.decision == SmartContextExactnessDecision::RequireExact
        && payload_changed
    {
        reasons.push(SmartContextRegressionSelfCheckReason::ExactnessRequiredButPayloadChanged);
    }
    if payload_changed && input.token_count_source != SmartContextTokenCountSource::TokenizerCounted
    {
        reasons.push(SmartContextRegressionSelfCheckReason::TokenizerEstimateNotEligible);
    }
    if payload_changed && input.after_tokens >= input.before_tokens {
        reasons.push(SmartContextRegressionSelfCheckReason::TokenBudgetDidNotImprove);
    } else if payload_changed && net_saved_tokens < required_saved_tokens {
        reasons.push(SmartContextRegressionSelfCheckReason::TokenSavingsBelowSafetyMargin);
    }
    if input.after_critical_signal_count < input.before_critical_signal_count {
        reasons.push(SmartContextRegressionSelfCheckReason::CriticalSignalDropped);
    }
    if input
        .missing_rehydrate_refs
        .iter()
        .any(|value| non_empty(value))
        && !input.unresolved_rehydrate_refs_are_segment_local
    {
        reasons.push(SmartContextRegressionSelfCheckReason::MissingRehydrateRefs);
    }
    if input.before_tokens > 0 && input.after_tokens == 0 {
        reasons.push(SmartContextRegressionSelfCheckReason::EmptyAfterPayload);
    }

    SmartContextRegressionSelfCheck {
        decision: if reasons.is_empty() {
            SmartContextRegressionSelfCheckDecision::Pass
        } else {
            SmartContextRegressionSelfCheckDecision::FallbackExact
        },
        reasons,
        saved_tokens: net_saved_tokens,
        before_tokens: input.before_tokens,
        after_tokens: input.after_tokens,
        token_count_source: input.token_count_source,
        before_hash: input.before_hash,
        after_hash: input.after_hash,
    }
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_tests {
    use super::*;

    #[test]
    fn regression_plan_matches_rust_oracle() {
        let mut state = 0x7265_6772_6573_7369_u64;
        for case in 0..1_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let input = SmartContextRegressionSelfCheckInput {
                exactness_guard: SmartContextExactnessGuard {
                    decision: if state & 1 == 0 {
                        SmartContextExactnessDecision::Allow
                    } else {
                        SmartContextExactnessDecision::RequireExact
                    },
                    reasons: Vec::new(),
                },
                before_hash: if state & 2 == 0 { "same" } else { "before" }.to_string(),
                after_hash: if state & 4 == 0 { "same" } else { "after" }.to_string(),
                before_tokens: state.rotate_left(7) % 20_000,
                after_tokens: state.rotate_right(11) % 20_000,
                token_count_source: if state & 8 == 0 {
                    SmartContextTokenCountSource::TokenizerCounted
                } else {
                    SmartContextTokenCountSource::Estimated
                },
                future_retrieval_overhead_tokens: state.rotate_left(17) % 2_000,
                injected_protocol_overhead_tokens: state.rotate_right(19) % 2_000,
                expected_recovery_overhead_tokens: state.rotate_left(23) % 2_000,
                before_critical_signal_count: (state.rotate_right(29) % 20) as usize,
                after_critical_signal_count: (state.rotate_left(31) % 20) as usize,
                missing_rehydrate_refs: (state & 16 != 0)
                    .then(|| "artifact".to_string())
                    .into_iter()
                    .collect(),
                unresolved_rehydrate_refs_are_segment_local: state & 32 != 0,
            };
            assert_eq!(
                smart_context_regression_self_check(input.clone()),
                smart_context_regression_self_check_rust(input),
                "regression case {case}"
            );
        }
    }
}

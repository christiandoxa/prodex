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

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextRegressionSelfCheckDecision {
    Pass,
    FallbackExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextRegressionSelfCheckReason {
    ExactnessRequiredButPayloadChanged,
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
    pub before_estimated_tokens: u64,
    pub after_estimated_tokens: u64,
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
    pub before_hash: String,
    pub after_hash: String,
}

pub fn smart_context_regression_self_check(
    input: SmartContextRegressionSelfCheckInput,
) -> SmartContextRegressionSelfCheck {
    let mut reasons = Vec::new();
    let payload_changed = input.before_hash != input.after_hash;
    let gross_saved_tokens = input
        .before_estimated_tokens
        .saturating_sub(input.after_estimated_tokens);
    let overhead_tokens = input
        .future_retrieval_overhead_tokens
        .saturating_add(input.injected_protocol_overhead_tokens)
        .saturating_add(input.expected_recovery_overhead_tokens);
    let net_saved_tokens = gross_saved_tokens.saturating_sub(overhead_tokens);
    let required_saved_tokens = 128.max(
        input
            .before_estimated_tokens
            .saturating_mul(3)
            .saturating_add(99)
            / 100,
    );

    if input.exactness_guard.decision == SmartContextExactnessDecision::RequireExact
        && payload_changed
    {
        reasons.push(SmartContextRegressionSelfCheckReason::ExactnessRequiredButPayloadChanged);
    }
    if payload_changed && input.after_estimated_tokens >= input.before_estimated_tokens {
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
    if input.before_estimated_tokens > 0 && input.after_estimated_tokens == 0 {
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
        before_hash: input.before_hash,
        after_hash: input.after_hash,
    }
}

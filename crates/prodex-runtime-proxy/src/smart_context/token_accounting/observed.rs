#[cfg(any(not(feature = "mojo"), test))]
use super::{
    smart_context_accounted_input_tokens, smart_context_observed_usage_context_tokens_rust,
};
use crate::RuntimeTokenUsage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SmartContextObservedUsageTotals {
    pub(super) input_tokens: u64,
    pub(super) cached_input_tokens: u64,
    pub(super) output_tokens: u64,
    pub(super) reasoning_tokens: u64,
    pub(super) last_input_tokens: u64,
    pub(super) last_accounted_input_tokens: u64,
    pub(super) last_observed_context_tokens: u64,
}

#[cfg(feature = "mojo")]
pub(super) fn smart_context_observed_usage_totals(
    usages: &[RuntimeTokenUsage],
) -> SmartContextObservedUsageTotals {
    #[cfg(feature = "mojo")]
    {
        let summary = crate::quota::mojo::smart_context_token_usage_summary(usages)
            .expect("Mojo Smart Context usage summary returned invalid output");
        SmartContextObservedUsageTotals {
            input_tokens: summary.observed_input_tokens,
            cached_input_tokens: summary.observed_cached_input_tokens,
            output_tokens: summary.observed_output_tokens,
            reasoning_tokens: summary.observed_reasoning_tokens,
            last_input_tokens: summary.last_input_tokens,
            last_accounted_input_tokens: summary.last_accounted_input_tokens,
            last_observed_context_tokens: summary.last_observed_context_tokens,
        }
    }

    #[cfg(not(feature = "mojo"))]
    smart_context_observed_usage_totals_rust(usages)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn smart_context_observed_usage_totals_rust(
    usages: &[RuntimeTokenUsage],
) -> SmartContextObservedUsageTotals {
    let mut totals = SmartContextObservedUsageTotals {
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        reasoning_tokens: 0,
        last_input_tokens: 0,
        last_accounted_input_tokens: 0,
        last_observed_context_tokens: 0,
    };
    for usage in usages {
        totals.input_tokens = totals.input_tokens.saturating_add(usage.input_tokens);
        totals.cached_input_tokens = totals
            .cached_input_tokens
            .saturating_add(usage.cached_input_tokens);
        totals.output_tokens = totals.output_tokens.saturating_add(usage.output_tokens);
        totals.reasoning_tokens = totals
            .reasoning_tokens
            .saturating_add(usage.reasoning_tokens);
        totals.last_input_tokens = usage.input_tokens;
        totals.last_accounted_input_tokens =
            smart_context_accounted_input_tokens(*usage).unwrap_or(0);
        totals.last_observed_context_tokens =
            smart_context_observed_usage_context_tokens_rust(*usage).unwrap_or(0);
    }
    totals
}

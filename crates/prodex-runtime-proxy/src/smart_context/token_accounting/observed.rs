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

pub(super) fn smart_context_observed_usage_totals(
    usages: &[RuntimeTokenUsage],
) -> SmartContextObservedUsageTotals {
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

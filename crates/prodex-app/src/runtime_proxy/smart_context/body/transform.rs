//! Smart-context JSON body transform steps that need proxy-state access.

use super::super::{
    RuntimeSmartContextBudget, RuntimeSmartContextProxyState, RuntimeSmartContextTransformOutcome,
};
use super::*;

pub(super) struct RuntimeSmartContextBodyTransformInput<'a> {
    pub(super) budget: &'a RuntimeSmartContextBudget,
}

pub(super) fn runtime_smart_context_transform_body(
    input: RuntimeSmartContextBodyTransformInput<'_>,
    state: &mut RuntimeSmartContextProxyState,
    value: &mut serde_json::Value,
) -> RuntimeSmartContextTransformOutcome {
    let mut outcome = RuntimeSmartContextTransformOutcome::default();
    let budget_allows_rewrite =
        input.budget.policy.mode != runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough;
    runtime_smart_context_rehydrate_value(value, &state.artifacts, &mut outcome.stats);
    if budget_allows_rewrite {
        runtime_smart_context_dedupe_input_text_within_request(value, &mut outcome.stats);
        runtime_smart_context_append_inline_reference_protocol(value, &outcome.stats);
    }
    outcome
}

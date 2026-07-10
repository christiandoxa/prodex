//! Smart-context JSON body transform steps that need proxy-state access.

use super::super::{RuntimeSmartContextBudget, RuntimeSmartContextIntentSignals};
use super::*;

pub(super) struct RuntimeSmartContextBodyTransformInput<'a> {
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) request_id: u64,
    pub(super) transform_exactness: &'a runtime_proxy_crate::SmartContextExactnessGuard,
    pub(super) budget: &'a RuntimeSmartContextBudget,
    pub(super) tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    pub(super) intent_signals: &'a RuntimeSmartContextIntentSignals,
}

pub(super) fn runtime_smart_context_transform_body(
    input: RuntimeSmartContextBodyTransformInput<'_>,
    value: &mut serde_json::Value,
) -> Option<RuntimeSmartContextTransformOutcome> {
    with_runtime_smart_context_proxy_state(input.shared, |state| {
        let mut outcome = RuntimeSmartContextTransformOutcome::default();
        let budget_allows_rewrite = input.budget.policy.mode
            != runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough;
        {
            let store = &mut state.artifacts;
            let rehydrate_plan = runtime_smart_context_auto_rehydrate_plan(
                value,
                store,
                input.budget.available_tokens,
                input.tier,
            );
            outcome.deferred_rehydrate_refs =
                runtime_smart_context_deferred_rehydrate_refs(&rehydrate_plan);
            runtime_smart_context_rehydrate_value_with_plan(
                value,
                store,
                &rehydrate_plan,
                &mut outcome.stats,
            );
            runtime_smart_context_selective_rehydrate_budget_aware_ranges(
                value,
                store,
                input.transform_exactness,
                &input.intent_signals.semantic_terms,
                &rehydrate_plan,
                input
                    .budget
                    .available_tokens
                    .saturating_sub(rehydrate_plan.used_tokens),
                &mut outcome.stats,
            );
        }
        runtime_smart_context_apply_repo_state_micro_cache(
            value,
            state,
            input.request_id,
            input.transform_exactness,
            budget_allows_rewrite,
            &mut outcome.stats,
        );
        if budget_allows_rewrite {
            let store = &mut state.artifacts;
            runtime_smart_context_condense_tool_outputs(
                value,
                store,
                input.request_id,
                input.tier,
                input.budget.policy.max_inline_tool_output_bytes,
                input.intent_signals,
                &mut outcome.stats,
            );
            runtime_smart_context_condense_historical_tool_call_arguments(
                value,
                store,
                input.request_id,
                input.tier,
                input.budget.policy.max_inline_tool_output_bytes,
                &mut outcome.stats,
            );
            runtime_smart_context_dedupe_input_text(
                value,
                store,
                input.transform_exactness,
                &mut outcome.stats,
            );
        }
        if budget_allows_rewrite {
            runtime_smart_context_append_artifact_manifest_delta_if_useful(
                value,
                state,
                &outcome.stats,
                input.intent_signals,
            );
        }
        outcome
    })
}

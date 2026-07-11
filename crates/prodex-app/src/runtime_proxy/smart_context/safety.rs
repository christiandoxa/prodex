use super::RuntimeSmartContextBudget;

#[cfg(test)]
pub(super) const RUNTIME_SMART_CONTEXT_MAX_JSON_DEPTH: usize =
    runtime_proxy_crate::SMART_CONTEXT_MAX_JSON_DEPTH;
#[cfg(test)]
pub(super) const RUNTIME_SMART_CONTEXT_MAX_JSON_NODES: usize =
    runtime_proxy_crate::SMART_CONTEXT_MAX_JSON_NODES;

pub(super) fn runtime_smart_context_unsupported_json_shape_reason(
    value: &serde_json::Value,
) -> Option<&'static str> {
    runtime_proxy_crate::smart_context_unsupported_json_shape_reason(value)
}

pub(super) fn runtime_smart_context_affinity_pressure_rewrite_allowed(
    exactness: &runtime_proxy_crate::SmartContextExactnessGuard,
    budget: &RuntimeSmartContextBudget,
) -> bool {
    runtime_proxy_crate::smart_context_affinity_pressure_rewrite_allowed(
        runtime_proxy_crate::SmartContextAffinityPressureRewriteInput {
            exactness_guard: exactness,
            tier: budget.tier,
            available_tokens: budget.available_tokens,
            policy_reasons: &budget.policy.reasons,
        },
    )
}

pub(super) fn runtime_smart_context_affinity_pressure_rewrite_guard(
    exactness: &runtime_proxy_crate::SmartContextExactnessGuard,
) -> runtime_proxy_crate::SmartContextExactnessGuard {
    runtime_proxy_crate::smart_context_affinity_pressure_rewrite_guard(exactness)
}

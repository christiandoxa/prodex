use super::*;

pub(super) const RUNTIME_SMART_CONTEXT_MAX_JSON_DEPTH: usize = 64;
pub(super) const RUNTIME_SMART_CONTEXT_MAX_JSON_NODES: usize = 50_000;

pub(super) fn runtime_smart_context_unsupported_json_shape_reason(
    value: &serde_json::Value,
) -> Option<&'static str> {
    let mut stack = vec![(value, 1usize)];
    let mut nodes = 0usize;
    while let Some((value, depth)) = stack.pop() {
        if depth > RUNTIME_SMART_CONTEXT_MAX_JSON_DEPTH {
            return Some("json_depth_limit");
        }
        nodes = nodes.saturating_add(1);
        if nodes > RUNTIME_SMART_CONTEXT_MAX_JSON_NODES {
            return Some("json_node_limit");
        }
        match value {
            serde_json::Value::Array(items) => {
                stack.extend(items.iter().map(|item| (item, depth.saturating_add(1))));
            }
            serde_json::Value::Object(object) => {
                stack.extend(object.values().map(|item| (item, depth.saturating_add(1))));
            }
            _ => {}
        }
    }
    None
}

pub(super) fn runtime_smart_context_affinity_pressure_rewrite_allowed(
    exactness: &runtime_proxy_crate::SmartContextExactnessGuard,
    budget: &RuntimeSmartContextBudget,
) -> bool {
    exactness.decision == runtime_proxy_crate::SmartContextExactnessDecision::RequireExact
        && !exactness.reasons.is_empty()
        && exactness
            .reasons
            .iter()
            .all(runtime_smart_context_exactness_reason_is_affinity)
        && runtime_smart_context_budget_has_critical_pressure(budget)
        && !runtime_smart_context_budget_has_non_affinity_safety_block(budget)
}

pub(super) fn runtime_smart_context_affinity_pressure_rewrite_guard(
    exactness: &runtime_proxy_crate::SmartContextExactnessGuard,
) -> runtime_proxy_crate::SmartContextExactnessGuard {
    runtime_proxy_crate::SmartContextExactnessGuard {
        decision: runtime_proxy_crate::SmartContextExactnessDecision::Allow,
        reasons: exactness.reasons.clone(),
    }
}

fn runtime_smart_context_exactness_reason_is_affinity(
    reason: &runtime_proxy_crate::SmartContextExactnessReason,
) -> bool {
    matches!(
        reason,
        runtime_proxy_crate::SmartContextExactnessReason::PreviousResponseAffinity
            | runtime_proxy_crate::SmartContextExactnessReason::TurnStateAffinity
            | runtime_proxy_crate::SmartContextExactnessReason::SessionAffinity
    )
}

fn runtime_smart_context_budget_has_critical_pressure(budget: &RuntimeSmartContextBudget) -> bool {
    budget.available_tokens == 0
        || budget.tier == runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal
}

fn runtime_smart_context_budget_has_non_affinity_safety_block(
    budget: &RuntimeSmartContextBudget,
) -> bool {
    budget.policy.reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextBudgetPolicyReason::StaticContextChanged
                | runtime_proxy_crate::SmartContextBudgetPolicyReason::MissingRehydrateRefs
                | runtime_proxy_crate::SmartContextBudgetPolicyReason::UnknownTokenWindow
                | runtime_proxy_crate::SmartContextBudgetPolicyReason::UnsafeAccounting
        )
    })
}

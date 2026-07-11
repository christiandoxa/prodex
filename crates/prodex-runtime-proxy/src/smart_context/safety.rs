use super::*;

pub const SMART_CONTEXT_MAX_JSON_DEPTH: usize = 64;
pub const SMART_CONTEXT_MAX_JSON_NODES: usize = 50_000;

pub fn smart_context_unsupported_json_shape_reason(
    value: &serde_json::Value,
) -> Option<&'static str> {
    let mut stack = vec![(value, 1usize)];
    let mut nodes = 0usize;
    while let Some((value, depth)) = stack.pop() {
        if depth > SMART_CONTEXT_MAX_JSON_DEPTH {
            return Some("json_depth_limit");
        }
        nodes = nodes.saturating_add(1);
        if nodes > SMART_CONTEXT_MAX_JSON_NODES {
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

#[derive(Debug, Clone, Copy)]
pub struct SmartContextAffinityPressureRewriteInput<'a> {
    pub exactness_guard: &'a SmartContextExactnessGuard,
    pub tier: SmartContextTokenBudgetTier,
    pub available_tokens: usize,
    pub policy_reasons: &'a [SmartContextBudgetPolicyReason],
}

pub fn smart_context_affinity_pressure_rewrite_allowed(
    input: SmartContextAffinityPressureRewriteInput<'_>,
) -> bool {
    input.exactness_guard.decision == SmartContextExactnessDecision::RequireExact
        && !input.exactness_guard.reasons.is_empty()
        && input
            .exactness_guard
            .reasons
            .iter()
            .all(smart_context_exactness_reason_is_affinity)
        && !smart_context_budget_has_non_affinity_safety_block(input.policy_reasons)
}

pub fn smart_context_affinity_pressure_rewrite_guard(
    exactness: &SmartContextExactnessGuard,
) -> SmartContextExactnessGuard {
    SmartContextExactnessGuard {
        decision: SmartContextExactnessDecision::Allow,
        reasons: exactness.reasons.clone(),
    }
}

fn smart_context_exactness_reason_is_affinity(reason: &SmartContextExactnessReason) -> bool {
    matches!(
        reason,
        SmartContextExactnessReason::PreviousResponseAffinity
            | SmartContextExactnessReason::TurnStateAffinity
            | SmartContextExactnessReason::SessionAffinity
    )
}

fn smart_context_budget_has_non_affinity_safety_block(
    reasons: &[SmartContextBudgetPolicyReason],
) -> bool {
    reasons.iter().any(|reason| {
        matches!(
            reason,
            SmartContextBudgetPolicyReason::StaticContextChanged
                | SmartContextBudgetPolicyReason::UnknownTokenWindow
                | SmartContextBudgetPolicyReason::UnsafeAccounting
        )
    })
}

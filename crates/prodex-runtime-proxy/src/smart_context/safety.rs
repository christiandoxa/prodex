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
    prodex_mojo_core::runtime::smart_context_affinity_rewrite_allowed(
        input.exactness_guard.decision == SmartContextExactnessDecision::RequireExact,
        input
            .exactness_guard
            .reasons
            .iter()
            .fold(0, |bits, reason| {
                bits | smart_context_exactness_reason_bit(*reason)
            }),
        input.policy_reasons.iter().fold(0, |bits, reason| {
            bits | smart_context_budget_reason_bit(*reason)
        }),
    )
    .expect("Mojo Smart Context affinity rewrite policy returned invalid output")
}

pub fn smart_context_affinity_pressure_rewrite_guard(
    exactness: &SmartContextExactnessGuard,
) -> SmartContextExactnessGuard {
    SmartContextExactnessGuard {
        decision: SmartContextExactnessDecision::Allow,
        reasons: exactness.reasons.clone(),
    }
}

const fn smart_context_exactness_reason_bit(reason: SmartContextExactnessReason) -> u64 {
    match reason {
        SmartContextExactnessReason::ExplicitExactMode => 1,
        SmartContextExactnessReason::PreviousResponseAffinity => 2,
        SmartContextExactnessReason::TurnStateAffinity => 4,
        SmartContextExactnessReason::SessionAffinity => 8,
        SmartContextExactnessReason::ToolOutputWithoutArtifact => 16,
    }
}

const fn smart_context_budget_reason_bit(reason: SmartContextBudgetPolicyReason) -> u64 {
    match reason {
        SmartContextBudgetPolicyReason::ExactnessRequired => 1,
        SmartContextBudgetPolicyReason::StaticContextChanged => 2,
        SmartContextBudgetPolicyReason::MissingRehydrateRefs => 4,
        SmartContextBudgetPolicyReason::UnknownTokenWindow => 8,
        SmartContextBudgetPolicyReason::UnsafeAccounting => 16,
        SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe => 32,
        SmartContextBudgetPolicyReason::PlentyOfBudget => 64,
        SmartContextBudgetPolicyReason::ModerateBudget => 128,
        SmartContextBudgetPolicyReason::TightBudget => 256,
        SmartContextBudgetPolicyReason::CriticalBudget => 512,
    }
}

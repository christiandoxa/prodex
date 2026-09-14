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
    #[cfg(feature = "mojo")]
    return prodex_mojo_core::runtime::smart_context_affinity_rewrite_allowed(
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
    .expect("Mojo Smart Context affinity rewrite policy returned invalid output");

    #[cfg(not(feature = "mojo"))]
    smart_context_affinity_pressure_rewrite_allowed_rust(input)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_affinity_pressure_rewrite_allowed_rust(
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

#[cfg(any(not(feature = "mojo"), test))]
fn smart_context_exactness_reason_is_affinity(reason: &SmartContextExactnessReason) -> bool {
    matches!(
        reason,
        SmartContextExactnessReason::PreviousResponseAffinity
            | SmartContextExactnessReason::TurnStateAffinity
            | SmartContextExactnessReason::SessionAffinity
    )
}

#[cfg(any(not(feature = "mojo"), test))]
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

#[cfg(feature = "mojo")]
const fn smart_context_exactness_reason_bit(reason: SmartContextExactnessReason) -> u64 {
    match reason {
        SmartContextExactnessReason::ExplicitExactMode => 1,
        SmartContextExactnessReason::PreviousResponseAffinity => 2,
        SmartContextExactnessReason::TurnStateAffinity => 4,
        SmartContextExactnessReason::SessionAffinity => 8,
        SmartContextExactnessReason::ToolOutputWithoutArtifact => 16,
    }
}

#[cfg(feature = "mojo")]
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

#[cfg(all(test, feature = "mojo"))]
mod mojo_tests {
    use super::*;

    #[test]
    fn affinity_policy_matches_rust_oracle() {
        let exactness_reasons = [
            SmartContextExactnessReason::ExplicitExactMode,
            SmartContextExactnessReason::PreviousResponseAffinity,
            SmartContextExactnessReason::TurnStateAffinity,
            SmartContextExactnessReason::SessionAffinity,
            SmartContextExactnessReason::ToolOutputWithoutArtifact,
        ];
        let policy_reasons = [
            SmartContextBudgetPolicyReason::ExactnessRequired,
            SmartContextBudgetPolicyReason::StaticContextChanged,
            SmartContextBudgetPolicyReason::MissingRehydrateRefs,
            SmartContextBudgetPolicyReason::UnknownTokenWindow,
            SmartContextBudgetPolicyReason::UnsafeAccounting,
            SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe,
            SmartContextBudgetPolicyReason::PlentyOfBudget,
            SmartContextBudgetPolicyReason::ModerateBudget,
            SmartContextBudgetPolicyReason::TightBudget,
            SmartContextBudgetPolicyReason::CriticalBudget,
        ];
        for exactness_mask in 0..32_u16 {
            for policy_mask in 0..1_024_u16 {
                let exactness = SmartContextExactnessGuard {
                    decision: if exactness_mask & 1 == 0 {
                        SmartContextExactnessDecision::Allow
                    } else {
                        SmartContextExactnessDecision::RequireExact
                    },
                    reasons: exactness_reasons
                        .iter()
                        .enumerate()
                        .filter_map(|(index, reason)| {
                            (exactness_mask & (1 << index) != 0).then_some(*reason)
                        })
                        .collect(),
                };
                let reasons = policy_reasons
                    .iter()
                    .enumerate()
                    .filter_map(|(index, reason)| {
                        (policy_mask & (1 << index) != 0).then_some(*reason)
                    })
                    .collect::<Vec<_>>();
                let input = SmartContextAffinityPressureRewriteInput {
                    exactness_guard: &exactness,
                    tier: SmartContextTokenBudgetTier::Exact,
                    available_tokens: 1_000,
                    policy_reasons: &reasons,
                };
                assert_eq!(
                    smart_context_affinity_pressure_rewrite_allowed(input),
                    smart_context_affinity_pressure_rewrite_allowed_rust(input)
                );
            }
        }
    }
}

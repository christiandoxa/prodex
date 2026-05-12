use super::*;

#[test]
fn unsupported_json_shape_allows_limit_boundary_and_rejects_deeper_tree() {
    let mut boundary = serde_json::Value::Null;
    for _ in 1..SMART_CONTEXT_MAX_JSON_DEPTH {
        boundary = serde_json::json!([boundary]);
    }
    assert_eq!(smart_context_unsupported_json_shape_reason(&boundary), None);

    let too_deep = serde_json::json!([boundary]);
    assert_eq!(
        smart_context_unsupported_json_shape_reason(&too_deep),
        Some("json_depth_limit")
    );
}

#[test]
fn unsupported_json_shape_rejects_too_many_nodes() {
    let value =
        serde_json::Value::Array(vec![serde_json::Value::Null; SMART_CONTEXT_MAX_JSON_NODES]);

    assert_eq!(
        smart_context_unsupported_json_shape_reason(&value),
        Some("json_node_limit")
    );
}

#[test]
fn affinity_pressure_rewrite_allows_only_affinity_under_critical_pressure() {
    let exactness = SmartContextExactnessGuard {
        decision: SmartContextExactnessDecision::RequireExact,
        reasons: vec![
            SmartContextExactnessReason::PreviousResponseAffinity,
            SmartContextExactnessReason::SessionAffinity,
        ],
    };

    assert!(smart_context_affinity_pressure_rewrite_allowed(
        SmartContextAffinityPressureRewriteInput {
            exactness_guard: &exactness,
            tier: SmartContextTokenBudgetTier::Minimal,
            available_tokens: 1_000,
            policy_reasons: &[SmartContextBudgetPolicyReason::ExactnessRequired],
        },
    ));

    assert!(smart_context_affinity_pressure_rewrite_allowed(
        SmartContextAffinityPressureRewriteInput {
            exactness_guard: &exactness,
            tier: SmartContextTokenBudgetTier::Large,
            available_tokens: 0,
            policy_reasons: &[SmartContextBudgetPolicyReason::ExactnessRequired],
        },
    ));
}

#[test]
fn affinity_pressure_rewrite_blocks_non_affinity_or_safety_reasons() {
    let explicit = SmartContextExactnessGuard {
        decision: SmartContextExactnessDecision::RequireExact,
        reasons: vec![SmartContextExactnessReason::ExplicitExactMode],
    };
    assert!(!smart_context_affinity_pressure_rewrite_allowed(
        SmartContextAffinityPressureRewriteInput {
            exactness_guard: &explicit,
            tier: SmartContextTokenBudgetTier::Minimal,
            available_tokens: 1_000,
            policy_reasons: &[SmartContextBudgetPolicyReason::ExactnessRequired],
        },
    ));

    let affinity = SmartContextExactnessGuard {
        decision: SmartContextExactnessDecision::RequireExact,
        reasons: vec![SmartContextExactnessReason::TurnStateAffinity],
    };
    assert!(!smart_context_affinity_pressure_rewrite_allowed(
        SmartContextAffinityPressureRewriteInput {
            exactness_guard: &affinity,
            tier: SmartContextTokenBudgetTier::Minimal,
            available_tokens: 1_000,
            policy_reasons: &[SmartContextBudgetPolicyReason::StaticContextChanged],
        },
    ));

    assert!(!smart_context_affinity_pressure_rewrite_allowed(
        SmartContextAffinityPressureRewriteInput {
            exactness_guard: &affinity,
            tier: SmartContextTokenBudgetTier::Large,
            available_tokens: 1_000,
            policy_reasons: &[SmartContextBudgetPolicyReason::ExactnessRequired],
        },
    ));
}

#[test]
fn affinity_pressure_rewrite_guard_preserves_reasons_but_allows_rewrite() {
    let exactness = SmartContextExactnessGuard {
        decision: SmartContextExactnessDecision::RequireExact,
        reasons: vec![SmartContextExactnessReason::PreviousResponseAffinity],
    };

    let rewritten = smart_context_affinity_pressure_rewrite_guard(&exactness);

    assert_eq!(rewritten.decision, SmartContextExactnessDecision::Allow);
    assert_eq!(rewritten.reasons, exactness.reasons);
}

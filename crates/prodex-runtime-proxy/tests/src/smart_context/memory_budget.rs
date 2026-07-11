use super::*;

#[test]
fn memory_capsule_selection_prioritizes_required_then_relevance() {
    let selected = smart_context_select_memory_capsules(
        [
            SmartContextMemoryCapsule {
                id: "optional-low".to_string(),
                token_cost: 40,
                relevance: 0.1,
                required: false,
            },
            SmartContextMemoryCapsule {
                id: "required".to_string(),
                token_cost: 60,
                relevance: 0.0,
                required: true,
            },
            SmartContextMemoryCapsule {
                id: "optional-high".to_string(),
                token_cost: 30,
                relevance: 0.9,
                required: false,
            },
        ],
        90,
    );

    assert_eq!(selected.selected_ids, vec!["required", "optional-high"]);
    assert_eq!(selected.omitted_ids, vec!["optional-low"]);
    assert_eq!(selected.used_tokens, 90);
}

#[test]
fn memory_capsule_budget_bridge_scales_by_policy_mode_and_allows_safe_exact() {
    let minimal = smart_context_memory_capsule_policy_for_available_tokens(1_500);
    let condensed = smart_context_memory_capsule_policy_for_available_tokens(6_000);
    let large = smart_context_memory_capsule_policy_for_available_tokens(12_000);
    let exact = smart_context_memory_capsule_policy_for_available_tokens(40_000);

    assert_eq!(
        smart_context_memory_capsule_token_budget(&minimal.0, &minimal.1),
        SMART_CONTEXT_MEMORY_CAPSULE_MINIMAL_TOKEN_BUDGET
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget(&condensed.0, &condensed.1),
        SMART_CONTEXT_MEMORY_CAPSULE_CONDENSED_TOKEN_BUDGET
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget(&large.0, &large.1),
        SMART_CONTEXT_MEMORY_CAPSULE_LARGE_TOKEN_BUDGET
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget(&exact.0, &exact.1),
        usize::MAX
    );
}

#[test]
fn memory_capsule_budget_bridge_does_not_unbound_unsafe_exact_policy() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: None,
            reserved_output_tokens: 8_000,
            current_input_tokens: 12_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });
    let policy = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting: accounting.clone(),
        recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });

    assert_eq!(policy.mode, SmartContextBudgetMode::ExactPassThrough);
    assert_eq!(
        smart_context_memory_capsule_token_budget(&accounting, &policy),
        0
    );
}

#[test]
fn memory_capsule_selection_keeps_scanning_after_oversized_required_capsule() {
    let minimal = smart_context_memory_capsule_policy_for_available_tokens(1_500);
    let selected = smart_context_select_memory_capsules_for_policy(
        [
            SmartContextMemoryCapsule {
                id: "required-a-too-large".to_string(),
                token_cost: 300,
                relevance: 0.0,
                required: true,
            },
            SmartContextMemoryCapsule {
                id: "required-z-small".to_string(),
                token_cost: 100,
                relevance: 0.0,
                required: true,
            },
            SmartContextMemoryCapsule {
                id: "optional-high".to_string(),
                token_cost: 100,
                relevance: 0.9,
                required: false,
            },
            SmartContextMemoryCapsule {
                id: "optional-low".to_string(),
                token_cost: 100,
                relevance: 0.1,
                required: false,
            },
        ],
        &minimal.0,
        &minimal.1,
    );

    assert_eq!(
        selected.selected_ids,
        vec!["required-z-small", "optional-high"]
    );
    assert_eq!(
        selected.omitted_ids,
        vec!["required-a-too-large", "optional-low"]
    );
    assert_eq!(selected.used_tokens, 200);
}

#[test]
fn rehydrate_plan_respects_artifacts_tier_and_budget() {
    let plan = smart_context_auto_rehydrate_plan(
        [
            SmartContextRehydrateRef {
                id: "required".to_string(),
                token_cost: 70,
                required: true,
            },
            SmartContextRehydrateRef {
                id: "missing".to_string(),
                token_cost: 10,
                required: false,
            },
            SmartContextRehydrateRef {
                id: "optional".to_string(),
                token_cost: 10,
                required: false,
            },
        ],
        ["required".to_string(), "optional".to_string()],
        80,
        SmartContextTokenBudgetTier::Minimal,
    );

    assert_eq!(
        plan.actions,
        vec![
            SmartContextRehydrateAction::Rehydrate {
                id: "required".to_string(),
                token_cost: 70,
            },
            SmartContextRehydrateAction::Defer {
                id: "missing".to_string(),
                reason: SmartContextRehydrateDeferReason::MissingArtifact,
            },
            SmartContextRehydrateAction::Defer {
                id: "optional".to_string(),
                reason: SmartContextRehydrateDeferReason::MinimalBudgetTier,
            },
        ]
    );
    assert_eq!(plan.used_tokens, 70);
}

#[test]
fn token_budget_tiers_are_stable_boundaries() {
    assert_eq!(
        smart_context_token_budget_tier(1_999),
        SmartContextTokenBudgetTier::Minimal
    );
    assert_eq!(
        smart_context_token_budget_tier(2_000),
        SmartContextTokenBudgetTier::Condensed
    );
    assert_eq!(
        smart_context_token_budget_tier(8_000),
        SmartContextTokenBudgetTier::Large
    );
    assert_eq!(
        smart_context_token_budget_tier(16_000),
        SmartContextTokenBudgetTier::Exact
    );
}

fn smart_context_memory_capsule_policy_for_available_tokens(
    available_tokens: u64,
) -> (
    SmartContextObservedTokenAccounting,
    SmartContextAdaptiveBudgetPolicy,
) {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 120_000u64.saturating_sub(available_tokens),
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });
    let policy = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting: accounting.clone(),
        recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });

    (accounting, policy)
}

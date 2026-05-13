use super::super::*;

#[test]
fn adaptive_budget_policy_prefers_safe_exact_when_required() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 125_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });
    let guard = smart_context_exactness_guard(SmartContextExactnessInput {
        previous_response_id: Some("resp-owned".to_string()),
        ..SmartContextExactnessInput::default()
    });

    let policy = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: guard,
        accounting,
        recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });

    assert_eq!(policy.tier, SmartContextTokenBudgetTier::Minimal);
    assert_eq!(policy.mode, SmartContextBudgetMode::ExactPassThrough);
    assert_eq!(
        policy.reasons,
        vec![SmartContextBudgetPolicyReason::ExactnessRequired]
    );
}

#[test]
fn adaptive_budget_policy_uses_condensed_and_minimal_modes_by_real_budget() {
    let condensed =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 114_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });
    let minimal =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 119_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });

    let condensed_policy =
        smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: condensed,
            recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        });
    let minimal_policy =
        smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: minimal,
            recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        });

    assert_eq!(
        condensed_policy.tier,
        SmartContextTokenBudgetTier::Condensed
    );
    assert_eq!(
        condensed_policy.mode,
        SmartContextBudgetMode::ArtifactCondensed
    );
    assert_eq!(condensed_policy.max_inline_tool_output_bytes, 8 * 1024);
    assert_eq!(
        condensed_policy.reasons,
        vec![SmartContextBudgetPolicyReason::TightBudget]
    );
    assert_eq!(minimal_policy.tier, SmartContextTokenBudgetTier::Minimal);
    assert_eq!(minimal_policy.mode, SmartContextBudgetMode::MinimalRefsOnly);
    assert_eq!(minimal_policy.max_inline_bytes, 1024);
    assert_eq!(minimal_policy.max_inline_tool_output_bytes, 1024);
}

#[test]
fn adaptive_budget_policy_caps_rehydrate_budget_to_available_tokens() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(20_000),
            reserved_output_tokens: 0,
            current_input_tokens: 11_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });

    let policy = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting,
        recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });

    assert_eq!(policy.tier, SmartContextTokenBudgetTier::Large);
    assert_eq!(policy.max_inline_bytes, 32 * 1024);
    assert_eq!(policy.max_inline_tool_output_bytes, 32 * 1024);
    assert_eq!(policy.max_rehydrate_tokens, 9_000);
}

#[test]
fn adaptive_budget_policy_falls_back_exact_when_accounting_unknown_or_unsafe() {
    let unknown =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: None,
            reserved_output_tokens: 8_000,
            current_input_tokens: 12_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });
    let unsafe_accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(8_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 1,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });

    let unknown_policy =
        smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: unknown,
            recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        });
    let unsafe_policy =
        smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: unsafe_accounting,
            recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        });

    assert_eq!(
        unknown_policy.mode,
        SmartContextBudgetMode::ExactPassThrough
    );
    assert_eq!(
        unknown_policy.reasons,
        vec![SmartContextBudgetPolicyReason::UnknownTokenWindow]
    );
    assert_eq!(unsafe_policy.mode, SmartContextBudgetMode::ExactPassThrough);
    assert_eq!(
        unsafe_policy.reasons,
        vec![SmartContextBudgetPolicyReason::UnsafeAccounting]
    );
    assert_eq!(unsafe_policy.max_inline_bytes, usize::MAX);
}

#[test]
fn adaptive_budget_policy_expands_preview_only_after_recent_safe_savings() {
    let large = smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
        model_context_window_tokens: Some(64_000),
        reserved_output_tokens: 4_096,
        current_input_tokens: 48_000,
        current_request_body_bytes: 0,
        current_request_estimated_tokens: None,
        observed_usage: Vec::new(),
    });
    let exact = smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
        model_context_window_tokens: Some(128_000),
        reserved_output_tokens: 4_096,
        current_input_tokens: 32_000,
        current_request_body_bytes: 0,
        current_request_estimated_tokens: None,
        observed_usage: Vec::new(),
    });
    let safe = SmartContextRecentRewriteSafety {
        safe_rewrites: 2,
        fallback_rewrites: 0,
        saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2,
    };
    let mixed = SmartContextRecentRewriteSafety {
        safe_rewrites: 2,
        fallback_rewrites: 1,
        saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2,
    };

    let large_safe = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting: large.clone(),
        recent_rewrite_safety: safe,
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });
    let large_mixed = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting: large,
        recent_rewrite_safety: mixed,
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });
    let exact_safe = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting: exact,
        recent_rewrite_safety: safe,
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });

    assert_eq!(large_safe.tier, SmartContextTokenBudgetTier::Large);
    assert_eq!(large_safe.max_inline_tool_output_bytes, 64 * 1024);
    assert!(
        large_safe
            .reasons
            .contains(&SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe)
    );
    assert_eq!(
        large_mixed.max_inline_tool_output_bytes,
        (32 * 1024) * 9 / 10
    );
    assert!(
        !large_mixed
            .reasons
            .contains(&SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe)
    );
    assert_eq!(exact_safe.tier, SmartContextTokenBudgetTier::Exact);
    assert_eq!(exact_safe.mode, SmartContextBudgetMode::ExactPassThrough);
    assert_eq!(exact_safe.max_inline_tool_output_bytes, usize::MAX);
    assert!(
        !exact_safe
            .reasons
            .contains(&SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe)
    );
}

#[test]
fn recent_rewrite_safety_requires_savings_without_fallbacks() {
    assert!(!smart_context_recent_rewrite_safety_allows_larger_preview(
        &SmartContextRecentRewriteSafety {
            safe_rewrites: 2,
            fallback_rewrites: 0,
            saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2 - 1,
        }
    ));
    assert!(!smart_context_recent_rewrite_safety_allows_larger_preview(
        &SmartContextRecentRewriteSafety {
            safe_rewrites: 1,
            fallback_rewrites: 1,
            saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2,
        }
    ));
    assert!(smart_context_recent_rewrite_safety_allows_larger_preview(
        &SmartContextRecentRewriteSafety {
            safe_rewrites: 1,
            fallback_rewrites: 0,
            saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS,
        }
    ));
}

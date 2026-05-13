use super::super::*;

#[test]
fn regression_self_check_falls_back_on_quality_risks() {
    let check = smart_context_regression_self_check(SmartContextRegressionSelfCheckInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput {
            turn_state: Some("turn".to_string()),
            ..SmartContextExactnessInput::default()
        }),
        before_hash: smart_context_hash_text("before"),
        after_hash: smart_context_hash_text("after"),
        before_estimated_tokens: 100,
        after_estimated_tokens: 100,
        before_critical_signal_count: 3,
        after_critical_signal_count: 2,
        missing_rehydrate_refs: vec!["artifact-missing".to_string()],
    });

    assert_eq!(
        check.decision,
        SmartContextRegressionSelfCheckDecision::FallbackExact
    );
    assert_eq!(
        check.reasons,
        vec![
            SmartContextRegressionSelfCheckReason::ExactnessRequiredButPayloadChanged,
            SmartContextRegressionSelfCheckReason::TokenBudgetDidNotImprove,
            SmartContextRegressionSelfCheckReason::CriticalSignalDropped,
            SmartContextRegressionSelfCheckReason::MissingRehydrateRefs,
        ]
    );
    assert_eq!(check.saved_tokens, 0);
}

#[test]
fn regression_self_check_passes_when_condense_saves_and_signals_preserved() {
    let check = smart_context_regression_self_check(SmartContextRegressionSelfCheckInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        before_hash: smart_context_hash_text("long before"),
        after_hash: smart_context_hash_text("short after"),
        before_estimated_tokens: 400,
        after_estimated_tokens: 100,
        before_critical_signal_count: 2,
        after_critical_signal_count: 2,
        missing_rehydrate_refs: Vec::new(),
    });

    assert_eq!(
        check.decision,
        SmartContextRegressionSelfCheckDecision::Pass
    );
    assert!(check.reasons.is_empty());
    assert_eq!(check.saved_tokens, 300);
}

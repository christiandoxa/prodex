use super::super::*;
use super::smart_context_test_rewrite_telemetry_sample;

#[test]
fn rewrite_telemetry_budget_decision_relaxes_after_safe_savings() {
    let decision =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: vec![
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
                smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
            ],
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });

    assert_eq!(decision, SmartContextRewriteBudgetDecision::Relax);
}

#[test]
fn rewrite_telemetry_budget_decision_tightens_after_fallback_or_weak_savings() {
    let fallback =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: vec![SmartContextRewriteTelemetrySample {
                fallback: true,
                ..smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000)
            }],
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });
    let weak =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: vec![
                smart_context_test_rewrite_telemetry_sample(10_000, 9_000, 2_500, 2_400),
                smart_context_test_rewrite_telemetry_sample(8_000, 7_200, 2_000, 1_900),
            ],
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });

    assert_eq!(fallback, SmartContextRewriteBudgetDecision::Tighten);
    assert_eq!(weak, SmartContextRewriteBudgetDecision::Tighten);
}

#[test]
fn rewrite_telemetry_budget_decision_tightens_after_quality_risk() {
    let decision =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: vec![SmartContextRewriteTelemetrySample {
                model_reread_requests: 1,
                ..smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000)
            }],
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });

    assert_eq!(decision, SmartContextRewriteBudgetDecision::Tighten);
}

#[test]
fn rewrite_telemetry_budget_decision_keeps_neutral_for_moderate_safe_savings() {
    let decision =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: vec![
                smart_context_test_rewrite_telemetry_sample(10_000, 7_500, 2_500, 2_100),
                smart_context_test_rewrite_telemetry_sample(8_000, 6_000, 2_000, 1_650),
            ],
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });

    assert_eq!(decision, SmartContextRewriteBudgetDecision::NoChange);
}

#[test]
fn rewrite_budget_application_respects_bounds() {
    let policy = SmartContextAdaptiveBudgetPolicy {
        tier: SmartContextTokenBudgetTier::Condensed,
        mode: SmartContextBudgetMode::ArtifactCondensed,
        max_inline_bytes: 300,
        max_inline_tool_output_bytes: 300,
        max_rehydrate_tokens: 2,
        reasons: vec![SmartContextBudgetPolicyReason::TightBudget],
    };

    let tightened = smart_context_apply_rewrite_budget_decision(
        policy.clone(),
        SmartContextRewriteBudgetDecision::Tighten,
        Some(10),
    );
    let relaxed = smart_context_apply_rewrite_budget_decision(
        policy,
        SmartContextRewriteBudgetDecision::Relax,
        Some(2),
    );

    assert_eq!(tightened.max_inline_tool_output_bytes, 270);
    assert_eq!(tightened.max_inline_bytes, 270);
    assert_eq!(tightened.max_rehydrate_tokens, 1);
    assert_eq!(relaxed.max_inline_tool_output_bytes, 375);
    assert_eq!(relaxed.max_inline_bytes, 375);
    assert_eq!(relaxed.max_rehydrate_tokens, 2);
}

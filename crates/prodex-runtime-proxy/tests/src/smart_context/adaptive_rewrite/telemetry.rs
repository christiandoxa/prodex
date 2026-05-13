use super::super::*;
use super::{
    smart_context_test_rewrite_telemetry_sample,
    smart_context_test_transform_rewrite_telemetry_sample, smart_context_test_transform_score,
};

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
fn per_transform_rewrite_safety_scores_categories_independently() {
    let samples = vec![
        smart_context_test_transform_rewrite_telemetry_sample(
            SmartContextTransformCategory::CommandOutputCache,
            smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
        ),
        smart_context_test_transform_rewrite_telemetry_sample(
            SmartContextTransformCategory::CommandOutputCache,
            smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
        ),
        smart_context_test_transform_rewrite_telemetry_sample(
            SmartContextTransformCategory::StaticContextPromptCache,
            smart_context_test_rewrite_telemetry_sample(10_000, 9_000, 2_500, 2_450),
        ),
        smart_context_test_transform_rewrite_telemetry_sample(
            SmartContextTransformCategory::StaticContextPromptCache,
            smart_context_test_rewrite_telemetry_sample(8_000, 7_200, 2_000, 1_950),
        ),
        smart_context_test_transform_rewrite_telemetry_sample(
            SmartContextTransformCategory::CrossTurnDuplicateRef,
            SmartContextRewriteTelemetrySample {
                fallback: true,
                ..smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000)
            },
        ),
    ];
    let global =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: samples.iter().map(|sample| sample.sample).collect(),
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });

    let scores = smart_context_per_transform_rewrite_safety_scores(
        SmartContextPerTransformRewriteSafetyInput {
            recent_rewrite_safety: Vec::new(),
            telemetry_samples: samples,
        },
    );
    let command = smart_context_test_transform_score(
        &scores,
        SmartContextTransformCategory::CommandOutputCache,
    );
    let static_context = smart_context_test_transform_score(
        &scores,
        SmartContextTransformCategory::StaticContextPromptCache,
    );
    let cross_turn = smart_context_test_transform_score(
        &scores,
        SmartContextTransformCategory::CrossTurnDuplicateRef,
    );

    assert_eq!(global, SmartContextRewriteBudgetDecision::Tighten);
    assert_eq!(command.decision, SmartContextRewriteBudgetDecision::Relax);
    assert_eq!(command.safety_score, 100);
    assert_eq!(
        command.reasons,
        vec![SmartContextTransformRewriteSafetyReason::StableSavings]
    );
    assert_eq!(
        static_context.decision,
        SmartContextRewriteBudgetDecision::Tighten
    );
    assert_eq!(
        static_context.reasons,
        vec![SmartContextTransformRewriteSafetyReason::WeakSavings]
    );
    assert_eq!(
        cross_turn.decision,
        SmartContextRewriteBudgetDecision::Tighten
    );
    assert_eq!(cross_turn.fallback_samples, 1);
    assert_eq!(
        cross_turn.reasons,
        vec![SmartContextTransformRewriteSafetyReason::FallbackObserved]
    );
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

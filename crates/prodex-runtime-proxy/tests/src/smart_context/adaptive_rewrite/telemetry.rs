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

fn rewrite_budget_policy(
    tier: SmartContextTokenBudgetTier,
    mode: SmartContextBudgetMode,
    max_inline_bytes: usize,
    max_inline_tool_output_bytes: usize,
    max_rehydrate_tokens: u64,
) -> SmartContextAdaptiveBudgetPolicy {
    SmartContextAdaptiveBudgetPolicy {
        tier,
        mode,
        max_inline_bytes,
        max_inline_tool_output_bytes,
        max_rehydrate_tokens,
        reasons: vec![SmartContextBudgetPolicyReason::CriticalBudget],
    }
}

#[test]
fn rewrite_budget_adjustment_preserves_mode_and_available_context_contracts() {
    let exact = rewrite_budget_policy(
        SmartContextTokenBudgetTier::Minimal,
        SmartContextBudgetMode::ExactPassThrough,
        91,
        301,
        12,
    );
    assert_eq!(
        smart_context_apply_rewrite_budget_decision(
            exact.clone(),
            SmartContextRewriteBudgetDecision::Relax,
            Some(7),
        ),
        rewrite_budget_policy(
            SmartContextTokenBudgetTier::Minimal,
            SmartContextBudgetMode::ExactPassThrough,
            91,
            301,
            12,
        )
    );

    assert_eq!(
        smart_context_apply_rewrite_budget_decision(
            rewrite_budget_policy(
                SmartContextTokenBudgetTier::Exact,
                SmartContextBudgetMode::ArtifactCondensed,
                11,
                42,
                9,
            ),
            SmartContextRewriteBudgetDecision::NoChange,
            Some(0),
        ),
        rewrite_budget_policy(
            SmartContextTokenBudgetTier::Exact,
            SmartContextBudgetMode::ArtifactCondensed,
            11,
            42,
            0,
        )
    );
}

#[test]
fn rewrite_budget_adjustment_preserves_tier_rounding_and_clamps() {
    assert_eq!(
        smart_context_apply_rewrite_budget_decision(
            rewrite_budget_policy(
                SmartContextTokenBudgetTier::Large,
                SmartContextBudgetMode::LargeLossless,
                7,
                40_000,
                8,
            ),
            SmartContextRewriteBudgetDecision::Relax,
            None,
        ),
        rewrite_budget_policy(
            SmartContextTokenBudgetTier::Large,
            SmartContextBudgetMode::LargeLossless,
            65_536,
            65_536,
            10,
        )
    );

    assert_eq!(
        smart_context_apply_rewrite_budget_decision(
            rewrite_budget_policy(
                SmartContextTokenBudgetTier::Condensed,
                SmartContextBudgetMode::ArtifactCondensed,
                7,
                301,
                3,
            ),
            SmartContextRewriteBudgetDecision::Relax,
            None,
        ),
        rewrite_budget_policy(
            SmartContextTokenBudgetTier::Condensed,
            SmartContextBudgetMode::ArtifactCondensed,
            377,
            377,
            4,
        )
    );

    assert_eq!(
        smart_context_apply_rewrite_budget_decision(
            rewrite_budget_policy(
                SmartContextTokenBudgetTier::Minimal,
                SmartContextBudgetMode::MinimalRefsOnly,
                7,
                300,
                2,
            ),
            SmartContextRewriteBudgetDecision::Tighten,
            Some(1),
        ),
        rewrite_budget_policy(
            SmartContextTokenBudgetTier::Minimal,
            SmartContextBudgetMode::MinimalRefsOnly,
            270,
            270,
            1,
        )
    );

    assert_eq!(
        smart_context_apply_rewrite_budget_decision(
            rewrite_budget_policy(
                SmartContextTokenBudgetTier::Large,
                SmartContextBudgetMode::LargeLossless,
                7,
                70_000,
                1,
            ),
            SmartContextRewriteBudgetDecision::Relax,
            None,
        ),
        rewrite_budget_policy(
            SmartContextTokenBudgetTier::Large,
            SmartContextBudgetMode::LargeLossless,
            70_000,
            70_000,
            2,
        )
    );
}

#[test]
fn rewrite_budget_adjustment_preserves_zero_max_and_saturation_edges() {
    for (inline, rehydrate) in [(0, u64::MAX), (usize::MAX, 0)] {
        assert_eq!(
            smart_context_apply_rewrite_budget_decision(
                rewrite_budget_policy(
                    SmartContextTokenBudgetTier::Minimal,
                    SmartContextBudgetMode::MinimalRefsOnly,
                    17,
                    inline,
                    rehydrate,
                ),
                SmartContextRewriteBudgetDecision::Relax,
                None,
            ),
            rewrite_budget_policy(
                SmartContextTokenBudgetTier::Minimal,
                SmartContextBudgetMode::MinimalRefsOnly,
                inline,
                inline,
                rehydrate,
            )
        );
    }

    assert_eq!(
        smart_context_apply_rewrite_budget_decision(
            rewrite_budget_policy(
                SmartContextTokenBudgetTier::Minimal,
                SmartContextBudgetMode::MinimalRefsOnly,
                17,
                256,
                1,
            ),
            SmartContextRewriteBudgetDecision::Tighten,
            None,
        ),
        rewrite_budget_policy(
            SmartContextTokenBudgetTier::Minimal,
            SmartContextBudgetMode::MinimalRefsOnly,
            256,
            256,
            1,
        )
    );

    let expected_relaxed_inline = match usize::BITS {
        32 => usize::MAX,
        64 => usize::MAX - 1,
        bits => panic!("unsupported usize width: {bits}"),
    };
    assert_eq!(
        smart_context_apply_rewrite_budget_decision(
            rewrite_budget_policy(
                SmartContextTokenBudgetTier::Condensed,
                SmartContextBudgetMode::ArtifactCondensed,
                17,
                usize::MAX - 1,
                u64::MAX - 1,
            ),
            SmartContextRewriteBudgetDecision::Relax,
            None,
        ),
        rewrite_budget_policy(
            SmartContextTokenBudgetTier::Condensed,
            SmartContextBudgetMode::ArtifactCondensed,
            expected_relaxed_inline,
            expected_relaxed_inline,
            u64::MAX - 1,
        )
    );

    let expected_tightened_inline = match usize::BITS {
        32 => 3_865_470_565usize,
        64 => 1_844_674_407_370_955_161u64 as usize,
        bits => panic!("unsupported usize width: {bits}"),
    };
    assert_eq!(
        smart_context_apply_rewrite_budget_decision(
            rewrite_budget_policy(
                SmartContextTokenBudgetTier::Minimal,
                SmartContextBudgetMode::MinimalRefsOnly,
                17,
                usize::MAX,
                u64::MAX,
            ),
            SmartContextRewriteBudgetDecision::Tighten,
            None,
        ),
        rewrite_budget_policy(
            SmartContextTokenBudgetTier::Minimal,
            SmartContextBudgetMode::MinimalRefsOnly,
            expected_tightened_inline,
            expected_tightened_inline,
            1_844_674_407_370_955_161,
        )
    );
}

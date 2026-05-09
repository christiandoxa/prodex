use super::*;

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

#[test]
fn learned_rewrite_policy_relaxes_for_matching_route_model_profile_samples() {
    let bucket = smart_context_test_rewrite_bucket("responses", "GPT_5", "alpha");
    let learned = smart_context_test_learned_rewrite_policy(
        Some(bucket.clone()),
        SmartContextRecentRewriteSafety::default(),
        vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                bucket.clone(),
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                bucket.clone(),
                smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                smart_context_test_rewrite_bucket("responses", "gpt-5", "beta"),
                SmartContextRewriteTelemetrySample {
                    fallback: true,
                    ..smart_context_test_rewrite_telemetry_sample(10_000, 9_000, 2_500, 2_400)
                },
            ),
        ],
    );

    assert_eq!(learned.decision, SmartContextRewriteBudgetDecision::Relax);
    assert_eq!(
        learned.reasons,
        vec![SmartContextLearnedRewritePolicyReason::LearnedRelax]
    );
    assert_eq!(learned.matching_telemetry_samples, 2);
    assert_eq!(learned.policy.mode, SmartContextBudgetMode::LargeLossless);
    assert_eq!(learned.policy.max_inline_tool_output_bytes, 64 * 1024);
}

#[test]
fn learned_rewrite_policy_tightens_for_confident_weak_matching_savings() {
    let bucket = smart_context_test_rewrite_bucket("responses", "gpt-5", "alpha");
    let learned = smart_context_test_learned_rewrite_policy(
        Some(bucket.clone()),
        SmartContextRecentRewriteSafety::default(),
        vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                bucket.clone(),
                smart_context_test_rewrite_telemetry_sample(10_000, 9_000, 2_500, 2_400),
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                bucket,
                smart_context_test_rewrite_telemetry_sample(8_000, 7_200, 2_000, 1_900),
            ),
        ],
    );

    assert_eq!(learned.decision, SmartContextRewriteBudgetDecision::Tighten);
    assert_eq!(
        learned.reasons,
        vec![SmartContextLearnedRewritePolicyReason::LearnedTighten]
    );
    assert_eq!(learned.policy.mode, SmartContextBudgetMode::LargeLossless);
    assert_eq!(
        learned.policy.max_inline_tool_output_bytes,
        (32 * 1024) * 9 / 10
    );
    assert!(learned.policy.max_rehydrate_tokens < 12_000);
}

#[test]
fn learned_rewrite_policy_exact_passes_through_without_bucket_confidence() {
    let learned = smart_context_test_learned_rewrite_policy(
        Some(smart_context_test_rewrite_bucket(
            "responses",
            "gpt-5",
            "alpha",
        )),
        SmartContextRecentRewriteSafety::default(),
        vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                smart_context_test_rewrite_bucket("responses", "gpt-5", "beta"),
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
            ),
            SmartContextBucketedRewriteTelemetrySample {
                bucket_key: None,
                sample: smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
            },
        ],
    );

    assert_eq!(
        learned.decision,
        SmartContextRewriteBudgetDecision::NoChange
    );
    assert_eq!(
        learned.reasons,
        vec![SmartContextLearnedRewritePolicyReason::MissingBucketConfidence]
    );
    assert_eq!(learned.matching_telemetry_samples, 0);
    assert_eq!(
        learned.policy.mode,
        SmartContextBudgetMode::ExactPassThrough
    );
    assert_eq!(learned.policy.max_inline_tool_output_bytes, usize::MAX);
}

#[test]
fn learned_rewrite_policy_exact_passes_through_on_unsafe_matching_sample() {
    let bucket = smart_context_test_rewrite_bucket("compact", "gpt-5.2", "alpha");
    let learned = smart_context_test_learned_rewrite_policy(
        Some(bucket.clone()),
        SmartContextRecentRewriteSafety::default(),
        vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                bucket,
                SmartContextRewriteTelemetrySample {
                    fallback: true,
                    ..smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000)
                },
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                smart_context_test_rewrite_bucket("compact", "gpt-5.2", "beta"),
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
            ),
        ],
    );

    assert_eq!(
        learned.reasons,
        vec![SmartContextLearnedRewritePolicyReason::UnsafeRewriteSample]
    );
    assert_eq!(learned.matching_telemetry_samples, 1);
    assert_eq!(
        learned.policy.mode,
        SmartContextBudgetMode::ExactPassThrough
    );
    assert_eq!(learned.policy.max_inline_bytes, usize::MAX);
}

#[test]
fn learned_rewrite_policy_can_use_recent_safety_when_bucket_samples_are_absent() {
    let learned = smart_context_test_learned_rewrite_policy(
        Some(smart_context_test_rewrite_bucket(
            "responses",
            "gpt-5",
            "alpha",
        )),
        SmartContextRecentRewriteSafety {
            safe_rewrites: 2,
            fallback_rewrites: 0,
            saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2,
        },
        Vec::new(),
    );

    assert_eq!(learned.decision, SmartContextRewriteBudgetDecision::Relax);
    assert_eq!(
        learned.reasons,
        vec![
            SmartContextLearnedRewritePolicyReason::RecentSafetyFallback,
            SmartContextLearnedRewritePolicyReason::LearnedRelax,
        ]
    );
    assert_eq!(learned.matching_telemetry_samples, 0);
    assert_eq!(learned.safety_samples, 2);
    assert_eq!(learned.policy.max_inline_tool_output_bytes, 64 * 1024);
}

#[test]
fn learned_rewrite_policy_keeps_base_exact_when_exactness_required() {
    let accounting = smart_context_test_large_budget_accounting();
    let learned = smart_context_learned_rewrite_policy(SmartContextLearnedRewritePolicyInput {
        adaptive_policy_input: SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput {
                previous_response_id: Some("resp-owned".to_string()),
                ..SmartContextExactnessInput::default()
            }),
            accounting,
            recent_rewrite_safety: SmartContextRecentRewriteSafety {
                safe_rewrites: 2,
                fallback_rewrites: 0,
                saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2,
            },
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        },
        bucket_key: Some(smart_context_test_rewrite_bucket(
            "responses",
            "gpt-5",
            "alpha",
        )),
        telemetry_samples: vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                smart_context_test_rewrite_bucket("responses", "gpt-5", "alpha"),
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                smart_context_test_rewrite_bucket("responses", "gpt-5", "alpha"),
                smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
            ),
        ],
    });

    assert_eq!(
        learned.reasons,
        vec![SmartContextLearnedRewritePolicyReason::BasePolicyExactPassThrough]
    );
    assert_eq!(
        learned.policy.mode,
        SmartContextBudgetMode::ExactPassThrough
    );
    assert_eq!(
        learned.policy.reasons,
        vec![SmartContextBudgetPolicyReason::ExactnessRequired]
    );
}

fn smart_context_test_learned_rewrite_policy(
    bucket_key: Option<SmartContextRewritePolicyBucketKey>,
    recent_rewrite_safety: SmartContextRecentRewriteSafety,
    telemetry_samples: Vec<SmartContextBucketedRewriteTelemetrySample>,
) -> SmartContextLearnedRewritePolicy {
    smart_context_learned_rewrite_policy(SmartContextLearnedRewritePolicyInput {
        adaptive_policy_input: SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: smart_context_test_large_budget_accounting(),
            recent_rewrite_safety,
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        },
        bucket_key,
        telemetry_samples,
    })
}

fn smart_context_test_large_budget_accounting() -> SmartContextObservedTokenAccounting {
    smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
        model_context_window_tokens: Some(64_000),
        reserved_output_tokens: 4_096,
        current_input_tokens: 48_000,
        current_request_body_bytes: 0,
        current_request_estimated_tokens: None,
        observed_usage: Vec::new(),
    })
}

fn smart_context_test_rewrite_bucket(
    route: &str,
    model: &str,
    profile: &str,
) -> SmartContextRewritePolicyBucketKey {
    SmartContextRewritePolicyBucketKey {
        route: Some(route.to_string()),
        model: Some(model.to_string()),
        profile: Some(profile.to_string()),
    }
}

fn smart_context_test_bucketed_rewrite_telemetry_sample(
    bucket_key: SmartContextRewritePolicyBucketKey,
    sample: SmartContextRewriteTelemetrySample,
) -> SmartContextBucketedRewriteTelemetrySample {
    SmartContextBucketedRewriteTelemetrySample {
        bucket_key: Some(bucket_key),
        sample,
    }
}

fn smart_context_test_transform_rewrite_telemetry_sample(
    category: SmartContextTransformCategory,
    sample: SmartContextRewriteTelemetrySample,
) -> SmartContextTransformRewriteTelemetrySample {
    SmartContextTransformRewriteTelemetrySample { category, sample }
}

fn smart_context_test_transform_score(
    scores: &[SmartContextTransformRewriteSafetyScore],
    category: SmartContextTransformCategory,
) -> &SmartContextTransformRewriteSafetyScore {
    scores
        .iter()
        .find(|score| score.category == category)
        .unwrap_or_else(|| panic!("missing transform score: {category:?}"))
}

fn smart_context_test_rewrite_telemetry_sample(
    body_bytes_before: usize,
    body_bytes_after: usize,
    estimated_tokens_before: u64,
    estimated_tokens_after: u64,
) -> SmartContextRewriteTelemetrySample {
    SmartContextRewriteTelemetrySample {
        body_bytes_before,
        body_bytes_after,
        estimated_tokens_before,
        estimated_tokens_after,
        safe: true,
        fallback: false,
    }
}

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

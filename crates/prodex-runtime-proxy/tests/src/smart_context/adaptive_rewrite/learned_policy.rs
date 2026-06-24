use super::super::*;
use super::{
    smart_context_test_bucketed_rewrite_telemetry_sample,
    smart_context_test_large_budget_accounting, smart_context_test_learned_rewrite_policy,
    smart_context_test_rewrite_bucket, smart_context_test_rewrite_telemetry_sample,
};

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
fn learned_rewrite_policy_isolates_extended_quality_buckets() {
    let mut target = smart_context_test_rewrite_bucket("responses", "gpt-5", "alpha");
    target.provider = Some("openai".to_string());
    target.context_window_band = Some("200k".to_string());
    target.session_length_band = Some("long".to_string());
    target.task_class = Some("refactor".to_string());
    target.transform_category = Some("semantic_rehydrate".to_string());

    let matching = target.clone();
    let mut wrong_window = target.clone();
    wrong_window.context_window_band = Some("32k".to_string());
    let mut wrong_transform = target.clone();
    wrong_transform.transform_category = Some("tool_output".to_string());

    let learned = smart_context_test_learned_rewrite_policy(
        Some(target),
        SmartContextRecentRewriteSafety::default(),
        vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                matching.clone(),
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                matching,
                smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                wrong_window,
                SmartContextRewriteTelemetrySample {
                    fallback: true,
                    ..smart_context_test_rewrite_telemetry_sample(10_000, 9_000, 2_500, 2_400)
                },
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                wrong_transform,
                SmartContextRewriteTelemetrySample {
                    fallback: true,
                    ..smart_context_test_rewrite_telemetry_sample(10_000, 9_000, 2_500, 2_400)
                },
            ),
        ],
    );

    assert_eq!(learned.decision, SmartContextRewriteBudgetDecision::Relax);
    assert_eq!(learned.matching_telemetry_samples, 2);
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

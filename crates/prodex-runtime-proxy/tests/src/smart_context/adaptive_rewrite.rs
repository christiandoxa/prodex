use super::*;

#[path = "adaptive_rewrite/learned_policy.rs"]
mod learned_policy;

#[path = "adaptive_rewrite/policy.rs"]
mod policy;

#[path = "adaptive_rewrite/regression.rs"]
mod regression;

#[path = "adaptive_rewrite/telemetry.rs"]
mod telemetry;

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
        ..SmartContextRewritePolicyBucketKey::default()
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
        upstream_context_errors: 0,
        previous_response_not_found: false,
        invalid_tool_call_continuation: false,
        missing_artifact_requests: 0,
        repeated_tool_call_count: 0,
        model_reread_requests: 0,
        corrective_user_messages: 0,
        test_or_build_failed_after_rewrite: false,
        task_completed: None,
        additional_turns_before_task_completion: None,
        final_total_input_tokens: None,
    }
}

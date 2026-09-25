use super::*;

#[path = "adaptive_rewrite/policy.rs"]
#[cfg(feature = "mojo")]
mod policy;

#[path = "adaptive_rewrite/regression.rs"]
mod regression;

#[path = "adaptive_rewrite/telemetry.rs"]
mod telemetry;

fn smart_context_test_rewrite_telemetry_sample(
    body_bytes_before: usize,
    body_bytes_after: usize,
    tokens_before: u64,
    tokens_after: u64,
) -> SmartContextRewriteTelemetrySample {
    SmartContextRewriteTelemetrySample {
        body_bytes_before,
        body_bytes_after,
        tokens_before,
        tokens_after,
        token_count_source: SmartContextTokenCountSource::TokenizerCounted,
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

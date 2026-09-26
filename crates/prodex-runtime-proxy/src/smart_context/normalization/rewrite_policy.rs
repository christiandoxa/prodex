use super::*;

pub(in crate::smart_context) fn smart_context_rewrite_telemetry_sample_quality_risk(
    sample: &SmartContextRewriteTelemetrySample,
) -> bool {
    sample.upstream_context_errors > 0
        || sample.previous_response_not_found
        || sample.invalid_tool_call_continuation
        || sample.missing_artifact_requests > 0
        || sample.repeated_tool_call_count > 0
        || sample.model_reread_requests > 0
        || sample.corrective_user_messages > 0
        || sample.test_or_build_failed_after_rewrite
        || sample.task_completed == Some(false)
}

//! Gemini response status and finish-reason bridge helpers.

use crate::translators::{
    gemini_finish_reason, gemini_finish_reason_failure, gemini_finish_reason_incomplete,
    gemini_prompt_feedback_failure,
};

pub fn gemini_provider_core_prompt_feedback_failure(
    value: &serde_json::Value,
) -> Option<(String, String)> {
    gemini_prompt_feedback_failure(value)
}

pub fn gemini_provider_core_finish_reason(value: &serde_json::Value) -> Option<String> {
    gemini_finish_reason(value)
}

pub fn gemini_provider_core_finish_reason_failure(reason: &str) -> Option<(String, String)> {
    gemini_finish_reason_failure(reason)
}

pub fn gemini_provider_core_finish_reason_incomplete(reason: &str) -> Option<(String, String)> {
    gemini_finish_reason_incomplete(reason)
}

pub fn gemini_provider_core_finish_reason_retryable_invalid(reason: &str) -> bool {
    matches!(
        reason,
        "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL" | "OTHER"
    )
}

pub fn gemini_provider_core_response_terminal_without_history(
    response: &serde_json::Value,
) -> bool {
    matches!(
        response.get("status").and_then(serde_json::Value::as_str),
        Some("failed" | "incomplete")
    ) || response.get("error").is_some()
}

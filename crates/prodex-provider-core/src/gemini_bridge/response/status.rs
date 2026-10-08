//! Gemini response status and finish-reason bridge helpers.

use prodex_mojo_core::provider_constraints::{GeminiFailureOperation, gemini_failure_policy};

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
    gemini_failure_policy(
        GeminiFailureOperation::RetryableFinishReason,
        Some(reason),
        false,
    )
    .expect("Mojo Gemini retryable-finish policy returned invalid output")
}

pub fn gemini_provider_core_response_terminal_without_history(
    response: &serde_json::Value,
) -> bool {
    gemini_failure_policy(
        GeminiFailureOperation::TerminalWithoutHistory,
        response.get("status").and_then(serde_json::Value::as_str),
        response.get("error").is_some(),
    )
    .expect("Mojo Gemini terminal history policy returned invalid output")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn gemini_precommit_mojo_retryable_finish_reason_preserves_exact_match() {
        for (value, retry) in [
            ("MALFORMED_FUNCTION_CALL", true),
            ("UNEXPECTED_TOOL_CALL", true),
            ("OTHER", true),
            ("STOP", false),
            ("SAFETY", false),
            ("other", false),
            (" OTHER ", false),
            ("", false),
            ("異常", false),
        ] {
            assert_eq!(
                gemini_provider_core_finish_reason_retryable_invalid(value),
                retry,
                "reason: {value:?}"
            );
        }
    }

    #[test]
    fn gemini_runtime_mojo_terminal_history_preserves_status_and_error_presence() {
        for (input, expected) in [
            (json!({}), false),
            (json!({"status": "failed"}), true),
            (json!({"status": "incomplete"}), true),
            (json!({"status": "completed"}), false),
            (json!({"status": "FAILED"}), false),
            (json!({"status": 7}), false),
            (json!({"status": null}), false),
            (json!({"error": null}), true),
            (json!({"status": "completed", "error": false}), true),
            (json!({"error": {}, "status": "unexpected"}), true),
            (json!({"output": "x".repeat(5 * 1024 * 1024)}), false),
        ] {
            assert_eq!(
                gemini_provider_core_response_terminal_without_history(&input),
                expected,
                "response: {:?}",
                input.get("status")
            );
        }
    }
}

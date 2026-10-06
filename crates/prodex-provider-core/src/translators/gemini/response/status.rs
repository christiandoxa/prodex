//! Gemini finish reason and response status normalization.

use serde_json::Value;

pub(crate) fn gemini_prompt_feedback_failure(value: &Value) -> Option<(String, String)> {
    let feedback = value.get("promptFeedback")?;
    let reason = feedback
        .get("blockReason")
        .and_then(Value::as_str)
        .filter(|reason| !reason.trim().is_empty())?;
    gemini_status_pair(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::PromptFeedbackFailure,
        reason,
    )
}

pub(crate) fn gemini_finish_reason(value: &Value) -> Option<String> {
    let raw = serde_json::to_string(value).expect("Gemini response serializes");
    let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::RawFinishReason,
    );
    input.response = Some(&raw);
    super::super::stream::gemini_mojo_value(input)
        .as_str()
        .map(str::to_string)
}

pub(crate) fn gemini_finish_reason_failure(reason: &str) -> Option<(String, String)> {
    gemini_status_pair(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::FinishReasonFailure,
        reason,
    )
}

pub(crate) fn gemini_finish_reason_incomplete(reason: &str) -> Option<(String, String)> {
    gemini_status_pair(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::FinishReasonIncomplete,
        reason,
    )
}

fn gemini_status_pair(
    operation: prodex_mojo_core::rich::GeminiResponseKernelOperation,
    reason: &str,
) -> Option<(String, String)> {
    let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(operation);
    input.reason = Some(reason);
    let value = super::super::stream::gemini_mojo_value(input);
    let pair = value.as_array()?;
    Some((
        pair.first()?.as_str()?.to_string(),
        pair.get(1)?.as_str()?.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finish_reasons_have_expected_status_mappings() {
        for (reason, code) in [
            ("MALFORMED_FUNCTION_CALL", "gemini_malformed_function_call"),
            ("UNEXPECTED_TOOL_CALL", "gemini_unexpected_tool_call"),
            ("OTHER", "gemini_finish_other"),
            ("NO_IMAGE", "gemini_no_image"),
            ("SAFETY", "invalid_prompt"),
            ("RECITATION", "invalid_prompt"),
            ("LANGUAGE", "invalid_prompt"),
            ("BLOCKLIST", "invalid_prompt"),
            ("PROHIBITED_CONTENT", "invalid_prompt"),
            ("SPII", "invalid_prompt"),
            ("IMAGE_SAFETY", "invalid_prompt"),
            ("IMAGE_PROHIBITED_CONTENT", "invalid_prompt"),
        ] {
            assert_eq!(
                gemini_finish_reason_failure(reason),
                Some((
                    code.to_string(),
                    format!("Gemini ended the stream with finishReason={reason}"),
                ))
            );
        }
        assert_eq!(
            gemini_finish_reason_incomplete("MAX_TOKENS"),
            Some((
                "max_output_tokens".to_string(),
                "Gemini stopped because it reached the maximum output token limit.".to_string(),
            ))
        );
        for reason in ["MAX_TOKENS", "STOP", "UNKNOWN", "安全🙂", "", "  "] {
            assert_eq!(gemini_finish_reason_failure(reason), None);
        }
        for reason in ["STOP", "UNKNOWN", "安全🙂", "", "  "] {
            assert_eq!(gemini_finish_reason_incomplete(reason), None);
        }
    }

    #[test]
    fn prompt_feedback_mapping_preserves_unicode_and_block_precedence() {
        let reason = "地域ポリシー 🚫";
        let value = json!({
            "promptFeedback": {"blockReason": reason},
            "candidates": [{"finishReason": "STOP"}],
        });
        assert_eq!(
            gemini_prompt_feedback_failure(&value),
            Some((
                "gemini_prompt_blocked".to_string(),
                format!("Gemini blocked the prompt: {reason}"),
            ))
        );
        assert_eq!(
            gemini_prompt_feedback_failure(&json!({
                "promptFeedback": {"blockReason": " "},
            })),
            None
        );
        assert_eq!(
            gemini_prompt_feedback_failure(&json!({
                "promptFeedback": {"blockReason": "  "},
            })),
            None
        );
    }
}

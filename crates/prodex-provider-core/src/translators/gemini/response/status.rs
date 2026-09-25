//! Gemini finish reason and response status normalization.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GeminiResponseStatus {
    Failed { code: String, message: String },
    Incomplete { reason: String, message: String },
}

pub(super) fn gemini_response_status(
    value: &Value,
    has_visible_output: bool,
) -> Option<GeminiResponseStatus> {
    if let Some((code, message)) = gemini_prompt_feedback_failure(value) {
        return Some(GeminiResponseStatus::Failed { code, message });
    }
    if let Some(reason) = gemini_finish_reason(value) {
        if let Some((reason, message)) = gemini_finish_reason_incomplete(&reason) {
            return Some(GeminiResponseStatus::Incomplete { reason, message });
        }
        if let Some((code, message)) = gemini_finish_reason_failure(&reason) {
            return Some(GeminiResponseStatus::Failed { code, message });
        }
    }
    if !has_visible_output {
        let suffix = gemini_finish_reason(value)
            .map(|reason| format!(" finishReason={reason}"))
            .unwrap_or_default();
        return Some(GeminiResponseStatus::Failed {
            code: "gemini_empty_response".to_string(),
            message: format!("Gemini returned no visible response content.{suffix}"),
        });
    }
    None
}

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
    value
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("finishReason"))
        .and_then(Value::as_str)
        .filter(|reason| !reason.trim().is_empty())
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

    #[test]
    fn empty_and_unknown_finish_reasons_keep_response_status_behavior() {
        assert_eq!(
            gemini_response_status(
                &json!({
                    "candidates": [{"finishReason": "NOT_RECOGNIZED"}],
                }),
                false
            ),
            Some(GeminiResponseStatus::Failed {
                code: "gemini_empty_response".to_string(),
                message: "Gemini returned no visible response content. finishReason=NOT_RECOGNIZED"
                    .to_string(),
            })
        );
        assert_eq!(
            gemini_response_status(
                &json!({
                    "candidates": [{"finishReason": " "}],
                }),
                false
            ),
            Some(GeminiResponseStatus::Failed {
                code: "gemini_empty_response".to_string(),
                message: "Gemini returned no visible response content.".to_string(),
            })
        );
        assert_eq!(
            gemini_response_status(
                &json!({
                    "candidates": [{"finishReason": "NOT_RECOGNIZED"}],
                }),
                true
            ),
            None
        );
    }
}

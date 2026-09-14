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
    #[cfg(feature = "mojo")]
    return gemini_status_pair(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::PromptFeedbackFailure,
        reason,
    );
    #[cfg(not(feature = "mojo"))]
    gemini_prompt_feedback_failure_oracle(reason)
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
    #[cfg(feature = "mojo")]
    return gemini_status_pair(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::FinishReasonFailure,
        reason,
    );
    #[cfg(not(feature = "mojo"))]
    gemini_finish_reason_failure_oracle(reason)
}

#[cfg(any(not(feature = "mojo"), test))]
fn gemini_finish_reason_failure_oracle(reason: &str) -> Option<(String, String)> {
    let code = match reason {
        "MALFORMED_FUNCTION_CALL" => "gemini_malformed_function_call",
        "UNEXPECTED_TOOL_CALL" => "gemini_unexpected_tool_call",
        "OTHER" => "gemini_finish_other",
        "NO_IMAGE" => "gemini_no_image",
        "SAFETY"
        | "RECITATION"
        | "LANGUAGE"
        | "BLOCKLIST"
        | "PROHIBITED_CONTENT"
        | "SPII"
        | "IMAGE_SAFETY"
        | "IMAGE_PROHIBITED_CONTENT" => "invalid_prompt",
        _ => return None,
    };
    Some((
        code.to_string(),
        format!("Gemini ended the stream with finishReason={reason}"),
    ))
}

pub(crate) fn gemini_finish_reason_incomplete(reason: &str) -> Option<(String, String)> {
    #[cfg(feature = "mojo")]
    return gemini_status_pair(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::FinishReasonIncomplete,
        reason,
    );
    #[cfg(not(feature = "mojo"))]
    gemini_finish_reason_incomplete_oracle(reason)
}

#[cfg(any(not(feature = "mojo"), test))]
fn gemini_finish_reason_incomplete_oracle(reason: &str) -> Option<(String, String)> {
    match reason {
        "MAX_TOKENS" => Some((
            "max_output_tokens".to_string(),
            "Gemini stopped because it reached the maximum output token limit.".to_string(),
        )),
        _ => None,
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn gemini_prompt_feedback_failure_oracle(reason: &str) -> Option<(String, String)> {
    Some((
        "gemini_prompt_blocked".to_string(),
        format!("Gemini blocked the prompt: {reason}"),
    ))
}

#[cfg(feature = "mojo")]
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

#[cfg(all(test, feature = "mojo"))]
mod mojo_parity_tests {
    use super::*;

    #[test]
    fn finish_reason_mapping_matches_rust_oracle() {
        for reason in [
            "MAX_TOKENS",
            "MALFORMED_FUNCTION_CALL",
            "UNEXPECTED_TOOL_CALL",
            "OTHER",
            "NO_IMAGE",
            "SAFETY",
            "RECITATION",
            "LANGUAGE",
            "BLOCKLIST",
            "PROHIBITED_CONTENT",
            "SPII",
            "IMAGE_SAFETY",
            "IMAGE_PROHIBITED_CONTENT",
            "STOP",
            "安全🙂",
            "",
        ] {
            assert_eq!(
                gemini_finish_reason_failure(reason),
                gemini_finish_reason_failure_oracle(reason)
            );
            assert_eq!(
                gemini_finish_reason_incomplete(reason),
                gemini_finish_reason_incomplete_oracle(reason)
            );
        }
        for reason in ["blocked", "安全🙂", ""] {
            assert_eq!(
                gemini_status_pair(
                    prodex_mojo_core::rich::GeminiResponseKernelOperation::PromptFeedbackFailure,
                    reason,
                ),
                gemini_prompt_feedback_failure_oracle(reason),
            );
        }
    }
}

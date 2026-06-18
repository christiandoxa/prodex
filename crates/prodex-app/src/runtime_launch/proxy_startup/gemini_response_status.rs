#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super::super) enum RuntimeGeminiResponseStatus {
    Failed { code: String, message: String },
    Incomplete { reason: String, message: String },
}

pub(in super::super::super) fn runtime_gemini_response_status(
    value: &serde_json::Value,
    has_visible_output: bool,
) -> Option<RuntimeGeminiResponseStatus> {
    if let Some(failure) = runtime_gemini_prompt_feedback_failure(value) {
        let (code, message) = failure;
        return Some(RuntimeGeminiResponseStatus::Failed { code, message });
    }
    if let Some(reason) = runtime_gemini_finish_reason(value) {
        if let Some((reason, message)) = runtime_gemini_finish_reason_incomplete(&reason) {
            return Some(RuntimeGeminiResponseStatus::Incomplete { reason, message });
        }
        if let Some(failure) = runtime_gemini_finish_reason_failure(&reason) {
            let (code, message) = failure;
            return Some(RuntimeGeminiResponseStatus::Failed { code, message });
        }
    }
    if !has_visible_output {
        let suffix = runtime_gemini_finish_reason(value)
            .map(|reason| format!(" finishReason={reason}"))
            .unwrap_or_default();
        return Some(RuntimeGeminiResponseStatus::Failed {
            code: "gemini_empty_response".to_string(),
            message: format!("Gemini returned no visible response content.{suffix}"),
        });
    }
    None
}

pub(in super::super::super) fn runtime_gemini_prompt_feedback_failure(
    value: &serde_json::Value,
) -> Option<(String, String)> {
    let feedback = value.get("promptFeedback")?;
    let reason = feedback
        .get("blockReason")
        .and_then(serde_json::Value::as_str)
        .filter(|reason| !reason.trim().is_empty())?;
    Some((
        "gemini_prompt_blocked".to_string(),
        format!("Gemini blocked the prompt: {reason}"),
    ))
}

pub(in super::super::super) fn runtime_gemini_finish_reason(
    value: &serde_json::Value,
) -> Option<String> {
    value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("finishReason"))
        .and_then(serde_json::Value::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .map(str::to_string)
}

pub(in super::super::super) fn runtime_gemini_finish_reason_failure(
    reason: &str,
) -> Option<(String, String)> {
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

pub(in super::super::super) fn runtime_gemini_finish_reason_incomplete(
    reason: &str,
) -> Option<(String, String)> {
    match reason {
        "MAX_TOKENS" => Some((
            "max_output_tokens".to_string(),
            "Gemini stopped because it reached the maximum output token limit.".to_string(),
        )),
        _ => None,
    }
}

pub(in super::super::super) fn runtime_gemini_finish_reason_retryable_invalid(
    reason: &str,
) -> bool {
    matches!(
        reason,
        "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL" | "OTHER"
    )
}

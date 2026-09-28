//! Gemini tool-output and assistant-text guardrails.

mod exact_output;
mod intent;
mod tool_text;

pub use self::exact_output::{
    gemini_provider_core_conversation_requests_command_output_only,
    gemini_provider_core_forced_command_output,
};
pub use self::intent::gemini_provider_core_tool_intent_without_call;
use self::tool_text::gemini_provider_core_tool_texts_since_latest_user;
use prodex_mojo_core::gemini_guardrails::{
    GeminiWaitOrPollReason, gemini_process_exited_zero, gemini_success_claim,
    gemini_tool_text_has_failure, gemini_verification_marker, gemini_wait_or_poll_reason,
};

pub fn gemini_provider_core_non_actionable_wait_or_poll_text(text: &str) -> Option<&'static str> {
    match gemini_wait_or_poll_reason(text)
        .expect("Mojo Gemini wait/poll guardrail should accept Rust strings")
    {
        Some(GeminiWaitOrPollReason::IWillPoll) => Some("i will poll"),
        Some(GeminiWaitOrPollReason::IllPoll) => Some("i'll poll"),
        Some(GeminiWaitOrPollReason::INeedToWait) => Some("i need to wait"),
        Some(GeminiWaitOrPollReason::LetsWait) => Some("let's wait"),
        Some(GeminiWaitOrPollReason::StillRunning) => Some("still running"),
        Some(GeminiWaitOrPollReason::IsStillRunning) => Some("is still running"),
        Some(GeminiWaitOrPollReason::IWillWait) => Some("i will wait"),
        Some(GeminiWaitOrPollReason::IllWait) => Some("i'll wait"),
        None => None,
    }
}

pub fn gemini_provider_core_unverified_success_claim(
    text: &str,
    conversation_messages: &[serde_json::Value],
) -> bool {
    if !gemini_success_claim(text)
        .expect("Mojo Gemini success-claim guardrail should accept Rust strings")
    {
        return false;
    }
    let tool_texts = gemini_provider_core_tool_texts_since_latest_user(conversation_messages);
    if tool_texts.is_empty() {
        return true;
    }
    let last_tool = tool_texts.last().map(|text| text.as_str()).unwrap_or("");
    if gemini_tool_text_has_failure(last_tool)
        .expect("Mojo Gemini tool-failure guardrail should accept Rust strings")
    {
        return true;
    }
    let has_verification_marker = gemini_verification_marker(last_tool)
        .expect("Mojo Gemini verification guardrail should accept Rust strings");
    let clean_final_verification = has_verification_marker
        && !tool_texts
            .iter()
            .skip(tool_texts.len().saturating_sub(2))
            .any(|tool| {
                gemini_tool_text_has_failure(tool)
                    .expect("Mojo Gemini tool-failure guardrail should accept Rust strings")
                    && !gemini_process_exited_zero(tool)
                        .expect("Mojo Gemini process-exit guardrail should accept Rust strings")
            });
    !clean_final_verification
}

pub fn gemini_provider_core_blocked_tool_call_item(message: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": message,
        }],
    })
}

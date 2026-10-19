//! Gemini tool-output and assistant-text guardrails.

mod exact_output;
mod intent;
mod tool_text;

pub use self::exact_output::{
    gemini_provider_core_conversation_requests_command_output_only,
    gemini_provider_core_forced_command_output,
};
pub use self::intent::gemini_provider_core_tool_intent_without_call;
use self::tool_text::gemini_provider_core_guardrail_projection;
use prodex_mojo_core::gemini_guardrails::{
    GeminiGuardrailJsonOperation, GeminiWaitOrPollReason, gemini_success_claim,
    gemini_wait_or_poll_reason,
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
    // Ordinary output avoids acquiring the conversation JSON after this Mojo gate.
    if !gemini_success_claim(text)
        .expect("Mojo Gemini success-claim guardrail should accept Rust strings")
    {
        return false;
    }
    gemini_provider_core_guardrail_projection(
        &serde_json::json!({"text": text, "messages": conversation_messages}),
        GeminiGuardrailJsonOperation::UnverifiedSuccessClaim,
    )
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

#[cfg(test)]
mod tests;

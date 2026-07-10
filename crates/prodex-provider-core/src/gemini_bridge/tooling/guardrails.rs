//! Gemini tool-output and assistant-text guardrails.

mod exact_output;
mod intent;
mod tool_text;

pub use self::exact_output::{
    gemini_provider_core_conversation_requests_command_output_only,
    gemini_provider_core_forced_command_output,
};
pub use self::intent::gemini_provider_core_tool_intent_without_call;
use self::tool_text::{
    gemini_provider_core_tool_text_has_failure, gemini_provider_core_tool_text_has_version_lines,
    gemini_provider_core_tool_texts_since_latest_user,
};

pub fn gemini_provider_core_non_actionable_wait_or_poll_text(text: &str) -> Option<&'static str> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.len() < 8 {
        return None;
    }
    [
        ("i will poll", "i will poll"),
        ("i'll poll", "i'll poll"),
        ("i need to wait", "i need to wait"),
        ("let's wait", "let's wait"),
        ("still running", "still running"),
        ("is still running", "is still running"),
        ("i will wait", "i will wait"),
        ("i'll wait", "i'll wait"),
    ]
    .into_iter()
    .find_map(|(needle, reason)| lower.contains(needle).then_some(reason))
}

pub fn gemini_provider_core_unverified_success_claim(
    text: &str,
    conversation_messages: &[serde_json::Value],
) -> bool {
    let lower = text.to_ascii_lowercase();
    let claims_success = [
        "blocker/unresolved: none",
        "blockers/unresolved: none",
        "unresolved: none",
        "everything is complete",
        "semua optional tools berhasil",
        "berhasil diupdate",
        "successfully updated all",
        "all optional tools",
        "up-to-date",
        "latest version",
        "latest versions",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    if !claims_success {
        return false;
    }
    let tool_texts = gemini_provider_core_tool_texts_since_latest_user(conversation_messages);
    if tool_texts.is_empty() {
        return true;
    }
    let last_tool = tool_texts.last().map(|text| text.as_str()).unwrap_or("");
    let last_tool_lower = last_tool.to_ascii_lowercase();
    if gemini_provider_core_tool_text_has_failure(last_tool) {
        return true;
    }
    let has_verification_marker = [
        "--version",
        "version:",
        "verification:",
        "already up to date",
        "up-to-date",
    ]
    .iter()
    .any(|needle| last_tool_lower.contains(needle))
        || gemini_provider_core_tool_text_has_version_lines(last_tool);
    let clean_final_verification = has_verification_marker
        && !tool_texts
            .iter()
            .skip(tool_texts.len().saturating_sub(2))
            .any(|tool| {
                gemini_provider_core_tool_text_has_failure(tool)
                    && !tool
                        .to_ascii_lowercase()
                        .contains("process exited with code 0")
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

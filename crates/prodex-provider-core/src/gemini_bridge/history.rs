//! Gemini history/session bridge helpers.

mod session_import;

pub use self::session_import::{
    gemini_provider_core_import_contents_from_value, gemini_provider_core_session_checkpoint_value,
    gemini_provider_core_truncate_to_bytes,
};

use crate::translators::{
    gemini_contextual_user_instruction_text, gemini_is_contextual_user_fragment,
};

pub fn gemini_provider_core_chat_message_text(message: &serde_json::Value) -> Option<String> {
    match message.get("content") {
        Some(serde_json::Value::String(text)) => Some(text.clone()),
        Some(serde_json::Value::Array(items)) => Some(
            items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(serde_json::Value::as_str)
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => None,
    }
}

pub fn gemini_provider_core_system_instruction_from_chat(
    chat: &serde_json::Value,
    original: &serde_json::Value,
    codex_parity_instruction: &str,
    tool_discipline_instruction: &str,
    memory: Option<&str>,
    policy: Option<&str>,
) -> Option<serde_json::Value> {
    let messages = chat.get("messages")?.as_array()?;
    let mut system_text = messages
        .iter()
        .filter(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("system"))
        .filter_map(gemini_provider_core_chat_message_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    let contextual_user_text = messages
        .iter()
        .filter_map(gemini_provider_core_contextual_user_instruction_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    if !contextual_user_text.is_empty() {
        if !system_text.is_empty() {
            system_text.push_str("\n\n");
        }
        system_text.push_str(&contextual_user_text);
    }

    if original
        .get("prodex_gemini_compaction")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return (!system_text.trim().is_empty())
            .then(|| serde_json::json!({ "parts": [{ "text": system_text }] }));
    }

    if !system_text.is_empty() {
        system_text.push_str("\n\n");
        system_text.push_str(codex_parity_instruction);
    } else {
        system_text = codex_parity_instruction.to_string();
    }
    system_text.push_str("\n\n");
    system_text.push_str(tool_discipline_instruction);
    if let Some(memory) = memory {
        system_text.push_str("\n\n# Gemini CLI Memory Compatibility\n");
        system_text.push_str(memory);
    }
    if let Some(policy) = policy {
        system_text.push_str("\n\n# Gemini CLI Policy Compatibility\n");
        system_text.push_str(policy);
    }

    (!system_text.trim().is_empty())
        .then(|| serde_json::json!({ "parts": [{ "text": system_text }] }))
}

pub fn gemini_provider_core_contextual_user_instruction_text(
    message: &serde_json::Value,
) -> Option<String> {
    gemini_contextual_user_instruction_text(message)
}

pub fn gemini_provider_core_is_contextual_user_fragment(text: &str) -> bool {
    gemini_is_contextual_user_fragment(text)
}

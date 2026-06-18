use super::super::gemini_request_policy::RuntimeGeminiPolicyCompat;
use super::{
    PRODEX_GEMINI_CODEX_PARITY_INSTRUCTION, PRODEX_GEMINI_TOOL_DISCIPLINE_INSTRUCTION,
    chat_message_text, runtime_gemini_hierarchical_memory,
};

pub(super) fn runtime_gemini_system_instruction(
    chat: &serde_json::Value,
    original: &serde_json::Value,
) -> Option<serde_json::Value> {
    let messages = chat.get("messages")?.as_array()?;
    let mut system_text = messages
        .iter()
        .filter(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("system"))
        .filter_map(chat_message_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    let contextual_user_text = messages
        .iter()
        .filter_map(runtime_gemini_contextual_user_instruction_text)
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
        system_text.push_str(PRODEX_GEMINI_CODEX_PARITY_INSTRUCTION);
    } else {
        system_text = PRODEX_GEMINI_CODEX_PARITY_INSTRUCTION.to_string();
    }
    system_text.push_str("\n\n");
    system_text.push_str(PRODEX_GEMINI_TOOL_DISCIPLINE_INSTRUCTION);
    if let Some(memory) = runtime_gemini_hierarchical_memory(original) {
        system_text.push_str("\n\n# Gemini CLI Memory Compatibility\n");
        system_text.push_str(&memory);
    }
    if let Some(policy) = RuntimeGeminiPolicyCompat::from_request_and_files(original).summary() {
        system_text.push_str("\n\n# Gemini CLI Policy Compatibility\n");
        system_text.push_str(&policy);
    }

    (!system_text.trim().is_empty())
        .then(|| serde_json::json!({ "parts": [{ "text": system_text }] }))
}

pub(super) fn runtime_gemini_contextual_user_instruction_text(
    message: &serde_json::Value,
) -> Option<String> {
    if message
        .get("role")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|role| role != "user")
    {
        return None;
    }
    let text = chat_message_text(message)?;
    if runtime_gemini_is_contextual_user_fragment(&text)
        || text
            .split("\n\n")
            .filter(|fragment| !fragment.trim().is_empty())
            .all(runtime_gemini_is_contextual_user_fragment)
    {
        Some(text)
    } else {
        None
    }
}

fn runtime_gemini_is_contextual_user_fragment(text: &str) -> bool {
    let trimmed = text.trim_start();
    [
        "# AGENTS.md instructions for ",
        "<environment_context>",
        "<permissions instructions>",
        "<collaboration_mode>",
        "<skills_instructions>",
        "<plugins_instructions>",
        "<model_switch>",
        "<personality_spec>",
        "<realtime_conversation>",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
}

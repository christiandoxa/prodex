//! Text extraction and contextual-user instruction detection for Gemini request contents.

use serde_json::Value;

pub(super) fn gemini_message_text(message: &Value) -> Option<String> {
    match message.get("content") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(Value::as_str)
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => message
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

pub(crate) fn gemini_contextual_user_instruction_text(message: &Value) -> Option<String> {
    if message
        .get("role")
        .and_then(Value::as_str)
        .is_some_and(|role| role != "user")
    {
        return None;
    }
    let text = gemini_message_text(message)?;
    if gemini_is_contextual_user_fragment(&text)
        || text
            .split("\n\n")
            .filter(|fragment| !fragment.trim().is_empty())
            .all(gemini_is_contextual_user_fragment)
    {
        Some(text)
    } else {
        None
    }
}

pub(crate) fn gemini_is_contextual_user_fragment(text: &str) -> bool {
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

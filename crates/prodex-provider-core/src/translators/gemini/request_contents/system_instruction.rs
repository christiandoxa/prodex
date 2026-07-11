//! Gemini systemInstruction shaping from Responses input.

use serde_json::{Value, json};

use super::text::{gemini_contextual_user_instruction_text, gemini_message_text};

pub(crate) fn gemini_system_instruction_from_request(value: &Value) -> Option<Value> {
    let items = value.get("input")?.as_array()?;
    let mut system_text = items
        .iter()
        .filter(|item| item.get("role").and_then(Value::as_str) == Some("system"))
        .filter_map(gemini_message_text)
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let contextual_user_text = items
        .iter()
        .filter_map(gemini_contextual_user_instruction_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    if !contextual_user_text.is_empty() {
        if !system_text.is_empty() {
            system_text.push_str("\n\n");
        }
        system_text.push_str(&contextual_user_text);
    }
    (!system_text.trim().is_empty()).then(|| json!({ "parts": [{ "text": system_text }] }))
}

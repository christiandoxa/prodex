//! Gemini systemInstruction shaping from Responses input.

use serde_json::Value;
#[cfg(not(feature = "mojo"))]
use serde_json::json;

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
    if system_text.trim().is_empty() {
        return None;
    }
    #[cfg(feature = "mojo")]
    {
        let text = serde_json::to_vec(&system_text).expect("Gemini system instruction serializes");
        Some(super::gemini_request_content_mojo_value(
            prodex_mojo_core::provider_constraints::GeminiRequestContentOperation::SystemInstruction,
            Some(&text),
            None,
            None,
            None,
            0,
        ))
    }
    #[cfg(not(feature = "mojo"))]
    Some(json!({ "parts": [{ "text": system_text }] }))
}

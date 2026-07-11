//! Text extraction from Kiro chat-compatible message content.

use serde_json::Value;

pub(super) fn kiro_provider_core_chat_message_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.trim().is_empty()).then(|| text.to_string()),
        Value::Array(items) => {
            let mut text = String::new();
            for item in items {
                if let Some(chunk) = item
                    .get("text")
                    .and_then(Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(chunk);
                } else if let Some(chunk) = kiro_provider_core_chat_message_text(item) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&chunk);
                }
            }
            (!text.trim().is_empty()).then_some(text)
        }
        Value::Object(object) => object
            .get("text")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(str::to_string),
        _ => None,
    }
}

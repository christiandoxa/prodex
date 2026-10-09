use crate::mojo_json::Document;
use prodex_mojo_core::json::{AnthropicChatRequestTransform, transform_anthropic_chat_request};
use serde_json::Value;

const ANTHROPIC_CHAT_REQUEST_MAX_BYTES: usize = 4 * 1024 * 1024;

pub(super) fn transform(chat: &Value) -> Result<AnthropicChatRequestTransform, String> {
    let mut document = Document::default();
    document.push(chat, None, "");
    if document.raw.len() > ANTHROPIC_CHAT_REQUEST_MAX_BYTES {
        return Err(format!(
            "Anthropic Messages request exceeds the {} byte limit",
            ANTHROPIC_CHAT_REQUEST_MAX_BYTES
        ));
    }
    let raw = std::str::from_utf8(&document.raw)
        .map_err(|error| format!("Anthropic Messages request JSON is not UTF-8: {error}"))?;
    transform_anthropic_chat_request(&document.nodes, raw)
        .map_err(|error| format!("Anthropic Messages request kernel failed: {error:?}"))
}

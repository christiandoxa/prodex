use crate::mojo_json::Document;
use prodex_mojo_core::json::{AnthropicChatRequestTransform, transform_anthropic_chat_request};
use serde_json::Value;

pub(super) fn transform(chat: &Value) -> AnthropicChatRequestTransform {
    let mut document = Document::default();
    document.push(chat, None, "");
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    transform_anthropic_chat_request(&document.nodes, raw)
        .expect("Mojo Anthropic chat request transform returned invalid output")
}

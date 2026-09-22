use crate::ProviderId;
use crate::mojo_json::Document;
use prodex_mojo_core::json::{OpenAiChatRequestTransform, transform_openai_chat_request};
use serde_json::Value;

pub(super) fn transform(
    provider: ProviderId,
    value: &Value,
    input_model: Option<&str>,
    default_model: &str,
) -> Result<Vec<u8>, String> {
    let mut document = Document::default();
    document.openai_chat_request_context(value, provider.label(), default_model, input_model);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    match transform_openai_chat_request(&document.nodes, raw)
        .expect("Mojo OpenAI chat request transform returned invalid output")
    {
        OpenAiChatRequestTransform::Body(body) => Ok(body),
        OpenAiChatRequestTransform::Rejected(reason) => Err(reason),
    }
}

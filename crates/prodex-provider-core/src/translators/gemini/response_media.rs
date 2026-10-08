use crate::mojo_json::Document;
use prodex_mojo_core::rich::GeminiResponseMediaOperation;
use serde_json::{Value, json};

fn media_kernel_value(
    part: &Value,
    operation: GeminiResponseMediaOperation,
    response_id: Option<&str>,
    index: Option<usize>,
) -> Option<Value> {
    let mut envelope = json!({"part": part});
    if let Some(response_id) = response_id {
        envelope["response_id"] = Value::String(response_id.to_string());
    }
    if let Some(index) = index {
        envelope["index"] = Value::String(index.to_string());
    }
    let mut document = Document::default();
    document.push(&envelope, None, "");
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let body = prodex_mojo_core::rich::gemini_response_media(&document.nodes, raw, operation)
        .expect("Mojo Gemini response-media kernel returned invalid output")?;
    serde_json::from_slice(&body).ok()
}

pub(crate) fn gemini_media_content_item_from_part(part: &Value) -> Option<Value> {
    media_kernel_value(part, GeminiResponseMediaOperation::Content, None, None)
}

pub(crate) fn gemini_text_from_special_part(part: &Value) -> Option<String> {
    media_kernel_value(part, GeminiResponseMediaOperation::SpecialText, None, None)?
        .as_str()
        .map(str::to_string)
}

pub(crate) fn gemini_image_generation_call_item_from_part(
    response_id: &str,
    index: usize,
    part: &Value,
) -> Option<Value> {
    media_kernel_value(
        part,
        GeminiResponseMediaOperation::ImageGeneration,
        Some(response_id),
        Some(index),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_media_projection_is_mojo_owned_at_provider_boundary() {
        assert_eq!(
            gemini_media_content_item_from_part(
                &json!({"fileData": {"fileUri": "https://files.example/IMAGE.PNG"}})
            ),
            Some(json!({
                "type": "input_image",
                "image_url": "https://files.example/IMAGE.PNG"
            }))
        );
        assert_eq!(
            gemini_text_from_special_part(
                &json!({"executableCode": {"language": "rust", "code": "println!(\"ok\");"}})
            ),
            Some("Gemini executable code (rust):\n```rust\nprintln!(\"ok\");\n```".to_string())
        );
    }
}

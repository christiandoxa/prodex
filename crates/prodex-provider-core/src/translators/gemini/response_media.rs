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
    Some(serde_json::from_slice(&body).expect("Mojo Gemini media returned invalid JSON"))
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
    #[test]
    fn gemini_media_projection_keeps_data_url_suffix_and_mime_presence_semantics() {
        let cases = [
            (
                json!({"text": "data:image/png;base64,Zm9v"}),
                Some(json!({"type":"input_image", "image_url":"data:image/png;base64,Zm9v"})),
            ),
            (
                json!({"text": "data:audio/wav;base64,UklGRg=="}),
                Some(
                    json!({"type":"output_text", "text":"Gemini returned inline audio/wav media (8 base64 characters)."}),
                ),
            ),
            (json!({"text": "data:image/png;BASE64,a"}), None),
            (json!({"text": "data:image/png;base64"}), None),
            (
                json!({"inlineData": {"mimeType": "", "data": "abcd"}}),
                Some(
                    json!({"type":"output_text", "text":"Gemini returned inline  media (4 base64 characters)."}),
                ),
            ),
            (
                json!({"inlineData": {"mimeType": 42, "data": "abcd"}}),
                Some(
                    json!({"type":"output_text", "text":"Gemini returned inline application/octet-stream media (4 base64 characters)."}),
                ),
            ),
            (
                json!({"inline_data": {"mime_type": "image/webp", "data": "im"}}),
                Some(json!({"type":"input_image", "image_url":"data:image/webp;base64,im"})),
            ),
            (
                json!({"fileData": {"fileUri": "https://files.example/movie.MOV"}}),
                Some(
                    json!({"type":"output_text", "text":"Gemini returned video/quicktime media: https://files.example/movie.MOV"}),
                ),
            ),
            (
                json!({"fileData": {"fileUri": "https://files.example/PIC.PNG"}}),
                Some(json!({"type":"input_image", "image_url":"https://files.example/PIC.PNG"})),
            ),
        ];
        for (part, expected) in cases {
            assert_eq!(
                gemini_media_content_item_from_part(&part),
                expected,
                "part={part:?}"
            );
        }
        assert_eq!(
            gemini_text_from_special_part(&json!({"codeExecutionResult":{"output":"snow 雪"}})),
            Some(
                "Gemini code execution result (OUTCOME_UNSPECIFIED):\n```text\nsnow 雪\n```"
                    .to_string()
            )
        );
        let metadata = json!({"label":"雪", "escaped":"line\nquote\""});
        assert_eq!(
            gemini_text_from_special_part(&json!({"videoMetadata": metadata})),
            Some(format!(
                "Gemini video metadata: {}",
                serde_json::to_string(&metadata).unwrap()
            ))
        );
        assert_eq!(
            gemini_image_generation_call_item_from_part(
                "resp_\"雪",
                4,
                &json!({
                    "inlineData":{"mimeType":"image/png","data":"result"}
                })
            ),
            Some(
                json!({"type":"image_generation_call", "id":"ig_resp_\"雪_4", "status":"completed", "result":"result"})
            )
        );
    }
}

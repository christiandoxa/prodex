#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::json::{JsonKind, JsonNode};
use prodex_mojo_core::rich::{GeminiResponseMediaOperation, gemini_response_media};

fn node(
    kind: JsonKind,
    parent: Option<usize>,
    first_child: Option<usize>,
    next_sibling: Option<usize>,
    key: &'static str,
    text: &'static str,
) -> JsonNode<'static> {
    JsonNode {
        kind,
        first_child,
        next_sibling,
        parent,
        key,
        text,
        raw_start: 0,
        raw_length: 2,
    }
}

#[test]
fn direct_media_kernel_projects_inline_image() {
    let nodes = [
        node(JsonKind::Object, None, Some(1), None, "", ""),
        node(JsonKind::Object, Some(0), Some(2), None, "part", ""),
        node(JsonKind::Object, Some(1), Some(3), None, "inlineData", ""),
        node(
            JsonKind::String,
            Some(2),
            None,
            Some(4),
            "mimeType",
            "image/png",
        ),
        node(JsonKind::String, Some(2), None, None, "data", "aW1hZ2U="),
    ];

    assert_eq!(
        gemini_response_media(&nodes, "{}", GeminiResponseMediaOperation::Content).unwrap(),
        Some(br#"{"type":"input_image","image_url":"data:image/png;base64,aW1hZ2U="}"#.to_vec())
    );
}

#[test]
fn direct_media_kernel_projects_special_text_and_image_call() {
    let special_nodes = [
        node(JsonKind::Object, None, Some(1), None, "", ""),
        node(JsonKind::Object, Some(0), Some(2), None, "part", ""),
        node(
            JsonKind::Object,
            Some(1),
            Some(3),
            None,
            "executableCode",
            "",
        ),
        node(JsonKind::String, Some(2), None, Some(4), "language", "rust"),
        node(
            JsonKind::String,
            Some(2),
            None,
            None,
            "code",
            "println!(\"ok\");",
        ),
    ];
    assert_eq!(
        gemini_response_media(
            &special_nodes,
            "{}",
            GeminiResponseMediaOperation::SpecialText,
        )
        .unwrap(),
        Some(br#""Gemini executable code (rust):\n```rust\nprintln!(\"ok\");\n```""#.to_vec())
    );

    let image_nodes = [
        node(JsonKind::Object, None, Some(1), None, "", ""),
        node(JsonKind::Object, Some(0), Some(4), Some(2), "part", ""),
        node(
            JsonKind::String,
            Some(0),
            None,
            Some(3),
            "response_id",
            "resp_1",
        ),
        node(JsonKind::String, Some(0), None, None, "index", "7"),
        node(JsonKind::Object, Some(1), Some(5), None, "inlineData", ""),
        node(
            JsonKind::String,
            Some(4),
            None,
            Some(6),
            "mimeType",
            "image/png",
        ),
        node(JsonKind::String, Some(4), None, None, "data", "abc"),
    ];
    assert_eq!(
        gemini_response_media(
            &image_nodes,
            "{}",
            GeminiResponseMediaOperation::ImageGeneration,
        )
        .unwrap(),
        Some(
            br#"{"type":"image_generation_call","id":"ig_resp_1_7","status":"completed","result":"abc"}"#
                .to_vec(),
        )
    );
}

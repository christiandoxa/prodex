use super::*;
use crate::json::{JsonKind, ffi_nodes};

const RAW: &str = r#"{"provider":"copilot","slug":"synthetic","display_name":"Display","description":"Description","priority":1,"context_window":200000,"auto_compact_token_limit":190000,"default_reasoning_effort":null,"supported_reasoning_efforts":null}"#;

fn input() -> Vec<JsonNode<'static>> {
    use JsonKind::{Null, Number, Object, String};
    let mut nodes = vec![JsonNode {
        kind: Object,
        parent: None,
        first_child: Some(1),
        next_sibling: None,
        key: "",
        text: "",
        raw_start: 0,
        raw_length: RAW.len(),
    }];
    for (index, (kind, key, text, start, length)) in [
        (String, "provider", "copilot", 12, 9),
        (String, "slug", "synthetic", 29, 11),
        (String, "display_name", "Display", 56, 9),
        (String, "description", "Description", 80, 13),
        (Number, "priority", "", 105, 1),
        (Number, "context_window", "", 124, 6),
        (Number, "auto_compact_token_limit", "", 158, 6),
        (Null, "default_reasoning_effort", "", 192, 4),
        (Null, "supported_reasoning_efforts", "", 227, 4),
    ]
    .into_iter()
    .enumerate()
    {
        nodes.push(JsonNode {
            kind,
            parent: Some(0),
            first_child: None,
            next_sibling: (index < 8).then_some(index + 2),
            key,
            text,
            raw_start: start,
            raw_length: length,
        });
    }
    nodes
}

#[test]
fn compiled_runtime_schema_bridge_preserves_defaults_and_provider_capabilities() {
    let mut nodes = input();
    let output = runtime_model_catalog_json(&nodes, RAW).unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains("\"default_reasoning_level\":\"high\""));
    assert!(text.contains("\"effort\":\"xhigh\",\"description\":\"Extra-high reasoning effort\""));
    assert!(text.contains("\"input_modalities\":[\"text\",\"image\"]"));
    nodes[1].text = "kiro";
    let output = runtime_model_catalog_json(&nodes, RAW).unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains("\"supports_reasoning_summaries\":false"));
    assert!(text.contains("\"supports_search_tool\":false"));
    assert!(text.contains("\"input_modalities\":[\"text\"]"));
    nodes[1].text = "deepseek";
    let output = runtime_model_catalog_json(&nodes, RAW).unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains("\"default_reasoning_level\":\"high\""));
    assert!(
        text.contains("\"effort\":\"xhigh\",\"description\":\"DeepSeek max reasoning effort\"")
    );
    assert!(text.contains("\"input_modalities\":[\"text\"]"));
    assert!(text.contains("\"supports_reasoning_summaries\":false"));
    assert!(text.contains("\"supports_reasoning_summary_parameter\":false"));
    assert!(text.contains("\"supports_search_tool\":false"));
    assert!(text.contains("share the same workspace.\\n\\nFocus on the user's software task."));
    nodes[1].text = "unsupported";
    assert_eq!(
        runtime_model_catalog_json(&nodes, RAW),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_gemini_schema_keeps_complete_efforts_and_capabilities() {
    let mut nodes = input();
    nodes[1].text = "gemini";
    let output = runtime_model_catalog_json(&nodes, RAW).unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains(concat!(
        r#""supported_reasoning_levels":[{"effort":"minimal","description":"Disable Gemini thinking budget where supported"},"#,
        r#"{"effort":"low","description":"Small Gemini thinking budget"},"#,
        r#"{"effort":"medium","description":"Default Gemini thinking budget"},"#,
        r#"{"effort":"high","description":"High Gemini thinking budget"},"#,
        r#"{"effort":"xhigh","description":"Maximum Gemini thinking effort"}]"#,
    )));
    assert!(text.contains(r#""supports_reasoning_summaries":true"#));
    assert!(text.contains(r#""supports_search_tool":true"#));
    assert!(text.contains(r#""input_modalities":["text","image"]"#));
    assert!(text.contains("share the same workspace.\\n\\nFocus on the user's software task."));
}

#[test]
fn compiled_runtime_schema_abi_rejects_invalid_input_before_output_and_metadata() {
    let nodes = ffi_nodes(&input(), RAW).unwrap();
    let mut scratch = [0_u64; 20];
    let mut output = [93_u8; 2048];
    let mut metadata = [77_i64, 88];
    let input = nodes.as_ptr() as u64;
    let source = RAW.as_ptr() as u64;
    let storage = scratch.as_mut_ptr() as u64;
    let target = output.as_mut_ptr() as u64;
    let meta = metadata.as_mut_ptr() as u64;
    for (
        version,
        operation,
        flag,
        address,
        count,
        phase,
        destination,
        capacity,
        result,
        expected,
    ) in [
        (2, 0, 0, input, 10, 0, target, 2048, meta, 4),
        (1, 1, 0, input, 10, 0, target, 2048, meta, 1),
        (1, 0, 2, input, 10, 0, target, 2048, meta, 1),
        (1, 0, 0, input + 1, 10, 0, target, 2048, meta, 1),
        (1, 0, 0, input, 0, 0, target, 2048, meta, 1),
        (1, 0, 0, input, 10, 1, target, 0, meta, 1),
        (1, 0, 0, input, 10, 0, target, 2048, meta + 1, 1),
        (1, 0, 0, input, 10, 0, target, 1, meta, 3),
    ] {
        assert_eq!(
            unsafe {
                prodex_mojo_runtime_model_catalog_v1(
                    version,
                    operation,
                    flag,
                    address,
                    count,
                    source,
                    RAW.len() as i64,
                    storage,
                    10,
                    phase,
                    destination,
                    capacity,
                    result,
                )
            },
            expected
        );
        assert_eq!(output, [93; 2048]);
        assert_eq!(metadata, [77, 88]);
    }
}

#[test]
fn compiled_dynamic_catalog_bridge_preserves_order_duplicates_and_raw_entry_bounds() {
    let raw = r#"{"models":[{"id":" A "},{"id":"a"},{"id":null,"slug":"ignored"},{"model":"雪"}]}"#;
    let nodes = [
        (JsonKind::Object, None, Some(1), None, "", "", 0, 81),
        (
            JsonKind::Array,
            Some(0),
            Some(2),
            None,
            "models",
            "",
            10,
            70,
        ),
        (JsonKind::Object, Some(1), Some(3), Some(4), "", "", 11, 12),
        (JsonKind::String, Some(2), None, None, "id", " A ", 17, 5),
        (JsonKind::Object, Some(1), Some(5), Some(6), "", "", 24, 10),
        (JsonKind::String, Some(4), None, None, "id", "a", 30, 3),
        (JsonKind::Object, Some(1), Some(7), Some(9), "", "", 35, 28),
        (JsonKind::Null, Some(6), None, Some(8), "id", "", 41, 4),
        (
            JsonKind::String,
            Some(6),
            None,
            None,
            "slug",
            "ignored",
            53,
            9,
        ),
        (JsonKind::Object, Some(1), Some(10), None, "", "", 64, 15),
        (JsonKind::String, Some(9), None, None, "model", "雪", 73, 5),
    ]
    .into_iter()
    .map(
        |(kind, parent, first_child, next_sibling, key, text, raw_start, raw_length)| JsonNode {
            kind,
            parent,
            first_child,
            next_sibling,
            key,
            text,
            raw_start,
            raw_length,
        },
    )
    .collect::<Vec<_>>();
    let output = runtime_dynamic_catalog_json(&nodes, raw, 4).unwrap();
    assert_eq!(
        std::str::from_utf8(&output).unwrap(),
        r#"{"Ok":[{"slug":"A","display_name":null,"description":null,"context_window":null},{"slug":"a","display_name":null,"description":null,"context_window":null},{"slug":"雪","display_name":null,"description":null,"context_window":null}]}"#
    );

    let output = runtime_dynamic_catalog_json(&nodes, raw, 3).unwrap();
    assert_eq!(
        std::str::from_utf8(&output).unwrap(),
        r#"{"Err":"provider model catalog exceeds the hard limit of 3 entries"}"#
    );
    if let Ok(too_large) = usize::try_from(i64::MAX as u64 + 1) {
        assert_eq!(
            runtime_dynamic_catalog_json(&nodes, raw, too_large),
            Err(MojoError::InvalidInput)
        );
    }
}

#[test]
fn compiled_local_schema_from_minimal_observations_keeps_fixed_metadata() {
    let raw = r#"{"provider":"local","slug":"synthetic","context_window":0,"auto_compact_token_limit":18446744073709551615}"#;
    let nodes = [
        (JsonKind::Object, None, Some(1), None, "", "", 0, 106),
        (
            JsonKind::String,
            Some(0),
            None,
            Some(2),
            "provider",
            "local",
            12,
            7,
        ),
        (
            JsonKind::String,
            Some(0),
            None,
            Some(3),
            "slug",
            "synthetic",
            27,
            11,
        ),
        (
            JsonKind::Number,
            Some(0),
            None,
            Some(4),
            "context_window",
            "",
            56,
            1,
        ),
        (
            JsonKind::Number,
            Some(0),
            None,
            None,
            "auto_compact_token_limit",
            "",
            85,
            20,
        ),
    ]
    .into_iter()
    .map(
        |(kind, parent, first_child, next_sibling, key, text, raw_start, raw_length)| JsonNode {
            kind,
            parent,
            first_child,
            next_sibling,
            key,
            text,
            raw_start,
            raw_length,
        },
    )
    .collect::<Vec<_>>();
    let output = runtime_model_catalog_json(&nodes, raw).unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains(r#""slug":"synthetic","display_name":"synthetic""#));
    assert!(
        text.contains(r#""description":"Local OpenAI-compatible model routed through Prodex.""#)
    );
    assert!(text.contains(r#""priority":1"#));
    assert!(text.contains(r#""effort":"xhigh","description":"Max reasoning effort""#));
    assert!(text.contains(r#""supports_reasoning_summaries":false"#));
    assert!(text.contains(r#""supports_search_tool":true"#));
    assert!(text.contains(r#""input_modalities":["text"]"#));
    assert!(text.contains(r#""context_window":0,"max_context_window":0,"auto_compact_token_limit":18446744073709551615"#));
}

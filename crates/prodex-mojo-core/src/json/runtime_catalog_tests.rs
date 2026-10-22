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
    nodes[1].text = "unsupported";
    assert_eq!(
        runtime_model_catalog_json(&nodes, RAW),
        Err(MojoError::InvalidInput)
    );
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
        (1, 0, 1, input, 10, 0, target, 2048, meta, 1),
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

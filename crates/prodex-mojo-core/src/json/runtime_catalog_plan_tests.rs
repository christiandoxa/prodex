use super::*;
use crate::json::{JsonKind, ffi_nodes};

const RAW: &str = r#"{"provider":"copilot","launch_model":"synthetic","context_window":100000,"auto_compact_token_limit":90000,"dynamic_models":[],"static_models":[]}"#;

fn input() -> Vec<JsonNode<'static>> {
    use JsonKind::{Array, Number, Object, String};
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
        (String, "launch_model", "synthetic", 37, 11),
        (Number, "context_window", "", 66, 6),
        (Number, "auto_compact_token_limit", "", 100, 5),
        (Array, "dynamic_models", "", 123, 2),
        (Array, "static_models", "", 142, 2),
    ]
    .into_iter()
    .enumerate()
    {
        nodes.push(JsonNode {
            kind,
            parent: Some(0),
            first_child: None,
            next_sibling: (index < 5).then_some(index + 2),
            key,
            text,
            raw_start: start,
            raw_length: length,
        });
    }
    nodes
}

#[test]
fn compiled_catalog_plan_bridge_preserves_budget_and_zero_limit_rejection() {
    let nodes = input();
    let output = runtime_catalog_plan_json(&nodes, RAW, 1).unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.starts_with(r#"{"Ok":[{"slug":"synthetic""#));
    assert!(text.contains(r#""context_window":100000,"auto_compact_token_limit":90000"#));
    let output = runtime_catalog_plan_json(&nodes, RAW, 0).unwrap();
    assert_eq!(
        std::str::from_utf8(&output).unwrap(),
        r#"{"Err":"provider model catalog exceeds the hard limit of 0 entries"}"#
    );
    if let Ok(too_large) = usize::try_from(i64::MAX as u64 + 1) {
        assert_eq!(
            runtime_catalog_plan_json(&nodes, RAW, too_large),
            Err(MojoError::InvalidInput)
        );
    }
}

#[test]
fn compiled_catalog_plan_abi_preserves_output_and_metadata_on_invalid_input() {
    let nodes = ffi_nodes(&input(), RAW).unwrap();
    let mut scratch = vec![0_u64; nodes.len() * 8];
    let mut output = [93_u8; 2048];
    let mut metadata = [77_i64, 88];
    let input = nodes.as_ptr() as u64;
    let count = nodes.len() as i64;
    let source = RAW.as_ptr() as u64;
    let storage = scratch.as_mut_ptr() as u64;
    let target = output.as_mut_ptr() as u64;
    let meta = metadata.as_mut_ptr() as u64;
    for (version, limit, flag, address, units, phase, capacity, result, expected) in [
        (2, 1, 0, input, count * 4, 0, 2048, meta, 4),
        (1, -1, 0, input, count * 4, 0, 2048, meta, 1),
        (1, 1, 1, input, count * 4, 0, 2048, meta, 1),
        (1, 1, 0, input + 1, count * 4, 0, 2048, meta, 1),
        (1, 1, 0, input, count * 4 - 1, 0, 2048, meta, 1),
        (1, 1, 0, input, count * 4, 1, 2048, meta, 1),
        (1, 1, 0, input, count * 4, 0, 1, meta, 3),
        (1, 1, 0, input, count * 4, 0, 2048, meta + 1, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_mojo_runtime_catalog_plan_v1(
                    version,
                    limit,
                    flag,
                    address,
                    count,
                    source,
                    RAW.len() as i64,
                    storage,
                    units,
                    phase,
                    target,
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
fn compiled_deepseek_catalog_plan_bridge_preserves_complete_metadata() {
    let mut nodes = input();
    nodes[1].text = "deepseek";
    let output = runtime_catalog_plan_json(&nodes, RAW, 1).unwrap();
    assert_eq!(
        std::str::from_utf8(&output).unwrap(),
        r#"{"Ok":[{"slug":"synthetic","display_name":"synthetic","description":"DeepSeek model routed through the Prodex Responses adapter.","priority":1,"context_window":100000,"auto_compact_token_limit":90000}]}"#
    );
}

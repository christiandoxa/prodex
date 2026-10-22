use super::*;
use crate::json::{JsonKind, ffi_nodes};

fn input() -> [JsonNode<'static>; 4] {
    let root = JsonNode {
        kind: JsonKind::Object,
        parent: None,
        first_child: Some(1),
        next_sibling: None,
        key: "",
        text: "",
        raw_start: 0,
        raw_length: 0,
    };
    [
        root,
        JsonNode {
            kind: JsonKind::Array,
            parent: Some(0),
            first_child: Some(2),
            key: "models",
            ..root
        },
        JsonNode {
            parent: Some(1),
            first_child: Some(3),
            ..root
        },
        JsonNode {
            kind: JsonKind::String,
            parent: Some(2),
            first_child: None,
            key: "id",
            text: " x ",
            ..root
        },
    ]
}

#[test]
fn compiled_copilot_catalog_projects_models_and_reports_raw_limit_rejection() {
    let raw = r#"{"models":[{"id":" x "}]}"#;
    assert_eq!(
        copilot_model_catalog_json(&input(), raw, 1024).unwrap(),
        br#"{"Ok":[{"id":"x","object":"model","owned_by":"github-copilot","display_name":"x","description":"GitHub Copilot model available for this account: x.","context_window":200000,"input_cost_per_million_microusd":0,"output_cost_per_million_microusd":0}]}"#
    );
    assert_eq!(
        copilot_model_catalog_json(&input(), raw, 0).unwrap(),
        br#"{"Err":"Copilot model catalog exceeds the hard limit of 0 entries"}"#
    );
    let mut invalid_id = input();
    invalid_id[3].kind = JsonKind::Null;
    assert_eq!(
        copilot_model_catalog_json(&invalid_id, raw, 1024).unwrap(),
        br#"{"Ok":[]}"#
    );
}

#[test]
fn compiled_copilot_catalog_abi_validates_before_output_and_metadata_writes() {
    let raw = r#"{"models":[{"id":" x "}]}"#;
    let mut nodes = ffi_nodes(&input(), raw).unwrap();
    let mut scratch = [0_u64; 8];
    let mut output = [93_u8; 512];
    let mut metadata = [77_i64, 88];
    let input = nodes.as_ptr() as u64;
    let source = raw.as_ptr() as u64;
    let target = output.as_mut_ptr() as u64;
    let storage = scratch.as_mut_ptr() as u64;
    let meta = metadata.as_mut_ptr() as u64;
    for (version, limit, flag, expected) in [(2, 1024, 0, 4), (1, -1, 0, 1), (1, 1024, 1, 1)] {
        assert_eq!(
            unsafe {
                prodex_mojo_copilot_model_catalog_v1(
                    version,
                    limit,
                    flag,
                    input,
                    4,
                    source,
                    raw.len() as i64,
                    storage,
                    4,
                    0,
                    target,
                    512,
                    meta,
                )
            },
            expected
        );
    }
    for (address, count, scratch_count, phase, destination, capacity, result, expected) in [
        (input + 1, 4, 4, 0, target, 512, meta, 1),
        (input, 0, 4, 0, target, 512, meta, 1),
        (input, 4, 3, 0, target, 512, meta, 1),
        (input, 4, 4, 2, target, 512, meta, 1),
        (input, 4, 4, 1, target, 0, meta, 1),
        (input, 4, 4, 0, target, -1, meta, 1),
        (input, 4, 4, 0, target, 512, meta + 1, 1),
        (input, 4, 4, 0, target, 1, meta, 3),
    ] {
        assert_eq!(
            unsafe {
                prodex_mojo_copilot_model_catalog_v1(
                    1,
                    1024,
                    0,
                    address,
                    count,
                    source,
                    raw.len() as i64,
                    storage,
                    scratch_count,
                    phase,
                    destination,
                    capacity,
                    result,
                )
            },
            expected
        );
        assert_eq!(output, [93; 512]);
        assert_eq!(metadata, [77, 88]);
    }
    nodes[3].parent = 3;
    assert_eq!(
        unsafe {
            prodex_mojo_copilot_model_catalog_v1(
                1,
                1024,
                0,
                input,
                4,
                source,
                raw.len() as i64,
                storage,
                4,
                0,
                target,
                512,
                meta,
            )
        },
        1
    );
    assert_eq!(output, [93; 512]);
    assert_eq!(metadata, [77, 88]);
}

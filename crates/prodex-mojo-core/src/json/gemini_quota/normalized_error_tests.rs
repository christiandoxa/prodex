use super::*;
use crate::json::{JsonKind, ffi_nodes};

#[test]
fn compiled_normalization_probe_preserves_lazy_success_and_error_statuses() {
    for (status, needs_body) in [
        (0, false),
        (200, false),
        (399, false),
        (400, true),
        (429, true),
        (65535, true),
    ] {
        assert_eq!(
            gemini_error_normalization_needs_body(status),
            Ok(needs_body)
        );
    }
}

#[test]
fn normalized_error_abi_rejects_bad_probes_and_context_without_metadata_writes() {
    let mut metadata = [71_i64; 2];
    let ptr = metadata.as_mut_ptr() as u64;
    let call = |version, status, flag, nodes, count, address| unsafe {
        prodex_gemini_normalized_error_v1(
            version, status, flag, nodes, count, 0, 0, 0, 0, 1, 0, 0, address,
        )
    };
    assert_eq!(call(2, 400, 0, 0, 0, ptr), 4);
    assert_eq!(call(1, -1, 0, 0, 0, ptr), 1);
    assert_eq!(call(1, 65536, 0, 0, 0, ptr), 1);
    assert_eq!(call(1, 400, 2, 0, 0, ptr), 1);
    assert_eq!(call(1, 400, 0, 0, 0, 1), 1);
    assert_eq!(call(1, 400, 0, 1, 0, ptr), 1);
    assert_eq!(call(1, 400, 0, 0, -1, ptr), 1);
    assert_eq!(metadata, [71; 2]);
    assert_eq!(call(1, 400, 0, 0, 0, ptr), 0);
    assert_eq!(metadata, [2, 0]);
}

#[test]
fn compiled_normalized_error_measures_exact_json_and_checks_context_tags() {
    let raw =
        r#"{"parsed":false,"rate":false,"source":"unavailable","terminal":false,"value":null}"#;
    let mut nodes = vec![JsonNode {
        kind: JsonKind::Object,
        first_child: Some(1),
        next_sibling: None,
        parent: None,
        key: "",
        text: "",
        raw_start: 0,
        raw_length: raw.len(),
    }];
    for (index, (key, kind, text, literal)) in [
        ("parsed", JsonKind::False, "", "false"),
        ("rate", JsonKind::False, "", "false"),
        ("source", JsonKind::String, "unavailable", "\"unavailable\""),
        ("terminal", JsonKind::False, "", "false"),
        ("value", JsonKind::Null, "", "null"),
    ]
    .into_iter()
    .enumerate()
    {
        nodes.push(JsonNode {
            kind,
            first_child: None,
            next_sibling: (index < 4).then_some(index + 2),
            parent: Some(0),
            key,
            text,
            raw_start: raw.find(&format!("\"{key}\":")).unwrap() + key.len() + 3,
            raw_length: literal.len(),
        });
    }
    let expected = br#"{"error":{"code":"provider_error","gemini_error":{"message":"unavailable","status":500},"message":"unavailable","param":null,"type":"server_error"}}"#;
    assert_eq!(
        gemini_normalized_error_json(500, false, &nodes, raw),
        Ok(Some(expected.to_vec()))
    );
    assert_eq!(
        gemini_normalized_error_json(429, true, &nodes, raw),
        Ok(None)
    );
    let ffi = ffi_nodes(&nodes, raw).unwrap();
    let mut scratch = [0_u64; 12];
    let mut output = [77_u8; 1];
    let mut meta = [71_i64; 2];
    assert_eq!(
        unsafe {
            prodex_gemini_normalized_error_v1(
                1,
                500,
                0,
                ffi.as_ptr() as u64,
                6,
                raw.as_ptr() as u64,
                raw.len() as i64,
                scratch.as_mut_ptr() as u64,
                6,
                0,
                output.as_mut_ptr() as u64,
                1,
                meta.as_mut_ptr() as u64,
            )
        },
        3
    );
    nodes[1].kind = JsonKind::Null;
    assert_eq!(
        gemini_normalized_error_json(500, false, &nodes, raw),
        Err(MojoError::InvalidInput)
    );
}

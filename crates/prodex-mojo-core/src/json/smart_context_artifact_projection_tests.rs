use super::*;
use crate::json::JsonKind;

fn string_node(text: &str) -> JsonNode<'_> {
    JsonNode {
        kind: JsonKind::String,
        first_child: None,
        next_sibling: None,
        parent: None,
        key: "",
        text,
        raw_start: 0,
        raw_length: text.len() + 2,
    }
}

#[test]
fn compiled_artifact_projection_reuses_the_canonical_token_and_range_parser() {
    let text = "@1=psc:0123456789abcdef @1#L2-L3,1.";
    let raw = format!("\"{text}\"");
    let node = string_node(text);
    assert_eq!(smart_context_artifact_projection(&[node], &raw, SmartContextArtifactProjection::Request).unwrap(),
        br#"{"aliases":{"@1":"sc:0123456789abcdef"},"references":[{"id":"sc:0123456789abcdef","marker":"@1#L2-L3,1","line_ranges":[{"start":2,"end":3},{"start":1,"end":1}],"line_range":{"start":2,"end":3}}]}"#);
    assert_eq!(
        smart_context_artifact_projection(&[node], &raw, SmartContextArtifactProjection::Aliases)
            .unwrap(),
        br#"{"aliases":{"@1":"sc:0123456789abcdef"},"references":[]}"#
    );
}

#[test]
fn compiled_artifact_projection_abi_rejects_invalid_inputs_before_output_writes() {
    let text = "@1=psc:0123456789abcdef @1";
    let raw = format!("\"{text}\"");
    let input = ffi_nodes(&[string_node(text)], &raw).unwrap();
    let mut scratch = [StringView::default(); 9];
    let mut output = [57_u8; 512];
    let mut metadata = [89_i64; 2];
    let valid = [
        1,
        0,
        0,
        input.as_ptr() as usize as i64,
        1,
        raw.as_ptr() as usize as i64,
        raw.len() as i64,
        scratch.as_mut_ptr() as usize as i64,
        scratch.len() as i64,
        0,
        output.as_mut_ptr() as usize as i64,
        output.len() as i64,
        metadata.as_mut_ptr() as usize as i64,
    ];
    let call = |v: [i64; 13]| unsafe {
        prodex_smart_context_artifact_projection_v1(
            v[0],
            v[1],
            v[2],
            v[3] as u64,
            v[4],
            v[5] as u64,
            v[6],
            v[7] as u64,
            v[8],
            v[9],
            v[10] as u64,
            v[11],
            v[12] as u64,
        )
    };
    for (index, value, expected) in [
        (0, 2, 4),
        (1, 3, 1),
        (2, 1, 1),
        (3, 0, 1),
        (4, 0, 1),
        (4, -1, 1),
        (5, 0, 1),
        (6, -1, 1),
        (7, 0, 1),
        (8, 1, 1),
        (8, 2, 3),
        (8, i64::MAX, 1),
        (9, 2, 1),
        (10, 0, 1),
        (11, 1, 3),
        (11, -1, 1),
        (12, 0, 1),
    ] {
        let mut v = valid;
        v[index] = value;
        assert_eq!(call(v), expected, "argument {index}={value}");
        assert_eq!(output, [57; 512]);
        assert_eq!(metadata, [89; 2]);
    }
    let mut required = 89_i64;
    let result = (&mut required as *mut i64) as usize as u64;
    let input_address = input.as_ptr() as usize as u64;
    let raw_address = raw.as_ptr() as usize as u64;
    let probe = |abi, op, nodes, count, out| unsafe {
        prodex_smart_context_artifact_alias_count_v1(
            abi,
            op,
            nodes,
            count,
            raw_address,
            raw.len() as i64,
            out,
        )
    };
    assert_eq!(probe(2, 0, input_address, 1, result), 4);
    assert_eq!(probe(1, 3, input_address, 1, result), 1);
    assert_eq!(probe(1, 0, 0, 1, result), 1);
    assert_eq!(probe(1, 0, input_address, 0, result), 1);
    assert_eq!(probe(1, 0, input_address, 1, 0), 1);
    assert_eq!(required, 89);
    assert_eq!(probe(1, 0, input_address, 1, result), 0);
    assert_eq!(required, 1);
    let invalid_utf8 = [0xff_u8];
    let mut malformed = ffi_nodes(&[string_node(text)], &raw).unwrap();
    malformed[0].text = StringView {
        address: invalid_utf8.as_ptr() as usize as u64,
        length: 1,
    };
    let mut v = valid;
    v[3] = malformed.as_ptr() as usize as i64;
    assert_eq!(call(v), 1);
    assert_eq!(output, [57; 512]);
    assert_eq!(metadata, [89; 2]);
    malformed[0].text.length = 67_108_865;
    assert_eq!(call(v), 1);
    let mut invalid = string_node(text);
    invalid.first_child = Some(0);
    assert_eq!(
        smart_context_artifact_projection(
            &[invalid],
            &raw,
            SmartContextArtifactProjection::Request
        ),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_artifact_alias_storage_is_isolated_across_parallel_calls() {
    std::thread::scope(|scope| {
        for worker in 0..8 {
            scope.spawn(move || {
                let text = format!("@{worker}=psc:0123456789abcdef @{worker}=psc:fedcba9876543210");
                let raw = format!("\"{text}\"");
                let expected = format!(
                    r#"{{"aliases":{{"@{worker}":"sc:0123456789abcdef"}},"references":[]}}"#
                );
                for _ in 0..32 {
                    assert_eq!(
                        smart_context_artifact_projection(
                            &[string_node(&text)],
                            &raw,
                            SmartContextArtifactProjection::Aliases
                        )
                        .unwrap(),
                        expected.as_bytes(),
                    );
                }
            });
        }
    });
}

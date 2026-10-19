use super::*;
use crate::json::{JsonKind, ffi_nodes};

fn empty() -> JsonNode<'static> {
    JsonNode {
        kind: JsonKind::Object,
        first_child: None,
        next_sibling: None,
        parent: None,
        key: "",
        text: "",
        raw_start: 0,
        raw_length: 0,
    }
}

#[test]
fn compiled_transcript_context_preserves_presence_before_type_and_raw_text() {
    let mut root = empty();
    root.first_child = Some(1);
    let nodes = [
        root,
        JsonNode {
            kind: JsonKind::Null,
            next_sibling: Some(2),
            parent: Some(0),
            key: "model_provider",
            ..empty()
        },
        JsonNode {
            kind: JsonKind::String,
            parent: Some(0),
            key: "provider",
            text: "東京🧭\0\n\"",
            ..empty()
        },
    ];
    assert_eq!(
        log_transcript_context(&nodes, TranscriptContextKind::SessionMetadata),
        Ok(b"[]".to_vec())
    );
    assert_eq!(
        log_transcript_context(&nodes, TranscriptContextKind::TurnContext),
        Ok(b"[]".to_vec())
    );
    assert_eq!(
        log_transcript_context(&nodes, TranscriptContextKind::Reasoning),
        Ok(b"[]".to_vec())
    );
    let nodes = [
        root,
        JsonNode {
            kind: JsonKind::String,
            parent: Some(0),
            key: "provider",
            text: "東京🧭\0\n\"",
            ..empty()
        },
    ];
    assert_eq!(
        log_transcript_context(&nodes, TranscriptContextKind::SessionMetadata),
        Ok("[[\"session-context\",\"provider=東京🧭\\u0000\\n\\\"\"]]"
            .as_bytes()
            .to_vec())
    );
    let mut invalid = nodes;
    invalid[1].next_sibling = Some(1);
    assert_eq!(
        log_transcript_context(&invalid, TranscriptContextKind::SessionMetadata),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_transcript_context_abi_rejects_fields_topology_utf8_and_capacity_before_writes() {
    let input = ffi_nodes(&[empty()], "").unwrap();
    let mut scratch = [0_u64; 2];
    let mut output = [61_u8; 64];
    let mut metadata = [71_i64; 2];
    let nodes = input.as_ptr() as u64;
    let mut call = |abi, op, flag, nodes, count, scratch_count, measure, dest, capacity, meta| unsafe {
        prodex_mojo_log_transcript_context_v1(
            abi,
            op,
            flag,
            nodes,
            count,
            0,
            0,
            scratch.as_mut_ptr() as u64,
            scratch_count,
            measure,
            dest,
            capacity,
            meta,
        )
    };
    let dest = output.as_mut_ptr() as u64;
    let meta = metadata.as_mut_ptr() as u64;
    assert_eq!(call(2, 0, 0, nodes, 1, 1, 0, dest, 64, meta), 4);
    for (op, flag, count, scratch_count, measuring, buffer, capacity, result) in [
        (8, 0, 1, 1, 0, dest, 64, 1),
        (0, 1, 1, 1, 0, dest, 64, 1),
        (0, 0, 0, 1, 0, dest, 64, 1),
        (0, 0, 1, 0, 0, dest, 64, 1),
        (0, 0, 1, 1, 2, dest, 64, 1),
        (0, 0, 1, 1, 1, dest, 64, 1),
        (0, 0, 1, 1, 0, 0, 64, 1),
        (0, 0, 1, 1, 0, dest, -1, 1),
        (0, 0, 1, 1, 0, dest, 1, 3),
    ] {
        assert_eq!(
            call(
                1,
                op,
                flag,
                nodes,
                count,
                scratch_count,
                measuring,
                buffer,
                capacity,
                meta
            ),
            result
        );
    }
    assert_eq!(call(1, 0, 0, nodes, 1, 1, 0, dest, 64, 1), 1);
    let mut malformed = ffi_nodes(&[empty()], "").unwrap();
    malformed[0].first_child = 0;
    assert_eq!(
        call(1, 0, 0, malformed.as_ptr() as u64, 1, 1, 0, dest, 64, meta),
        1
    );
    malformed[0].first_child = -1;
    let invalid = [0xff_u8];
    malformed[0].text.address = invalid.as_ptr() as u64;
    malformed[0].text.length = 1;
    assert_eq!(
        call(1, 0, 0, malformed.as_ptr() as u64, 1, 1, 0, dest, 64, meta),
        1
    );
    assert_eq!(output, [61; 64]);
    assert_eq!(metadata, [71; 2]);
    assert_eq!(call(1, 0, 0, nodes, 1, 1, 1, 0, 0, meta), 0);
    assert_eq!(metadata, [1, 2]);
}

use super::*;
use crate::json::JsonKind;

fn node<'a>(kind: JsonKind, key: &'a str, text: &'a str) -> JsonNode<'a> {
    JsonNode {
        kind,
        first_child: None,
        next_sibling: None,
        parent: None,
        key,
        text,
        raw_start: 0,
        raw_length: 0,
    }
}

#[test]
fn compiled_memory_estimate_counts_payloads_and_ignores_the_synthetic_array() {
    let nodes = [
        node(JsonKind::Array, "", ""),
        node(JsonKind::Number, "key", ""),
        node(JsonKind::False, "", ""),
        node(JsonKind::String, "x", "é"),
    ];
    let layout = JsonMemoryLayout {
        value_bytes: 32,
        bool_bytes: 1,
        number_bytes: 16,
    };
    assert_eq!(conversation_estimated_bytes(&nodes, 4, layout), Ok(123));
    assert_eq!(conversation_estimated_bytes(&nodes[..1], 4, layout), Ok(4));
    assert_eq!(
        conversation_estimated_bytes(&nodes, usize::MAX, layout),
        Ok(usize::MAX)
    );
    assert_eq!(
        conversation_estimated_bytes(&[], 4, layout),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        conversation_estimated_bytes(&nodes[1..], 4, layout),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_memory_abi_rejects_malformed_numeric_facts_atomically() {
    let mut nodes = [
        MemoryNode {
            kind: 5,
            key_bytes: 0,
            string_bytes: 0,
        },
        MemoryNode {
            kind: 2,
            key_bytes: 0,
            string_bytes: 0,
        },
    ];
    let mut output = 77_u64;
    let mut call = |version, address, value_bytes| unsafe {
        prodex_provider_conversation_memory_v1(
            version,
            address,
            2,
            0,
            value_bytes,
            1,
            16,
            u64::MAX,
            (&raw mut output) as u64,
        )
    };
    assert_eq!(call(2, nodes.as_ptr() as u64, 32), 4);
    assert_eq!(call(1, nodes.as_ptr() as u64 + 1, 32), 1);
    assert_eq!(call(1, nodes.as_ptr() as u64, 0), 1);
    nodes[1].string_bytes = 1;
    assert_eq!(call(1, nodes.as_ptr() as u64, 32), 1);
    assert_eq!(output, 77);
}

#![cfg(feature = "mojo-rich")]
use prodex_mojo_core::{
    MojoError,
    deepseek_messages::{DeepSeekMessageOperation as Op, transform_deepseek_messages},
    json::{JsonKind, JsonNode},
};

#[test]
fn deepseek_message_boundary_preserves_empty_array_and_scalar() {
    let array = JsonNode {
        kind: JsonKind::Array,
        first_child: None,
        next_sibling: None,
        parent: None,
        key: "",
        text: "",
        raw_start: 0,
        raw_length: 2,
    };
    for operation in [Op::ThinkingMessages, Op::RepairAdjacency] {
        assert_eq!(
            transform_deepseek_messages(&[array], "[]", operation).unwrap(),
            Some(b"[]".to_vec())
        );
    }
    assert_eq!(
        transform_deepseek_messages(&[array], "[]", Op::MergeMetadata).unwrap(),
        None
    );
    let scalar = JsonNode {
        kind: JsonKind::Null,
        raw_length: 4,
        ..array
    };
    assert_eq!(
        transform_deepseek_messages(&[scalar], "null", Op::AssistantContent).unwrap(),
        Some(b"null".to_vec())
    );
    assert_eq!(
        transform_deepseek_messages(&[scalar], "null", Op::ThinkingMessages).unwrap(),
        None
    );
}

#[test]
fn deepseek_message_boundary_rejects_bad_graphs() {
    let array = JsonNode {
        kind: JsonKind::Array,
        first_child: Some(0),
        next_sibling: None,
        parent: None,
        key: "",
        text: "",
        raw_start: 0,
        raw_length: 2,
    };
    assert_eq!(
        transform_deepseek_messages(&[array], "[]", Op::RepairAdjacency),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        transform_deepseek_messages(&[], "[]", Op::RepairAdjacency),
        Err(MojoError::InvalidInput)
    );
}

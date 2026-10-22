use super::*;
use crate::json::JsonKind;
use SessionPromptWriteJsonCheck::{
    ExactUserMessage, RecordShape, TranscriptPayloadVisible, VisibleUserMessage,
};

fn node(
    kind: JsonKind,
    key: &'static str,
    text: &'static str,
    parent: Option<usize>,
    child: Option<usize>,
    next: Option<usize>,
) -> JsonNode<'static> {
    JsonNode {
        kind,
        key,
        text,
        parent,
        first_child: child,
        next_sibling: next,
        raw_start: 0,
        raw_length: 0,
    }
}

fn response() -> Vec<JsonNode<'static>> {
    use JsonKind::{Array, Object, String};
    vec![
        node(Object, "", "", None, Some(1), None),
        node(String, "type", "response_item", Some(0), None, Some(2)),
        node(Object, "payload", "", Some(0), Some(3), None),
        node(String, "type", "message", Some(2), None, Some(4)),
        node(String, "role", "user", Some(2), None, Some(5)),
        node(Array, "content", "", Some(2), Some(6), Some(10)),
        node(Object, "", "", Some(5), Some(7), Some(8)),
        node(String, "text", "  東京🧭\0 ", Some(6), None, None),
        node(Object, "", "", Some(5), Some(9), None),
        node(String, "content", "\nend ", Some(8), None, None),
        node(
            Object,
            "internal_chat_message_metadata_passthrough",
            "",
            Some(2),
            Some(11),
            None,
        ),
        node(Array, "content_item_kinds", "", Some(10), Some(12), None),
        node(String, "", "user.text", Some(11), None, None),
    ]
}

#[test]
fn decoded_session_exact_match_keeps_parts_whitespace_and_present_text_priority() {
    let mut nodes = response();
    assert_eq!(
        session_prompt_write_json_check(
            &nodes,
            ExactUserMessage {
                expected: "  東京🧭\0 \n\nend "
            }
        ),
        Ok(true)
    );
    assert_eq!(
        session_prompt_write_json_check(
            &nodes,
            ExactUserMessage {
                expected: "東京🧭\0 \n\nend"
            }
        ),
        Ok(false)
    );
    nodes[7].text = "";
    nodes[9].text = "";
    assert_eq!(
        session_prompt_write_json_check(&nodes, ExactUserMessage { expected: "\n" }),
        Ok(true)
    );
    assert_eq!(
        session_prompt_write_json_check(&nodes, ExactUserMessage { expected: "" }),
        Ok(false)
    );
    nodes[7].kind = JsonKind::Null;
    nodes[7].next_sibling = Some(13);
    nodes.push(node(
        JsonKind::String,
        "content",
        "fallback",
        Some(6),
        None,
        None,
    ));
    assert_eq!(
        session_prompt_write_json_check(
            &nodes,
            ExactUserMessage {
                expected: "fallback\n"
            }
        ),
        Ok(false)
    );
    nodes[7].key = "other";
    assert_eq!(
        session_prompt_write_json_check(
            &nodes,
            ExactUserMessage {
                expected: "fallback\n"
            }
        ),
        Ok(true)
    );
    nodes[12].text = "internal";
    assert_eq!(
        session_prompt_write_json_check(
            &nodes,
            ExactUserMessage {
                expected: "fallback\n"
            }
        ),
        Ok(false)
    );
}

#[test]
fn decoded_session_shape_and_visibility_keep_event_response_and_metadata_distinct() {
    let mut nodes = response();
    assert_eq!(
        session_prompt_write_json_check(&nodes, RecordShape),
        Ok(true)
    );
    assert_eq!(
        session_prompt_write_json_check(&nodes, VisibleUserMessage),
        Ok(true)
    );
    nodes[5].kind = JsonKind::Null;
    nodes[5].first_child = None;
    nodes.truncate(6);
    nodes[5].next_sibling = None;
    assert_eq!(
        session_prompt_write_json_check(&nodes, VisibleUserMessage),
        Ok(true)
    );
    assert_eq!(
        session_prompt_write_json_check(&nodes, ExactUserMessage { expected: "" }),
        Ok(false)
    );
    nodes[1].text = "event_msg";
    nodes[3].text = "user_message";
    nodes[4].key = "message";
    nodes[4].text = "event";
    assert_eq!(
        session_prompt_write_json_check(&nodes, VisibleUserMessage),
        Ok(true)
    );
    assert_eq!(
        session_prompt_write_json_check(&nodes, ExactUserMessage { expected: "event" }),
        Ok(true)
    );
    nodes[4].kind = JsonKind::Null;
    assert_eq!(
        session_prompt_write_json_check(&nodes, VisibleUserMessage),
        Ok(false)
    );
    nodes[2].kind = JsonKind::Null;
    nodes[2].first_child = None;
    nodes.truncate(3);
    for (record_type, expected) in [
        ("event_msg", false),
        ("response_item", false),
        ("session_meta", false),
        ("turn_context", false),
        ("unknown", true),
    ] {
        nodes[1].text = record_type;
        assert_eq!(
            session_prompt_write_json_check(&nodes, RecordShape),
            Ok(expected)
        );
    }
    nodes[1].kind = JsonKind::Null;
    assert_eq!(
        session_prompt_write_json_check(&nodes, RecordShape),
        Ok(true)
    );
    nodes[1].next_sibling = None;
    nodes.truncate(2);
    assert_eq!(
        session_prompt_write_json_check(&nodes, RecordShape),
        Ok(false)
    );

    for (kind, child, expected) in [
        (JsonKind::Array, Some(12), true),
        (JsonKind::Array, None, false),
        (JsonKind::Null, None, true),
    ] {
        let mut nodes = response();
        nodes[11].kind = kind;
        nodes[11].first_child = child;
        if child.is_none() {
            nodes.truncate(12);
        }
        assert_eq!(
            session_prompt_write_json_check(&nodes, VisibleUserMessage),
            Ok(expected)
        );
    }
    let payload = [
        node(JsonKind::Object, "", "", None, Some(1), None),
        node(JsonKind::String, "role", "assistant", Some(0), None, None),
    ];
    assert_eq!(
        session_prompt_write_json_check(&payload, TranscriptPayloadVisible),
        Ok(true)
    );
}

#[test]
fn decoded_session_abi_rejects_version_operation_topology_utf8_and_bounds() {
    let nodes = response();
    let mut input = ffi_nodes(&nodes, "").unwrap();
    let call = |version, operation, address, count, expected, length| unsafe {
        prodex_session_prompt_write_json_v1(version, operation, address, count, expected, length)
    };
    let address = input.as_ptr() as u64;
    assert_eq!(call(2, 0, address, 13, 0, 0), -2);
    for (operation, address, count, expected, length) in [
        (4, address, 13, 0, 0),
        (0, 0, 13, 0, 0),
        (0, address, 0, 0, 0),
        (0, address, 1_048_577, 0, 0),
        (2, address, 13, 0, 1),
        (2, address, 13, 0, -1),
        (2, address, 13, 0, 67_108_865),
        (0, address, 13, 1, 1),
    ] {
        assert_eq!(call(1, operation, address, count, expected, length), -1);
    }
    input[1].next_sibling = 1;
    assert_eq!(call(1, 0, address, 13, 0, 0), -1);
    input[1].next_sibling = 2;
    input[1].key.length = 67_108_864;
    input[1].text.length = 1;
    assert_eq!(call(1, 0, address, 13, 0, 0), -1);
    input[1].key.length = 4;
    let invalid = [0xff_u8];
    input[1].text = super::super::StringView {
        address: invalid.as_ptr() as u64,
        length: 1,
    };
    assert_eq!(call(1, 0, address, 13, 0, 0), -1);
    assert_eq!(
        session_prompt_write_json_check(&[], RecordShape),
        Err(MojoError::InvalidInput)
    );
}

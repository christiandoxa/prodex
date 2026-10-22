use super::*;
use crate::json::JsonKind;

fn node(
    kind: JsonKind,
    parent: Option<usize>,
    child: Option<usize>,
    next: Option<usize>,
    key: &'static str,
    text: &'static str,
) -> JsonNode<'static> {
    JsonNode {
        kind,
        parent,
        first_child: child,
        next_sibling: next,
        key,
        text,
        raw_start: 0,
        raw_length: 0,
    }
}

fn fixture() -> (Vec<JsonNode<'static>>, Vec<TranscriptOperationValue>) {
    use JsonKind::{Array, Null, Object, String};
    let nodes = vec![
        node(Object, None, Some(1), None, "", ""),
        node(String, Some(0), None, Some(2), "type", "message"),
        node(String, Some(0), None, Some(3), "role", "user"),
        node(Array, Some(0), Some(4), Some(10), "content", ""),
        node(Object, Some(3), Some(5), Some(6), "", ""),
        node(String, Some(4), None, None, "text", "raw first"),
        node(Object, Some(3), Some(7), Some(8), "", ""),
        node(String, Some(6), None, None, "content", "raw second"),
        node(Object, Some(3), Some(9), None, "", ""),
        node(Null, Some(8), None, None, "text", ""),
        node(
            Object,
            Some(0),
            Some(11),
            None,
            "internal_chat_message_metadata_passthrough",
            "",
        ),
        node(Array, Some(10), Some(12), None, "content_item_kinds", ""),
        node(String, Some(11), None, None, "", "user.text"),
    ];
    let mut values: Vec<_> = (0..nodes.len())
        .map(|_| TranscriptOperationValue::default())
        .collect();
    values[5].output = "first".into();
    values[7].output = "second".into();
    (nodes, values)
}

#[test]
fn compiled_message_projection_joins_selected_prepared_parts_and_blocks_present_nonstring_fallback()
{
    let (mut nodes, mut values) = fixture();
    assert_eq!(
        log_transcript_operation(
            &nodes,
            "",
            &values,
            TranscriptOperationKind::ResponseMessage
        ),
        Ok(br#"[["user","first\nsecond"]]"#.to_vec())
    );
    nodes[9].next_sibling = Some(13);
    nodes.push(node(
        JsonKind::String,
        Some(8),
        None,
        None,
        "content",
        "fallback",
    ));
    values.push(TranscriptOperationValue {
        scalar: "unused".into(),
        output: "fallback".into(),
    });
    assert_eq!(
        log_transcript_operation(
            &nodes,
            "",
            &values,
            TranscriptOperationKind::ResponseMessage
        ),
        Ok(br#"[["user","first\nsecond"]]"#.to_vec())
    );
    values[5].output.clear();
    assert_eq!(
        log_transcript_operation(
            &nodes,
            "",
            &values,
            TranscriptOperationKind::ResponseMessage
        ),
        Ok(br#"[["user","second"]]"#.to_vec())
    );
    nodes[9].key = "other";
    assert_eq!(
        log_transcript_operation(
            &nodes,
            "",
            &values,
            TranscriptOperationKind::ResponseMessage
        ),
        Ok(br#"[["user","second\nfallback"]]"#.to_vec())
    );
}

#[test]
fn compiled_message_projection_keeps_role_defaults_and_shared_visibility_rules() {
    let (mut nodes, values) = fixture();
    nodes[12].text = "internal";
    assert_eq!(
        log_transcript_operation(
            &nodes,
            "",
            &values,
            TranscriptOperationKind::ResponseMessage
        ),
        Ok(b"[]".to_vec())
    );
    nodes[2].text = "assistant";
    assert_eq!(
        log_transcript_operation(
            &nodes,
            "",
            &values,
            TranscriptOperationKind::ResponseMessage
        ),
        Ok(br#"[["assistant","first\nsecond"]]"#.to_vec())
    );
    nodes[2].text = "";
    assert_eq!(
        log_transcript_operation(
            &nodes,
            "",
            &values,
            TranscriptOperationKind::ResponseMessage
        ),
        Ok(br#"[["","first\nsecond"]]"#.to_vec())
    );
    nodes[2].kind = JsonKind::Null;
    assert_eq!(
        log_transcript_operation(
            &nodes,
            "",
            &values,
            TranscriptOperationKind::ResponseMessage
        ),
        Ok(br#"[["message","first\nsecond"]]"#.to_vec())
    );
    nodes[2].kind = JsonKind::String;
    nodes[2].text = "user";
    nodes[11].kind = JsonKind::Null;
    nodes[11].first_child = None;
    nodes.truncate(12);
    assert_eq!(
        log_transcript_operation(
            &nodes,
            "",
            &values[..12],
            TranscriptOperationKind::ResponseMessage
        ),
        Ok(br#"[["user","first\nsecond"]]"#.to_vec())
    );
    nodes[11].kind = JsonKind::Array;
    assert_eq!(
        log_transcript_operation(
            &nodes,
            "",
            &values[..12],
            TranscriptOperationKind::ResponseMessage
        ),
        Ok(b"[]".to_vec())
    );
}

#[test]
fn compiled_event_messages_select_prepared_text_field_and_stable_source() {
    let mut nodes = [
        node(JsonKind::Object, None, Some(1), None, "", ""),
        node(
            JsonKind::String,
            Some(0),
            None,
            Some(2),
            "message",
            "raw message",
        ),
        node(
            JsonKind::String,
            Some(0),
            None,
            None,
            "text",
            "raw reasoning",
        ),
    ];
    let values = [
        TranscriptOperationValue::default(),
        TranscriptOperationValue {
            scalar: "unused".into(),
            output: "event".into(),
        },
        TranscriptOperationValue {
            scalar: "unused".into(),
            output: "reasoning".into(),
        },
    ];
    for (kind, source, text) in [
        (TranscriptOperationKind::UserMessage, "user", "event"),
        (
            TranscriptOperationKind::AssistantMessage,
            "assistant",
            "event",
        ),
        (
            TranscriptOperationKind::ReasoningMessage,
            "reasoning",
            "reasoning",
        ),
    ] {
        assert_eq!(
            log_transcript_operation(&nodes, "", &values, kind),
            Ok(format!("[[\"{source}\",\"{text}\"]]").into_bytes())
        );
    }
    nodes[1].kind = JsonKind::Null;
    assert_eq!(
        log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::UserMessage),
        Ok(br#"[["user",""]]"#.to_vec())
    );
}

#[test]
fn compiled_message_abi_rejects_invalid_shapes_views_and_capacity_without_writes() {
    let nodes = [node(JsonKind::Object, None, None, None, "", "")];
    let ffi = ffi_nodes(&nodes, "").unwrap();
    let mut views = [StringView::from(""); 2];
    let mut output = [61_u8; 64];
    let mut metadata = [71_i64; 2];
    let mut call = |op, count, address, capacity| unsafe {
        prodex_mojo_log_transcript_context_v1(
            1,
            op,
            0,
            ffi.as_ptr() as u64,
            1,
            0,
            0,
            address,
            count,
            0,
            output.as_mut_ptr() as u64,
            capacity,
            metadata.as_mut_ptr() as u64,
        )
    };
    assert_eq!(call(i64::MAX, 2, views.as_ptr() as u64, 64), 1);
    assert_eq!(call(13, 1, views.as_ptr() as u64, 64), 1);
    assert_eq!(call(14, 2, views.as_ptr() as u64, 1), 3);
    let invalid = [0xff_u8];
    views[1] = StringView {
        address: invalid.as_ptr() as u64,
        length: 1,
    };
    assert_eq!(call(16, 2, views.as_ptr() as u64, 64), 1);
    assert_eq!(output, [61; 64]);
    assert_eq!(metadata, [71; 2]);
}

use super::*;
use crate::json::JsonKind;

fn node(kind: JsonKind) -> JsonNode<'static> {
    JsonNode {
        kind,
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
fn compiled_ping_structured_failure_preserves_absence_fallback_and_decoded_fields() {
    assert_eq!(
        ping_structured_failure_detail(&[node(JsonKind::Array)]),
        Ok(None)
    );
    let mut nodes = vec![
        JsonNode {
            first_child: Some(1),
            ..node(JsonKind::Array)
        },
        JsonNode {
            first_child: Some(2),
            parent: Some(0),
            ..node(JsonKind::Object)
        },
        JsonNode {
            parent: Some(1),
            key: "type",
            text: "turn.failed",
            ..node(JsonKind::String)
        },
    ];
    assert_eq!(
        ping_structured_failure_detail(&nodes),
        Ok(Some("turn failed".to_string()))
    );
    nodes[2].text = "error";
    assert_eq!(
        ping_structured_failure_detail(&nodes),
        Ok(Some("Codex error".to_string()))
    );
    nodes[2].next_sibling = Some(3);
    nodes.push(JsonNode {
        parent: Some(1),
        next_sibling: Some(4),
        key: "message",
        text: "東京🧭\0\n",
        ..node(JsonKind::String)
    });
    nodes.push(JsonNode {
        parent: Some(1),
        key: "code",
        text: "code",
        ..node(JsonKind::String)
    });
    assert_eq!(
        ping_structured_failure_detail(&nodes),
        Ok(Some("東京🧭\0\n code".to_string()))
    );
    nodes[3].text = " \u{2003}\n";
    assert_eq!(
        ping_structured_failure_detail(&nodes),
        Ok(Some("code".to_string()))
    );
}

#[test]
fn compiled_ping_structured_failure_rejects_wrong_roots_and_malformed_topology() {
    assert_eq!(
        ping_structured_failure_detail(&[node(JsonKind::Object)]),
        Err(MojoError::InvalidInput)
    );
    let malformed = JsonNode {
        first_child: Some(0),
        ..node(JsonKind::Array)
    };
    assert_eq!(
        ping_structured_failure_detail(&[malformed]),
        Err(MojoError::InvalidInput)
    );
}

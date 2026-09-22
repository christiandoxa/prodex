#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::json::{
    AnthropicChatRequestTransform, JsonKind, JsonNode, transform_anthropic_chat_request,
};

fn document<'a>() -> (Vec<JsonNode<'a>>, &'static str) {
    let raw = "{}";
    (
        vec![
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(1),
                next_sibling: None,
                parent: None,
                key: "",
                text: "",
                raw_start: 0,
                raw_length: 2,
            },
            JsonNode {
                kind: JsonKind::Array,
                first_child: Some(2),
                next_sibling: None,
                parent: Some(0),
                key: "messages",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(3),
                next_sibling: None,
                parent: Some(1),
                key: "",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: Some(4),
                parent: Some(2),
                key: "role",
                text: "user",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: None,
                parent: Some(2),
                key: "content",
                text: "hello",
                raw_start: 0,
                raw_length: 0,
            },
        ],
        raw,
    )
}

#[test]
fn complete_anthropic_chat_boundary_returns_final_request() {
    let (nodes, raw) = document();
    let AnthropicChatRequestTransform::Body(body) =
        transform_anthropic_chat_request(&nodes, raw).expect("Anthropic transform")
    else {
        panic!("valid request unexpectedly rejected or degraded");
    };
    assert_eq!(
        String::from_utf8(body).expect("valid UTF-8"),
        r#"{"max_tokens":4096,"messages":[{"content":[{"text":"hello","type":"text"}],"role":"user"}],"model":"auto","stream":false}"#,
    );
}

#[test]
fn complete_anthropic_chat_boundary_rejects_bad_tree() {
    let (mut nodes, raw) = document();
    nodes[4].parent = Some(0);
    assert!(transform_anthropic_chat_request(&nodes, raw).is_err());
}

#[test]
fn complete_anthropic_chat_boundary_reports_policy_rejection() {
    let (mut nodes, raw) = document();
    nodes[1].key = "unknown";
    let AnthropicChatRequestTransform::Rejected(reason) =
        transform_anthropic_chat_request(&nodes, raw).expect("bounded rejection")
    else {
        panic!("invalid request should be rejected");
    };
    assert!(reason.contains("does not translate chat field"));
}

#[test]
fn complete_anthropic_chat_boundary_is_reentrant() {
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..200 {
                    let (nodes, raw) = document();
                    assert!(matches!(
                        transform_anthropic_chat_request(&nodes, raw).unwrap(),
                        AnthropicChatRequestTransform::Body(_)
                    ));
                }
            });
        }
    });
}

#![cfg(feature = "mojo-rich")]
#![allow(unsafe_code)]

use prodex_mojo_core::{
    MojoError,
    json::{JsonKind, JsonNode, OpenAiChatRequestTransform, transform_openai_chat_request},
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
                kind: JsonKind::String,
                first_child: None,
                next_sibling: Some(2),
                parent: Some(0),
                key: "provider",
                text: "Anthropic",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: Some(3),
                parent: Some(0),
                key: "default_model",
                text: "default-model",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(4),
                next_sibling: None,
                parent: Some(0),
                key: "request",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: None,
                parent: Some(3),
                key: "input",
                text: "hello",
                raw_start: 0,
                raw_length: 0,
            },
        ],
        raw,
    )
}

#[test]
fn complete_request_boundary_returns_one_authoritative_body() {
    let (nodes, raw) = document();
    let OpenAiChatRequestTransform::Body(body) =
        transform_openai_chat_request(&nodes, raw).expect("request transform")
    else {
        panic!("valid request unexpectedly rejected");
    };
    let text = String::from_utf8(body).expect("valid UTF-8");
    assert_eq!(
        text,
        r#"{"messages":[{"content":"hello","role":"user"}],"model":"default-model","stream":false}"#
    );
}

#[test]
fn complete_request_boundary_rejects_invalid_json_tree_before_calling_policy() {
    let (mut nodes, raw) = document();
    nodes[4].parent = Some(0);
    assert_eq!(
        transform_openai_chat_request(&nodes, raw),
        Err(MojoError::InvalidInput)
    );
}

#[repr(C)]
#[derive(Clone, Copy)]
struct View {
    ptr: u64,
    len: u64,
}

#[repr(C)]
struct Node {
    kind: i64,
    child: i64,
    next: i64,
    parent: i64,
    key: View,
    text: View,
    start: i64,
    length: i64,
}

unsafe extern "C" {
    fn prodex_mojo_openai_chat_request_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

#[test]
fn raw_request_boundary_rejects_bad_abi_and_capacity_without_writes() {
    const { assert!(std::mem::size_of::<Node>() == 80) };
    let empty = View { ptr: 0, len: 0 };
    let node = Node {
        kind: 6,
        child: -1,
        next: -1,
        parent: -1,
        key: empty,
        text: empty,
        start: 0,
        length: 2,
    };
    let mut scratch = [empty];
    let raw = b"{}";
    for (abi, operation, flag, measuring, output_address, capacity, expected) in [
        (2, 0, 0, 1, 0, 0, 4),
        (1, 1, 0, 1, 0, 0, 1),
        (1, 0, 1, 1, 0, 0, 1),
        (1, 0, 0, 2, 0, 0, 1),
        (1, 0, 0, 0, 0, 8, 1),
    ] {
        let mut metadata = [31_i64; 2];
        let mut output = [0xa5_u8; 8];
        let status = unsafe {
            prodex_mojo_openai_chat_request_v1(
                abi,
                operation,
                flag,
                &node as *const Node as u64,
                1,
                raw.as_ptr() as u64,
                2,
                scratch.as_mut_ptr() as u64,
                1,
                measuring,
                if output_address == 0 {
                    output_address
                } else {
                    output.as_mut_ptr() as u64
                },
                capacity,
                metadata.as_mut_ptr() as u64,
            )
        };
        assert_eq!(status, expected);
        assert_eq!(metadata, [31, 31]);
        assert_eq!(output, [0xa5; 8]);
    }
}

#[test]
fn complete_request_boundary_is_reentrant() {
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..200 {
                    let (nodes, raw) = document();
                    assert!(matches!(
                        transform_openai_chat_request(&nodes, raw).unwrap(),
                        OpenAiChatRequestTransform::Body(_)
                    ));
                }
            });
        }
    });
}

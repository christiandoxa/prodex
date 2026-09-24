#![cfg(feature = "mojo-rich")]
#![allow(unsafe_code)]

use prodex_mojo_core::{
    MojoError,
    json::{
        JsonKind, JsonNode, transform_openai_chat_response, transform_openai_chat_stream_event,
    },
};

fn response_document() -> (Vec<JsonNode<'static>>, &'static str) {
    let raw = r#"{"fallback_created_at":123,"response":{"choices":[]}}"#;
    let fallback_start = raw.find("123").expect("fallback span");
    let response_start = raw.find(r#"{"choices":[]}"#).expect("response span");
    let choices_start = raw.find("[]").expect("choices span");
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
                raw_length: raw.len(),
            },
            JsonNode {
                kind: JsonKind::Number,
                first_child: None,
                next_sibling: Some(2),
                parent: Some(0),
                key: "fallback_created_at",
                text: "",
                raw_start: fallback_start,
                raw_length: 3,
            },
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(3),
                next_sibling: None,
                parent: Some(0),
                key: "response",
                text: "",
                raw_start: response_start,
                raw_length: raw.len() - response_start - 1,
            },
            JsonNode {
                kind: JsonKind::Array,
                first_child: None,
                next_sibling: None,
                parent: Some(2),
                key: "choices",
                text: "",
                raw_start: choices_start,
                raw_length: 2,
            },
        ],
        raw,
    )
}

#[test]
fn complete_response_boundary_uses_caller_tree_and_clock_default() {
    let (nodes, raw) = response_document();
    let body = transform_openai_chat_response(&nodes, raw).expect("response transform");
    assert_eq!(
        body,
        br#"{"id":"resp_prodex","object":"response","created_at":123,"model":"unknown","output":[]}"#
    );
}

#[test]
fn complete_response_boundary_rejects_invalid_tree_before_policy() {
    let (mut nodes, raw) = response_document();
    nodes[2].parent = None;
    assert_eq!(
        transform_openai_chat_response(&nodes, raw),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn stream_boundary_reports_unsupported_event_without_output() {
    let (nodes, raw) = response_document();
    assert_eq!(transform_openai_chat_stream_event(&nodes, raw), Ok(None));
}

#[repr(C)]
#[derive(Clone, Copy)]
struct View {
    address: u64,
    length: u64,
}

#[repr(C)]
struct Node {
    kind: i64,
    first_child: i64,
    next_sibling: i64,
    parent: i64,
    key: View,
    text: View,
    raw_start: i64,
    raw_length: i64,
}

unsafe extern "C" {
    fn prodex_mojo_openai_chat_response_v1(
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
fn raw_response_boundary_rejects_bad_abi_without_writes() {
    const { assert!(std::mem::size_of::<Node>() == 80) };
    let empty = View {
        address: 0,
        length: 0,
    };
    let node = Node {
        kind: 6,
        first_child: -1,
        next_sibling: -1,
        parent: -1,
        key: empty,
        text: empty,
        raw_start: 0,
        raw_length: 2,
    };
    let raw = b"{}";
    let mut scratch = [empty];
    let mut metadata = [31_i64; 2];
    let mut output = [0xa5_u8; 8];
    let status = unsafe {
        prodex_mojo_openai_chat_response_v1(
            2,
            0,
            0,
            &node as *const Node as u64,
            1,
            raw.as_ptr() as u64,
            raw.len() as i64,
            scratch.as_mut_ptr() as u64,
            1,
            1,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            metadata.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 4);
    assert_eq!(metadata, [31, 31]);
    assert_eq!(output, [0xa5; 8]);
}

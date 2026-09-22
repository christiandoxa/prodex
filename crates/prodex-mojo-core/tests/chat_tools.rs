#![cfg(feature = "mojo-rich")]
#![allow(unsafe_code)]

use prodex_mojo_core::{
    MojoError,
    json::{ChatToolOperation, JsonKind, JsonNode, transform_chat_tools},
};

fn document<'a>(raw: &'a str) -> Vec<JsonNode<'a>> {
    vec![
        JsonNode {
            kind: JsonKind::Array,
            first_child: Some(1),
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: raw.len(),
        },
        JsonNode {
            kind: JsonKind::String,
            first_child: None,
            next_sibling: Some(2),
            parent: Some(0),
            key: "",
            text: " mcp__x ",
            raw_start: 1,
            raw_length: 10,
        },
        JsonNode {
            kind: JsonKind::String,
            first_child: None,
            next_sibling: None,
            parent: Some(0),
            key: "",
            text: " b ",
            raw_start: 12,
            raw_length: 5,
        },
    ]
}

#[test]
fn parsed_json_bridge_checks_links_ranges_and_utf8_boundaries() {
    let raw = r#"[" mcp__x "," b "]"#;
    let nodes = document(raw);
    assert_eq!(
        transform_chat_tools(&nodes, raw, ChatToolOperation::FlattenName, false).unwrap(),
        Some(br#""mcp__x__b""#.to_vec())
    );
    for (node, field, bad) in [(0, 0, 0), (0, 0, 3), (1, 1, 1), (1, 2, 1), (2, 2, 1)] {
        let mut invalid = nodes.clone();
        match field {
            0 => invalid[node].first_child = Some(bad),
            1 => invalid[node].next_sibling = Some(bad),
            _ => invalid[node].parent = Some(bad),
        }
        assert_eq!(
            transform_chat_tools(&invalid, raw, ChatToolOperation::FlattenName, false),
            Err(MojoError::InvalidInput)
        );
    }
    let mut invalid = nodes.clone();
    invalid[1].raw_length = usize::MAX;
    assert_eq!(
        transform_chat_tools(&invalid, raw, ChatToolOperation::FlattenName, false),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        transform_chat_tools(&[], raw, ChatToolOperation::Tools, false),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn complete_json_bridge_is_reentrant() {
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..500 {
                    let raw = r#"[" mcp__x "," b "]"#;
                    assert_eq!(
                        transform_chat_tools(
                            &document(raw),
                            raw,
                            ChatToolOperation::FlattenName,
                            false
                        )
                        .unwrap(),
                        Some(br#""mcp__x__b""#.to_vec())
                    );
                }
            });
        }
    });
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
    fn prodex_mojo_chat_tools_v1(
        abi: i64,
        op: i64,
        thinking: i64,
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
fn raw_json_boundary_rejects_invalid_abi_and_records_before_writes() {
    const { assert!(std::mem::size_of::<Node>() == 80) };
    let raw = b"null";
    let empty = View { ptr: 0, len: 0 };
    let node = Node {
        kind: 0,
        child: -1,
        next: -1,
        parent: -1,
        key: empty,
        text: empty,
        start: 0,
        length: 4,
    };
    let mut scratch = [empty];
    let mut metadata = [33_i64; 2];
    let mut output = [77_u8; 16];
    let address = &node as *const Node as u64;
    for (abi, op, thinking, nodes, count, scratch_count, measuring, capacity, expected) in [
        (2, 0, 0, address, 1, 1, 0, 16, 4),
        (1, 5, 0, address, 1, 1, 0, 16, 1),
        (1, 0, 2, address, 1, 1, 0, 16, 1),
        (1, 0, 0, 0, 1, 1, 0, 16, 1),
        (1, 0, 0, address, -1, 1, 0, 16, 1),
        (1, 0, 0, address, 1, 0, 0, 16, 1),
        (1, 0, 0, address, 1, 1, 2, 16, 1),
        (1, 0, 0, address, 1, 1, 1, 16, 1),
    ] {
        let status = unsafe {
            prodex_mojo_chat_tools_v1(
                abi,
                op,
                thinking,
                nodes,
                count,
                raw.as_ptr() as u64,
                4,
                scratch.as_mut_ptr() as u64,
                scratch_count,
                measuring,
                output.as_mut_ptr() as u64,
                capacity,
                metadata.as_mut_ptr() as u64,
            )
        };
        assert_eq!(status, expected);
        assert_eq!(metadata, [33; 2]);
        assert_eq!(output, [77; 16]);
    }
    let invalid_utf8 = [0xff_u8];
    let bad = Node {
        text: View {
            ptr: invalid_utf8.as_ptr() as u64,
            len: 1,
        },
        ..node
    };
    let status = unsafe {
        prodex_mojo_chat_tools_v1(
            1,
            0,
            0,
            &bad as *const Node as u64,
            1,
            raw.as_ptr() as u64,
            4,
            scratch.as_mut_ptr() as u64,
            1,
            1,
            0,
            0,
            metadata.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 1);
    assert_eq!(metadata, [33; 2]);
}

#[test]
fn measured_json_sink_respects_every_declared_output_capacity() {
    let namespace = "mcp__x";
    let name = "quote\"slash\\line\nnul\0🦀";
    let empty = View { ptr: 0, len: 0 };
    let view = |value: &str| View {
        ptr: value.as_ptr() as u64,
        len: value.len() as u64,
    };
    let nodes = [
        Node {
            kind: 5,
            child: 1,
            next: -1,
            parent: -1,
            key: empty,
            text: empty,
            start: 0,
            length: 2,
        },
        Node {
            kind: 4,
            child: -1,
            next: 2,
            parent: 0,
            key: empty,
            text: view(namespace),
            start: 0,
            length: 0,
        },
        Node {
            kind: 4,
            child: -1,
            next: -1,
            parent: 0,
            key: empty,
            text: view(name),
            start: 0,
            length: 0,
        },
    ];
    let expected = r#""mcp__x__quote\"slash\\line\nnul\u0000🦀""#.as_bytes();
    let mut scratch = [empty; 3];
    let mut metadata = [-1_i64; 2];
    let status = unsafe {
        prodex_mojo_chat_tools_v1(
            1,
            4,
            0,
            nodes.as_ptr() as u64,
            3,
            b"[]".as_ptr() as u64,
            2,
            scratch.as_mut_ptr() as u64,
            3,
            1,
            0,
            0,
            metadata.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(metadata, [1, expected.len() as i64]);
    for capacity in 0..=expected.len() {
        let mut guarded = vec![0xa5_u8; capacity + 32];
        metadata = [-1; 2];
        let status = unsafe {
            prodex_mojo_chat_tools_v1(
                1,
                4,
                0,
                nodes.as_ptr() as u64,
                3,
                b"[]".as_ptr() as u64,
                2,
                scratch.as_mut_ptr() as u64,
                3,
                0,
                guarded.as_mut_ptr().add(16) as u64,
                capacity as i64,
                metadata.as_mut_ptr() as u64,
            )
        };
        assert!(guarded[..16].iter().all(|&byte| byte == 0xa5));
        assert!(guarded[16 + capacity..].iter().all(|&byte| byte == 0xa5));
        if capacity == expected.len() {
            assert_eq!(status, 0);
            assert_eq!(&guarded[16..16 + capacity], expected);
            assert_eq!(metadata, [1, capacity as i64]);
        } else {
            assert_eq!(status, 3);
            assert_eq!(metadata, [-1, -1]);
        }
    }
}

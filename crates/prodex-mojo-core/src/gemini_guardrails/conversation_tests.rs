use super::*;
use crate::json::{JsonKind, ffi_nodes};

fn node<'a>(
    kind: JsonKind,
    key: &'a str,
    text: &'a str,
    parent: Option<usize>,
    first_child: Option<usize>,
    next_sibling: Option<usize>,
) -> JsonNode<'a> {
    JsonNode {
        kind,
        key,
        text,
        parent,
        first_child,
        next_sibling,
        raw_start: 0,
        raw_length: 0,
    }
}

#[test]
fn compiled_guardrail_payload_preserves_presence_priority_empty_leaves_and_prefix() {
    let mut nodes = [
        node(JsonKind::Object, "", "", None, Some(1), None),
        node(JsonKind::String, "prefix", "seed", Some(0), None, Some(2)),
        node(JsonKind::Array, "value", "", Some(0), Some(3), None),
        node(JsonKind::String, "", "", Some(2), None, Some(4)),
        node(JsonKind::Object, "", "", Some(2), Some(5), Some(7)),
        node(JsonKind::Null, "output", "", Some(4), None, Some(6)),
        node(JsonKind::String, "content", "ignored", Some(4), None, None),
        node(JsonKind::String, "", "東京", Some(2), None, Some(8)),
        node(JsonKind::String, "", "", Some(2), None, Some(9)),
        node(JsonKind::Object, "", "", Some(2), Some(10), None),
        node(JsonKind::String, "text", "end", Some(9), None, None),
    ];
    assert_eq!(
        gemini_guardrail_json(&nodes, "", GeminiGuardrailJsonOperation::PayloadText).unwrap(),
        "\"seed\\n\\n東京\\n\\nend\"".as_bytes()
    );
    nodes[1].text = "";
    assert_eq!(
        gemini_guardrail_json(&nodes, "", GeminiGuardrailJsonOperation::PayloadText).unwrap(),
        "\"東京\\n\\nend\"".as_bytes()
    );
}

#[test]
fn compiled_guardrail_payload_handles_large_strings_and_deep_flat_abi_trees() {
    let text = "a".repeat(1_048_577);
    let mut nodes = vec![
        node(JsonKind::Object, "", "", None, Some(1), None),
        node(JsonKind::String, "prefix", "", Some(0), None, Some(2)),
        node(JsonKind::Object, "value", "", Some(0), Some(3), None),
    ];
    for index in 3..10_003 {
        nodes.push(node(
            JsonKind::Object,
            "output",
            "",
            Some(index - 1),
            Some(index + 1),
            None,
        ));
    }
    nodes.push(node(
        JsonKind::String,
        "output",
        &text,
        Some(10_002),
        None,
        None,
    ));
    assert_eq!(
        gemini_guardrail_json(&nodes, "", GeminiGuardrailJsonOperation::PayloadText).unwrap(),
        format!("\"{text}\"").as_bytes()
    );
}

#[test]
fn compiled_guardrail_json_rejects_invalid_abi_and_scratch_before_output_writes() {
    let nodes = [
        node(JsonKind::Object, "", "", None, Some(1), None),
        node(
            JsonKind::String,
            "prefix",
            "a long fixture prefix",
            Some(0),
            None,
            None,
        ),
    ];
    let input = ffi_nodes(&nodes, "").unwrap();
    let mut scratch = [0_u64; 16];
    for (abi, operation, scratch_count, raw_length, measuring) in [
        (2, 0, 8, 0, 1),
        (1, 2, 8, 0, 1),
        (1, 0, 1, 0, 1),
        (1, 0, 8, 1, 1),
        (1, 0, 8, 0, 2),
    ] {
        let mut metadata = [71_i64; 2];
        let status = unsafe {
            prodex_mojo_gemini_guardrail_conversation_v1(
                abi,
                operation,
                0,
                input.as_ptr() as u64,
                2,
                0,
                raw_length,
                scratch.as_mut_ptr() as u64,
                scratch_count,
                measuring,
                0,
                0,
                metadata.as_mut_ptr() as u64,
            )
        };
        assert_eq!(status, if abi == 2 { 4 } else { 1 });
        assert_eq!(metadata, [71; 2]);
    }
}

#[test]
fn compiled_exact_output_selection_preserves_roles_command_extraction_and_unicode() {
    let mut nodes = [
        node(JsonKind::Object, "", "", None, Some(1), None),
        node(JsonKind::Array, "messages", "", Some(0), Some(2), None),
        node(JsonKind::Object, "", "", Some(1), Some(3), Some(5)),
        node(JsonKind::String, "role", "user", Some(2), None, Some(4)),
        node(
            JsonKind::String,
            "content",
            "Return only the command output.",
            Some(2),
            None,
            None,
        ),
        node(JsonKind::Object, "", "", Some(1), Some(6), None),
        node(JsonKind::String, "role", "tool", Some(5), None, Some(7)),
        node(
            JsonKind::String,
            "output",
            "Output:\n 東京 🚀 \n",
            Some(5),
            None,
            None,
        ),
    ];
    let policy = GeminiGuardrailJsonOperation::ExactOutputSelection;
    assert_eq!(
        gemini_guardrail_json(&nodes, "", policy).unwrap(),
        "[true,0,1,null,\"東京 🚀\"]".as_bytes(),
    );
    nodes[4].text = "Return only the command output. Run exactly:\n  cargo test  \n";
    assert_eq!(
        gemini_guardrail_json(&nodes, "", policy).unwrap(),
        "[true,0,1,\"cargo test\",\"東京 🚀\"]".as_bytes(),
    );
    nodes[7].text = "Success. No files changed.";
    assert_eq!(
        gemini_guardrail_json(&nodes, "", policy).unwrap(),
        "[true,0,1,\"cargo test\",null]".as_bytes(),
    );
    nodes[4].text = "Summarize the output";
    assert_eq!(
        gemini_guardrail_json(&nodes, "", policy).unwrap(),
        "[false,0,null,null,null]".as_bytes(),
    );
    nodes[3].text = "assistant";
    assert_eq!(
        gemini_guardrail_json(&nodes, "", policy).unwrap(),
        "[false,null,null,null,null]".as_bytes(),
    );
}

#[test]
fn compiled_exact_output_selection_rejects_invalid_shapes_before_metadata_write() {
    let nodes = [
        node(JsonKind::Object, "", "", None, Some(1), None),
        node(
            JsonKind::String,
            "messages",
            "not an array",
            Some(0),
            None,
            None,
        ),
    ];
    let input = ffi_nodes(&nodes, "").unwrap();
    let mut scratch = [0_u64; 16];
    for (abi, operation, expected) in [(1, 2, 1), (1, 3, 1), (2, 2, 4)] {
        let mut metadata = [99_i64; 2];
        let status = unsafe {
            prodex_mojo_gemini_guardrail_conversation_v1(
                abi,
                operation,
                0,
                input.as_ptr() as u64,
                2,
                0,
                0,
                scratch.as_mut_ptr() as u64,
                16,
                1,
                0,
                0,
                metadata.as_mut_ptr() as u64,
            )
        };
        assert_eq!(status, expected);
        assert_eq!(metadata, [99; 2]);
    }
}

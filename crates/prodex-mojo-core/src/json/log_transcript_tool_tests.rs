use super::*;
use crate::json::JsonKind;

fn root() -> JsonNode<'static> {
    JsonNode {
        kind: JsonKind::Object,
        parent: None,
        first_child: Some(1),
        next_sibling: None,
        key: "",
        text: "",
        raw_start: 0,
        raw_length: 0,
    }
}

fn field(
    kind: JsonKind,
    key: &'static str,
    text: &'static str,
    next: Option<usize>,
) -> JsonNode<'static> {
    JsonNode {
        kind,
        parent: Some(0),
        first_child: None,
        next_sibling: next,
        key,
        text,
        raw_start: 0,
        raw_length: 0,
    }
}

#[test]
fn compiled_tool_calls_use_raw_arguments_and_prepared_names_with_typed_defaults() {
    let mut nodes = [
        root(),
        field(JsonKind::String, "name", "raw name", Some(2)),
        field(JsonKind::String, "arguments", "raw \"東京\"\0", Some(3)),
        field(JsonKind::String, "input", "custom\nraw", None),
    ];
    let values = [
        TranscriptOperationValue::default(),
        TranscriptOperationValue {
            scalar: "unused".into(),
            output: "safe.name".into(),
        },
        TranscriptOperationValue {
            scalar: "truncated".into(),
            output: "unused".into(),
        },
        TranscriptOperationValue {
            scalar: "truncated".into(),
            output: "unused".into(),
        },
    ];
    assert_eq!(
        log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::FunctionCall),
        Ok("[[\"tool-call:safe.name\",\"raw \\\"東京\\\"\\u0000\"]]"
            .as_bytes()
            .to_vec())
    );
    assert_eq!(
        log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::CustomCall),
        Ok(br#"[["tool-call:safe.name","custom\nraw"]]"#.to_vec())
    );
    nodes[1].kind = JsonKind::Null;
    for (kind, expected) in [
        (TranscriptOperationKind::FunctionCall, "tool"),
        (TranscriptOperationKind::CustomCall, "custom-tool"),
    ] {
        nodes[2].kind = JsonKind::Null;
        nodes[3].kind = JsonKind::Null;
        assert_eq!(
            log_transcript_operation(&nodes, "", &values, kind),
            Ok(format!("[[\"tool-call:{expected}\",\"\"]]").into_bytes())
        );
    }
}

#[test]
fn compiled_tool_results_require_strings_and_preserve_full_prepared_output() {
    let mut nodes = [root(), field(JsonKind::String, "output", "raw", None)];
    let long = "a".repeat(300);
    let values = [
        TranscriptOperationValue::default(),
        TranscriptOperationValue {
            scalar: "truncated".into(),
            output: long.clone(),
        },
    ];
    assert_eq!(
        log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::ToolOutput),
        Ok(format!("[[\"tool-output\",\"{long}\"]]").into_bytes())
    );
    for kind in [
        JsonKind::Null,
        JsonKind::False,
        JsonKind::Number,
        JsonKind::Array,
        JsonKind::Object,
    ] {
        nodes[1].kind = kind;
        assert_eq!(
            log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::ToolOutput),
            Ok(br#"[["tool-output",""]]"#.to_vec())
        );
    }
}

#[test]
fn compiled_tool_abi_checks_new_modes_before_output_mutation() {
    let nodes = [JsonNode {
        first_child: None,
        ..root()
    }];
    let ffi = ffi_nodes(&nodes, "").unwrap();
    let mut views = [StringView::from(""); 2];
    let mut output = [61_u8; 64];
    let mut metadata = [71_i64; 2];
    let mut call = |operation, count, address, capacity| unsafe {
        prodex_mojo_log_transcript_context_v1(
            1,
            operation,
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
    assert_eq!(call(10, 1, views.as_ptr() as u64, 64), 1);
    assert_eq!(call(11, 2, views.as_ptr() as u64, 1), 3);
    let invalid = [0xff_u8];
    views[1] = StringView {
        address: invalid.as_ptr() as u64,
        length: 1,
    };
    assert_eq!(call(12, 2, views.as_ptr() as u64, 64), 1);
    assert_eq!(output, [61; 64]);
    assert_eq!(metadata, [71; 2]);
}

use super::*;
use crate::json::JsonKind;

fn node(kind: JsonKind, key: &'static str, next: Option<usize>) -> JsonNode<'static> {
    JsonNode {
        kind,
        first_child: None,
        next_sibling: next,
        parent: Some(0),
        key,
        text: "raw text",
        raw_start: 0,
        raw_length: 0,
    }
}

fn root() -> JsonNode<'static> {
    JsonNode {
        parent: None,
        first_child: Some(1),
        ..node(JsonKind::Object, "", None)
    }
}

#[test]
fn compiled_shell_projection_preserves_presence_before_type_and_prepared_names() {
    let mut nodes = [
        root(),
        node(JsonKind::Null, "command", Some(2)),
        node(JsonKind::String, "arguments", Some(3)),
        node(JsonKind::String, "name", None),
    ];
    let values = [
        TranscriptOperationValue::default(),
        TranscriptOperationValue::default(),
        TranscriptOperationValue {
            scalar: "echo \"東京\"\\path".into(),
            output: "visible output".into(),
        },
        TranscriptOperationValue {
            scalar: "unselected".into(),
            output: "safe.name".into(),
        },
    ];
    assert_eq!(
        log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::ShellCall),
        Ok(br#"[["tool-call:safe.name",""]]"#.to_vec())
    );
    nodes[1].key = "other";
    assert_eq!(
        log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::ShellCall),
        Ok("[[\"tool-call:safe.name\",\"echo \\\"東京\\\"\\\\path\"]]"
            .as_bytes()
            .to_vec())
    );
    nodes[3].kind = JsonKind::Null;
    assert_eq!(
        log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::ShellCall),
        Ok("[[\"tool-call:shell\",\"echo \\\"東京\\\"\\\\path\"]]"
            .as_bytes()
            .to_vec())
    );
    nodes[2].key = "stdout";
    nodes[1].key = "output";
    assert_eq!(
        log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::ShellOutput),
        Ok(br#"[["tool-output",""]]"#.to_vec())
    );
    nodes[1].key = "other";
    assert_eq!(
        log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::ShellOutput),
        Ok(br#"[["tool-output","visible output"]]"#.to_vec())
    );
}

#[test]
fn compiled_shell_projection_keeps_scalar_container_and_null_contracts() {
    for (kind, scalar, text) in [
        (JsonKind::False, "false", "false"),
        (JsonKind::True, "true", "true"),
        (JsonKind::Number, "1e+30", "1e+30"),
        (JsonKind::Array, "[]", "[]"),
        (JsonKind::Object, "{}", "{}"),
        (JsonKind::Null, "null", ""),
    ] {
        let mut nodes = [root(), node(kind, "action", None)];
        let values = [
            TranscriptOperationValue::default(),
            TranscriptOperationValue {
                scalar: scalar.into(),
                output: "projected".into(),
            },
        ];
        assert_eq!(
            log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::ShellCall),
            Ok(format!("[[\"tool-call:shell\",\"{text}\"]]").into_bytes())
        );
        nodes[1].key = "aggregated_output";
        let output = if matches!(kind, JsonKind::Null) {
            ""
        } else {
            "projected"
        };
        assert_eq!(
            log_transcript_operation(&nodes, "", &values, TranscriptOperationKind::ShellOutput),
            Ok(format!("[[\"tool-output\",\"{output}\"]]").into_bytes())
        );
    }
}

#[test]
fn compiled_shell_abi_rejects_invalid_shapes_and_capacity_before_writes() {
    let nodes = [JsonNode {
        first_child: None,
        ..root()
    }];
    let ffi = ffi_nodes(&nodes, "").unwrap();
    let mut views = [StringView::from(""); 2];
    let mut output = [61_u8; 64];
    let mut metadata = [71_i64; 2];
    let mut call = |abi, operation, count, capacity, address| unsafe {
        prodex_mojo_log_transcript_context_v1(
            abi,
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
    assert_eq!(call(2, 8, 2, 64, views.as_ptr() as u64), 4);
    assert_eq!(call(1, 10, 2, 64, views.as_ptr() as u64), 1);
    assert_eq!(call(1, 8, 1, 64, views.as_ptr() as u64), 1);
    assert_eq!(call(1, 9, 2, 1, views.as_ptr() as u64), 3);
    let invalid = [0xff_u8];
    views[1] = StringView {
        address: invalid.as_ptr() as u64,
        length: 1,
    };
    assert_eq!(call(1, 9, 2, 64, views.as_ptr() as u64), 1);
    assert_eq!(output, [61; 64]);
    assert_eq!(metadata, [71; 2]);
}

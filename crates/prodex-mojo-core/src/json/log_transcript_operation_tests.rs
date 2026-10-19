use super::*;
use crate::json::JsonKind;

fn root() -> JsonNode<'static> {
    JsonNode {
        kind: JsonKind::Object,
        first_child: None,
        next_sibling: None,
        parent: None,
        key: "",
        text: "",
        raw_start: 0,
        raw_length: 2,
    }
}

#[test]
fn compiled_transcript_operation_preserves_typed_sources_and_presentation_shape() {
    for (kind, source) in [
        (TranscriptOperationKind::ToolProtocol, "tool"),
        (TranscriptOperationKind::McpProtocol, "mcp"),
        (TranscriptOperationKind::AgentProtocol, "agent"),
        (TranscriptOperationKind::TerminalStatus, "terminal"),
        (TranscriptOperationKind::ErrorStatus, "error"),
    ] {
        let values = [TranscriptOperationValue {
            scalar: "{}".to_string(),
            output: "{}".to_string(),
        }];
        assert_eq!(
            log_transcript_operation(&[root()], "{}", &values, kind),
            Ok(format!("[[\"{source}\",\"\"]]").into_bytes())
        );
        assert_eq!(
            log_transcript_operation(&[root()], "{}", &[], kind),
            Err(MojoError::InvalidInput)
        );
    }
}

#[test]
fn compiled_transcript_operation_validates_presentation_views_before_writing() {
    let nodes = ffi_nodes(&[root()], "{}").unwrap();
    let mut values = [StringView::from("{}"); 2];
    let mut output = [61_u8; 64];
    let mut metadata = [71_i64; 2];
    let mut call = |operation, count, address, size| unsafe {
        prodex_mojo_log_transcript_context_v1(
            1,
            operation,
            0,
            nodes.as_ptr() as u64,
            1,
            b"{}".as_ptr() as u64,
            2,
            address,
            count,
            0,
            output.as_mut_ptr() as u64,
            size,
            metadata.as_mut_ptr() as u64,
        )
    };
    assert_eq!(call(3, 1, values.as_ptr() as u64, 64), 1);
    assert_eq!(call(3, 2, values.as_ptr() as u64, 1), 3);
    let invalid = [0xff_u8];
    values[1] = StringView {
        address: invalid.as_ptr() as u64,
        length: 1,
    };
    assert_eq!(call(3, 2, values.as_ptr() as u64, 64), 1);
    assert_eq!(output, [61; 64]);
    assert_eq!(metadata, [71; 2]);
}

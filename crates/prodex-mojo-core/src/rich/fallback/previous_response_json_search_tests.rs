use super::*;
use crate::json::{JsonKind, JsonNode, ffi_nodes};

fn node<'a>(
    kind: JsonKind,
    key: &'a str,
    parent: Option<usize>,
    first_child: Option<usize>,
    next_sibling: Option<usize>,
) -> JsonNode<'a> {
    JsonNode {
        kind,
        key,
        text: "",
        parent,
        first_child,
        next_sibling,
        raw_start: 0,
        raw_length: 0,
    }
}

#[test]
fn compiled_previous_response_json_selects_first_matching_object_and_alias() {
    let nodes = [
        node(JsonKind::Array, "", None, Some(1), None),
        node(JsonKind::Object, "", Some(0), None, Some(2)),
        node(JsonKind::Object, "", Some(0), None, None),
    ];
    let rows = [
        PreviousResponseJsonObjectFields::default(),
        PreviousResponseJsonObjectFields {
            code: Some("previous_response_not_found"),
            detail: Some("selected-雪"),
            ..Default::default()
        },
        PreviousResponseJsonObjectFields {
            code: Some("previous_response_not_found"),
            message: Some("later"),
            ..Default::default()
        },
    ];
    assert_eq!(
        previous_response_json_search(&nodes, &rows, PreviousResponseJsonOperation::Message),
        Ok(PreviousResponseJsonSelection::Message {
            object_index: 1,
            field: PreviousResponseJsonTextField::Detail,
        }),
    );
    let mut rows = rows;
    rows[1].detail = None;
    assert_eq!(
        previous_response_json_search(&nodes, &rows, PreviousResponseJsonOperation::Message),
        Ok(PreviousResponseJsonSelection::DefaultNotFound),
    );
    rows[1].code = None;
    assert_eq!(
        previous_response_json_search(&nodes, &rows, PreviousResponseJsonOperation::Message),
        Ok(PreviousResponseJsonSelection::Message {
            object_index: 2,
            field: PreviousResponseJsonTextField::Message,
        }),
    );
}

#[test]
fn compiled_previous_response_json_preserves_invalid_id_and_no_match() {
    let nodes = [node(JsonKind::Object, "", None, None, None)];
    let invalid = format!(
        "invalid {}previous_response_id{}.",
        char::from(96),
        char::from(96)
    );
    let rows = [PreviousResponseJsonObjectFields {
        error_type: Some("invalid_request_error"),
        message: Some(&invalid),
        ..Default::default()
    }];
    assert_eq!(
        previous_response_json_search(&nodes, &rows, PreviousResponseJsonOperation::InvalidId),
        Ok(PreviousResponseJsonSelection::InvalidId(0)),
    );
    assert_eq!(
        previous_response_json_search(&nodes, &rows, PreviousResponseJsonOperation::Message),
        Ok(PreviousResponseJsonSelection::Message {
            object_index: 0,
            field: PreviousResponseJsonTextField::Message,
        }),
    );
    let rows = [PreviousResponseJsonObjectFields::default()];
    assert_eq!(
        previous_response_json_search(&nodes, &rows, PreviousResponseJsonOperation::InvalidId),
        Ok(PreviousResponseJsonSelection::None),
    );
    assert_eq!(
        previous_response_json_search(&nodes, &[], PreviousResponseJsonOperation::Message),
        Err(MojoError::InvalidInput),
    );
}

#[test]
fn compiled_previous_response_json_abi_rejects_invalid_rows_before_output_write() {
    let nodes = [node(JsonKind::Object, "", None, None, None)];
    let ffi = ffi_nodes(&nodes, "").unwrap();
    let mut words = [0_u64; 18];
    let mut output = [83_i64; 3];
    let source = ffi.as_ptr() as usize as u64;
    let rows = words.as_ptr() as usize as u64;
    let result = output.as_mut_ptr() as usize as u64;
    for (version, operation, source, count, fields, destination, expected) in [
        (2, 0, source, 1, rows, result, 4),
        (1, 2, source, 1, rows, result, 1),
        (1, 0, 0, 1, rows, result, 1),
        (1, 0, source, 0, rows, result, 1),
        (1, 0, source, 2049, rows, result, 1),
        (1, 0, source, 1, 0, result, 1),
        (1, 0, source, 1, rows, 0, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_mojo_previous_response_json_search_v1(
                    version,
                    operation,
                    source,
                    count,
                    fields,
                    destination,
                )
            },
            expected,
        );
        assert_eq!(output, [83; 3]);
    }
    words[17] = 2;
    assert_eq!(
        unsafe {
            prodex_mojo_previous_response_json_search_v1(
                1,
                0,
                source,
                1,
                words.as_ptr() as usize as u64,
                result,
            )
        },
        1,
    );
    assert_eq!(output, [83; 3]);
    let invalid = [JsonNode {
        parent: Some(0),
        ..nodes[0]
    }];
    let invalid_nodes = ffi_nodes(&invalid, "").unwrap();
    words[17] = 0;
    assert_eq!(
        unsafe {
            prodex_mojo_previous_response_json_search_v1(
                1,
                0,
                invalid_nodes.as_ptr() as usize as u64,
                1,
                words.as_ptr() as usize as u64,
                result,
            )
        },
        1,
    );
    assert_eq!(output, [83; 3]);
}

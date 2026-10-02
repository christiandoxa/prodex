use super::*;

/// A byte span inside a string node selected by the response metadata kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonStringSpan {
    pub node: usize,
    pub start: usize,
    pub length: usize,
}

/// The response IDs, event/turn metadata, and token usage selected by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponseMetadataPlan {
    pub response_ids: [Option<usize>; 3],
    pub event_type: Option<JsonStringSpan>,
    pub turn_state: Option<JsonStringSpan>,
    pub headers_turn_state: Option<JsonStringSpan>,
    pub token_usage_present: bool,
    pub token_usage_nodes: [Option<usize>; 4],
}

fn validated_string_span(
    node: i64,
    start: i64,
    length: i64,
    nodes: &[JsonNode<'_>],
) -> Result<Option<JsonStringSpan>, MojoError> {
    let Some(node) = validated_optional_node_index(node, nodes.len())? else {
        return if start == -1 && length == -1 {
            Ok(None)
        } else {
            Err(MojoError::InvalidOutput)
        };
    };
    if !matches!(nodes[node].kind, JsonKind::String) || start < 0 || length <= 0 {
        return Err(MojoError::InvalidOutput);
    }
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let length = usize::try_from(length).map_err(|_| MojoError::InvalidOutput)?;
    nodes[node]
        .text
        .get(start..start.checked_add(length).ok_or(MojoError::InvalidOutput)?)
        .ok_or(MojoError::InvalidOutput)?;
    Ok(Some(JsonStringSpan {
        node,
        start,
        length,
    }))
}

/// Extract response metadata from a Serde-built JSON tree.
///
/// `number_texts` contains canonical Serde number strings by node index; those
/// strings cross the ABI as borrowed scalar text while all decisions stay in Mojo.
pub fn runtime_response_metadata(
    nodes: &[JsonNode<'_>],
    number_texts: &[Option<String>],
) -> Result<RuntimeResponseMetadataPlan, MojoError> {
    let raw = "";
    let input = ffi_nodes_with_text(nodes, raw, Some(number_texts))?;
    let mut output = [-1_i64; 18];
    status(unsafe {
        prodex_runtime_response_metadata_v1(
            1,
            input.as_ptr() as u64,
            signed(input.len())?,
            raw.as_ptr() as u64,
            0,
            output.as_mut_ptr() as u64,
        )
    })?;

    let response_id_count = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if response_id_count > 3 {
        return Err(MojoError::InvalidOutput);
    }
    let mut response_ids = [None; 3];
    for (slot, output_slot) in response_ids.iter_mut().enumerate() {
        let node = validated_optional_node_index(output[1 + slot], nodes.len())?;
        if (slot < response_id_count) != node.is_some()
            || node.is_some_and(|node| !matches!(nodes[node].kind, JsonKind::String))
        {
            return Err(MojoError::InvalidOutput);
        }
        *output_slot = node;
    }

    let event_type = validated_string_span(output[4], output[5], output[6], nodes)?;
    let turn_state = validated_string_span(output[7], output[8], output[9], nodes)?;
    let headers_turn_state = validated_string_span(output[10], output[11], output[12], nodes)?;
    let token_usage_present = match output[13] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut token_usage_nodes = [None; 4];
    for (slot, output_slot) in token_usage_nodes.iter_mut().enumerate() {
        let node = validated_optional_node_index(output[14 + slot], nodes.len())?;
        if node.is_some_and(|node| !matches!(nodes[node].kind, JsonKind::Number)) {
            return Err(MojoError::InvalidOutput);
        }
        *output_slot = node;
    }
    if token_usage_present != token_usage_nodes.iter().any(Option::is_some) {
        return Err(MojoError::InvalidOutput);
    }

    Ok(RuntimeResponseMetadataPlan {
        response_ids,
        event_type,
        turn_state,
        headers_turn_state,
        token_usage_present,
        token_usage_nodes,
    })
}

#[cfg(test)]
mod response_metadata_tests {
    use super::super::{JsonKind, JsonNode};
    use super::{JsonStringSpan, RuntimeResponseMetadataPlan, runtime_response_metadata};

    #[test]
    fn runtime_response_metadata_abi_keeps_precedence_trim_and_usage() {
        let nodes = [
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(1),
                next_sibling: None,
                parent: None,
                key: "",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: Some(2),
                parent: Some(0),
                key: "type",
                text: " response.completed ",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(3),
                next_sibling: Some(14),
                parent: Some(0),
                key: "response",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: Some(4),
                parent: Some(2),
                key: "id",
                text: "resp-a",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(5),
                next_sibling: Some(7),
                parent: Some(2),
                key: "headers",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Array,
                first_child: Some(6),
                next_sibling: None,
                parent: Some(4),
                key: "X-CODEX-TURN-STATE",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: None,
                parent: Some(5),
                key: "",
                text: "\t turn-a \n",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(8),
                next_sibling: None,
                parent: Some(2),
                key: "usage",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Number,
                first_child: None,
                next_sibling: Some(9),
                parent: Some(7),
                key: "input_tokens",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(10),
                next_sibling: Some(11),
                parent: Some(7),
                key: "input_tokens_details",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Number,
                first_child: None,
                next_sibling: None,
                parent: Some(9),
                key: "cached_tokens",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Number,
                first_child: None,
                next_sibling: Some(12),
                parent: Some(7),
                key: "output_tokens",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Object,
                first_child: Some(13),
                next_sibling: None,
                parent: Some(7),
                key: "output_tokens_details",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::Number,
                first_child: None,
                next_sibling: None,
                parent: Some(12),
                key: "reasoning_tokens",
                text: "",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: Some(15),
                parent: Some(0),
                key: "response_id",
                text: "resp-a",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: Some(16),
                parent: Some(0),
                key: "object",
                text: "response",
                raw_start: 0,
                raw_length: 0,
            },
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                next_sibling: None,
                parent: Some(0),
                key: "id",
                text: "resp-a",
                raw_start: 0,
                raw_length: 0,
            },
        ];
        let mut number_texts = vec![None; nodes.len()];
        number_texts[8] = Some("41".to_string());
        number_texts[10] = Some("11".to_string());
        number_texts[11] = Some("9".to_string());
        number_texts[13] = Some("3".to_string());

        assert_eq!(
            runtime_response_metadata(&nodes, &number_texts),
            Ok(RuntimeResponseMetadataPlan {
                response_ids: [Some(3), None, None],
                event_type: Some(JsonStringSpan {
                    node: 1,
                    start: 1,
                    length: 18
                }),
                turn_state: Some(JsonStringSpan {
                    node: 6,
                    start: 2,
                    length: 6
                }),
                headers_turn_state: None,
                token_usage_present: true,
                token_usage_nodes: [Some(8), Some(10), Some(11), Some(13)],
            })
        );

        let empty_object = [JsonNode {
            kind: JsonKind::Object,
            first_child: None,
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 0,
        }];
        assert_eq!(
            runtime_response_metadata(&empty_object, &[None]),
            Ok(RuntimeResponseMetadataPlan {
                response_ids: [None; 3],
                event_type: None,
                turn_state: None,
                headers_turn_state: None,
                token_usage_present: false,
                token_usage_nodes: [None; 4],
            })
        );
    }
}

use super::{JsonNode, StringView, ffi_nodes, transform_json_with_views};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_log_payload_projection_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        serialized: u64,
        serialized_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Serde presentation of one payload node, in borrowed-tree preorder.
#[derive(Debug, Clone)]
pub struct LogPayloadJsonPresentation {
    pub pretty: String,
    pub parsed_string_json: Option<String>,
}

/// Project a payload into `[kind, text]` display records. Kind 0 wraps text,
/// 1 emits a literal label, and 2 wraps an indented body. Mojo owns the choice
/// between a string's parsed JSON presentation and its original text.
pub fn log_payload_projection(
    nodes: &[JsonNode<'_>],
    raw: &str,
    serialized: &[LogPayloadJsonPresentation],
) -> Result<Vec<u8>, MojoError> {
    if serialized.len() != nodes.len() {
        return Err(MojoError::InvalidInput);
    }
    let input = ffi_nodes(nodes, raw)?;
    let mut views = serialized
        .iter()
        .flat_map(|value| {
            [
                value.pretty.as_str().into(),
                value
                    .parsed_string_json
                    .as_deref()
                    .unwrap_or_default()
                    .into(),
            ]
        })
        .collect::<Vec<StringView>>();
    transform_json_with_views(
        &input,
        raw,
        0,
        false,
        prodex_mojo_log_payload_projection_v1,
        &mut views,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::JsonKind;

    fn model_nodes(raw: &str) -> [JsonNode<'static>; 2] {
        [
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
                kind: JsonKind::String,
                first_child: None,
                next_sibling: None,
                parent: Some(0),
                key: "model",
                text: "月🙂",
                raw_start: 9,
                raw_length: raw.len() - 10,
            },
        ]
    }

    #[test]
    fn payload_projection_calls_compiled_mojo_and_validates_host_shapes() {
        let raw = r#"{"model":"月🙂"}"#;
        let nodes = model_nodes(raw);
        let serialized = vec![
            LogPayloadJsonPresentation {
                pretty: raw.to_string(),
                parsed_string_json: None,
            };
            2
        ];
        assert_eq!(
            log_payload_projection(&nodes, raw, &serialized),
            Ok(r#"[[0,"model=月🙂"]]"#.as_bytes().to_vec())
        );
        assert_eq!(
            log_payload_projection(&nodes, raw, &serialized[..1]),
            Err(MojoError::InvalidInput)
        );
        let mut invalid = nodes;
        invalid[1].next_sibling = Some(1);
        assert_eq!(
            log_payload_projection(&invalid, raw, &serialized),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn payload_projection_abi_rejects_invalid_views_versions_and_capacity() {
        let raw = r#"{"model":"月🙂"}"#;
        let nodes = ffi_nodes(&model_nodes(raw), raw).unwrap();
        let views = [StringView::from(raw); 4];
        let mut output = [0xA5_u8; 64];
        let mut metadata = [-7_i64; 2];
        let invoke = |abi,
                      operation,
                      flag,
                      count,
                      raw_address,
                      serialized_count,
                      measuring,
                      capacity,
                      output: &mut [u8],
                      metadata: &mut [i64; 2]| unsafe {
            prodex_mojo_log_payload_projection_v1(
                abi,
                operation,
                flag,
                nodes.as_ptr() as u64,
                count,
                raw_address,
                raw.len() as i64,
                views.as_ptr() as u64,
                serialized_count,
                measuring,
                output.as_mut_ptr() as u64,
                capacity,
                metadata.as_mut_ptr() as u64,
            )
        };
        let raw_address = raw.as_ptr() as u64;
        for (abi, op, flag, count, address, view_count, measure, capacity, expected) in [
            (2, 0, 0, 2, raw_address, 4, 0, 64, 4),
            (1, 1, 0, 2, raw_address, 4, 0, 64, 1),
            (1, 0, 1, 2, raw_address, 4, 0, 64, 1),
            (1, 0, 0, -1, raw_address, 4, 0, 64, 1),
            (1, 0, 0, 2, 0, 4, 0, 64, 1),
            (1, 0, 0, 2, raw_address, 3, 0, 64, 1),
            (1, 0, 0, 2, raw_address, 4, 2, 64, 1),
            (1, 0, 0, 2, raw_address, 4, 1, 0, 1),
            (1, 0, 0, 2, raw_address, 4, 0, -1, 1),
        ] {
            assert_eq!(
                invoke(
                    abi,
                    op,
                    flag,
                    count,
                    address,
                    view_count,
                    measure,
                    capacity,
                    &mut output,
                    &mut metadata
                ),
                expected
            );
            assert_eq!(metadata, [-7; 2]);
            assert_eq!(output, [0xA5; 64]);
        }
        assert_eq!(
            invoke(1, 0, 0, 2, raw_address, 4, 0, 1, &mut output, &mut metadata),
            3
        );
        assert_eq!(metadata, [-7; 2]);
        assert_eq!(&output[1..], &[0xA5; 63]);
    }

    #[test]
    fn payload_projection_abi_rejects_invalid_serialized_utf8_before_output() {
        let raw = r#"{"model":"月🙂"}"#;
        let nodes = ffi_nodes(&model_nodes(raw), raw).unwrap();
        let invalid = [0xFF_u8];
        let mut views = [StringView::from(raw); 4];
        views[3] = StringView {
            address: invalid.as_ptr() as u64,
            length: 1,
        };
        let mut output = [0xA5_u8; 64];
        let mut metadata = [-7_i64; 2];
        assert_eq!(
            unsafe {
                prodex_mojo_log_payload_projection_v1(
                    1,
                    0,
                    0,
                    nodes.as_ptr() as u64,
                    2,
                    raw.as_ptr() as u64,
                    raw.len() as i64,
                    views.as_ptr() as u64,
                    4,
                    0,
                    output.as_mut_ptr() as u64,
                    64,
                    metadata.as_mut_ptr() as u64,
                )
            },
            1
        );
        assert_eq!(metadata, [-7; 2]);
        assert_eq!(output, [0xA5; 64]);
    }
}

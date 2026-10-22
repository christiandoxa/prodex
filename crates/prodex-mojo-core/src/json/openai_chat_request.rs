//! Compiled chat-request mapping and ordered shape validation.
use super::{JsonNode, prodex_mojo_openai_chat_request_v1, transform_json};
use crate::MojoError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAiChatRequestTransform {
    Body(Vec<u8>),
    Rejected(String),
}

pub fn transform_openai_chat_request(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<OpenAiChatRequestTransform, MojoError> {
    let output = transform_json(nodes, raw, 0, false, prodex_mojo_openai_chat_request_v1)?
        .ok_or(MojoError::InvalidOutput)?;
    let Some((&tag, payload)) = output.split_first() else {
        return Err(MojoError::InvalidOutput);
    };
    match tag {
        b'S' => Ok(OpenAiChatRequestTransform::Body(payload.to_vec())),
        b'E' => String::from_utf8(payload.to_vec())
            .map(OpenAiChatRequestTransform::Rejected)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Validate the optional public fields in a borrowed request tree.
/// Mojo owns error precedence, Unicode blank-model checks and diagnostic text.
pub fn chat_compatible_request_shape_error(
    nodes: &[JsonNode<'_>],
    provider: &str,
) -> Result<Option<String>, MojoError> {
    transform_json(
        nodes,
        provider,
        1,
        false,
        prodex_mojo_openai_chat_request_v1,
    )?
    .map(|output| {
        let payload = output.strip_prefix(b"E").ok_or(MojoError::InvalidOutput)?;
        String::from_utf8(payload.to_vec()).map_err(|_| MojoError::InvalidOutput)
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{JsonKind, ffi_nodes};

    fn request(model: &str) -> [JsonNode<'_>; 2] {
        let root = JsonNode {
            kind: JsonKind::Object,
            first_child: Some(1),
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 0,
        };
        [
            root,
            JsonNode {
                kind: JsonKind::String,
                first_child: None,
                parent: Some(0),
                key: "model",
                text: model,
                ..root
            },
        ]
    }

    #[test]
    fn compiled_shape_preserves_unicode_model_and_missing_fields() {
        for (model, error) in [
            (" 東京🧭 ", None),
            ("\u{200b}", None),
            ("\0", None),
            (
                " \t\u{85}\u{a0}\u{2003}\u{3000}",
                Some("Gemini model must not be empty"),
            ),
            ("", Some("Gemini model must not be empty")),
        ] {
            assert_eq!(
                chat_compatible_request_shape_error(&request(model), "Gemini").unwrap(),
                error.map(str::to_string),
            );
        }
        let mut nodes = request("");
        nodes[0].first_child = None;
        assert_eq!(
            chat_compatible_request_shape_error(&nodes, "Gemini"),
            Ok(None)
        );
        nodes[0].first_child = Some(1);
        nodes[1].kind = JsonKind::Null;
        assert_eq!(
            chat_compatible_request_shape_error(&nodes, "Gemini").unwrap(),
            Some("Gemini model must be a string".to_string()),
        );
    }

    #[test]
    fn compiled_shape_rejects_malformed_context_and_tree() {
        let mut nodes = request("model");
        nodes[0].first_child = Some(99);
        assert_eq!(
            chat_compatible_request_shape_error(&nodes, "Gemini"),
            Err(MojoError::InvalidInput)
        );
        nodes[0].first_child = Some(0);
        assert_eq!(
            chat_compatible_request_shape_error(&nodes, "Gemini"),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn compiled_shape_abi_checks_capacity_version_and_alignment_before_writes() {
        let nodes = ffi_nodes(&request(""), "Gemini").unwrap();
        let mut scratch = [0_u64; 8];
        let mut output = [93_u8; 64];
        let mut metadata = [71_i64; 2];
        let source = nodes.as_ptr() as u64;
        let workspace = scratch.as_mut_ptr() as u64;
        let target = output.as_mut_ptr() as u64;
        let meta = metadata.as_mut_ptr() as u64;
        for (version, operation, flag, input, dest_meta, capacity, expected) in [
            (2, 1, 0, source, meta, 64, 4),
            (1, 2, 0, source, meta, 64, 1),
            (1, 1, 1, source, meta, 64, 1),
            (1, 1, 0, 0, meta, 64, 1),
            (1, 1, 0, source + 1, meta, 64, 1),
            (1, 1, 0, source, meta + 1, 64, 1),
            (1, 1, 0, source, meta, 1, 3),
        ] {
            let status = unsafe {
                prodex_mojo_openai_chat_request_v1(
                    version,
                    operation,
                    flag,
                    input,
                    2,
                    b"Gemini".as_ptr() as u64,
                    6,
                    workspace,
                    2,
                    0,
                    target,
                    capacity,
                    dest_meta,
                )
            };
            assert_eq!(status, expected);
            assert_eq!(output, [93; 64]);
            assert_eq!(metadata, [71; 2]);
        }
    }
}

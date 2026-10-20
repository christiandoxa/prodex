//! Typed JSON-tree adapter for compiled Gemini bridge value collection.
//! Rust allocates ABI storage and decodes the returned JSON; Mojo alone
//! traverses values and determines which strings to emit.

use super::{JsonNode, transform_json_with_scratch};
use crate::MojoError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiValueOperation {
    Strings = 0,
    Paths = 1,
    InputTexts = 2,
    StreamError = 3,
    Boolean = 4,
    MediaObjectIndices = 5,
    ThoughtSignature = 6,
}

unsafe extern "C" {
    fn prodex_mojo_gemini_value_scalar_v1(
        abi: i64,
        operation: i64,
        text_address: u64,
        text_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_gemini_value_policy_v1(
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

pub fn gemini_value_projection(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: GeminiValueOperation,
) -> Result<Vec<u8>, MojoError> {
    transform_json_with_scratch(
        nodes,
        raw,
        operation as i64,
        false,
        prodex_mojo_gemini_value_policy_v1,
        1,
    )?
    .ok_or(MojoError::InvalidOutput)
}

fn gemini_value_scalar(input: &str, operation: i64) -> Result<[u64; 5], MojoError> {
    let mut output = [u64::MAX; 5];
    match unsafe {
        prodex_mojo_gemini_value_scalar_v1(
            1,
            operation,
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    } {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Exact, case-sensitive Gemini-3 model substring requirement owned by Mojo.
pub fn gemini_model_requires_tool_thought_signature(model: &str) -> Result<bool, MojoError> {
    let [flag, a, b, c, d] = gemini_value_scalar(model, 2)?;
    if [a, b, c, d] != [0; 4] {
        return Err(MojoError::InvalidOutput);
    }
    match flag {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Return Mojo-selected UTF-8 slices for a command-specific tool and pattern.
pub fn gemini_tool_pattern_parts(value: &str) -> Result<Option<(&str, &str)>, MojoError> {
    let output = gemini_value_scalar(value, 0)?;
    if output == [0; 5] {
        return Ok(None);
    }
    if output[0] != 1 {
        return Err(MojoError::InvalidOutput);
    }
    let bounds = output[1..]
        .iter()
        .copied()
        .map(|index| usize::try_from(index).map_err(|_| MojoError::InvalidOutput))
        .collect::<Result<Vec<_>, _>>()?;
    let [tool_start, tool_end, pattern_start, pattern_end] = bounds.as_slice() else {
        return Err(MojoError::InvalidOutput);
    };
    if *tool_start >= *tool_end
        || *tool_end > *pattern_start
        || *pattern_start >= *pattern_end
        || *pattern_end > value.len()
    {
        return Err(MojoError::InvalidOutput);
    }
    let tool = value
        .get(*tool_start..*tool_end)
        .ok_or(MojoError::InvalidOutput)?;
    let pattern = value
        .get(*pattern_start..*pattern_end)
        .ok_or(MojoError::InvalidOutput)?;
    Ok(Some((tool, pattern)))
}

/// Exact, case-sensitive names excluded from a Gemini workspace context scan.
pub fn gemini_skip_context_path_name(name: &str) -> Result<bool, MojoError> {
    let [flag, a, b, c, d] = gemini_value_scalar(name, 1)?;
    if [a, b, c, d] != [0; 4] {
        return Err(MojoError::InvalidOutput);
    }
    match flag {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
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
    fn compiled_gemini_media_indices_preserve_content_only_order() {
        let nodes = [
            node(JsonKind::Object, "", "", None, Some(1), None),
            node(
                JsonKind::String,
                "ignored",
                "not media",
                Some(0),
                None,
                Some(2),
            ),
            node(JsonKind::Array, "content", "", Some(0), Some(3), None),
            node(JsonKind::Object, "", "", Some(2), Some(4), Some(5)),
            node(JsonKind::String, "type", "input_file", Some(3), None, None),
            node(JsonKind::Object, "", "", Some(2), Some(6), None),
            node(JsonKind::String, "type", "input_video", Some(5), None, None),
        ];
        assert_eq!(
            gemini_value_projection(&nodes, "", GeminiValueOperation::MediaObjectIndices).unwrap(),
            b"[0,3,5]",
        );
        let scalar = node(JsonKind::String, "", "scalar", None, None, None);
        assert_eq!(
            gemini_value_projection(&[scalar], "", GeminiValueOperation::MediaObjectIndices)
                .unwrap(),
            b"[]",
        );
    }

    #[test]
    fn compiled_gemini_media_indices_walk_deep_flat_tree_without_recursion() {
        let mut nodes = Vec::new();
        for index in 0_usize..4096 {
            nodes.push(node(
                JsonKind::Object,
                if index == 0 { "" } else { "content" },
                "",
                index.checked_sub(1),
                Some(index + 1),
                None,
            ));
        }
        nodes.push(node(
            JsonKind::Object,
            "content",
            "",
            Some(4095),
            None,
            None,
        ));
        let output =
            gemini_value_projection(&nodes, "", GeminiValueOperation::MediaObjectIndices).unwrap();
        let emitted = std::str::from_utf8(&output).unwrap();
        assert!(emitted.starts_with("[0,1,2,3,"));
        assert!(emitted.ends_with(",4094,4095,4096]"));
        assert_eq!(emitted.bytes().filter(|byte| *byte == b',').count(), 4096);
    }

    #[test]
    fn compiled_gemini_thought_signature_keeps_first_present_priority_and_unicode() {
        let mut nodes = [
            node(JsonKind::Object, "", "", None, Some(1), None),
            node(
                JsonKind::Null,
                "thoughtSignature",
                "",
                Some(0),
                None,
                Some(2),
            ),
            node(
                JsonKind::String,
                "thought_signature",
                "fallback",
                Some(0),
                None,
                None,
            ),
        ];
        let op = GeminiValueOperation::ThoughtSignature;
        assert_eq!(
            gemini_value_projection(&nodes, "", op),
            Ok(b"null".to_vec())
        );
        nodes[1].kind = JsonKind::String;
        nodes[1].text = " signed \u{3000} ";
        assert_eq!(
            gemini_value_projection(&nodes, "", op),
            Ok("\" signed \u{3000} \"".as_bytes().to_vec()),
        );
        nodes[1].text = " \u{3000} ";
        assert_eq!(
            gemini_value_projection(&nodes, "", op),
            Ok(b"null".to_vec())
        );
        nodes[0].first_child = None;
        assert_eq!(
            gemini_value_projection(&nodes[..1], "", op),
            Ok(b"null".to_vec())
        );
        let scalar = node(JsonKind::String, "", "not-object", None, None, None);
        assert_eq!(
            gemini_value_projection(&[scalar], "", op),
            Ok(b"null".to_vec())
        );
    }

    #[test]
    fn compiled_gemini_model_signature_classifies_substrings_without_rust_parser() {
        for (model, expected) in [
            ("gemini-3", true),
            ("gemini-3.5-flash", true),
            ("prefix-gemini-3", true),
            ("東京-gemini-3-雪", true),
            ("GEMINI-3", false),
            ("gemini-2.5", false),
            ("gemini-30", true),
            ("gemini-3suffix", true),
            ("", false),
        ] {
            assert_eq!(
                gemini_model_requires_tool_thought_signature(model),
                Ok(expected)
            );
        }
    }

    #[test]
    fn compiled_gemini_json_boolean_projector_accepts_non_object_trees() {
        let mut root = node(JsonKind::True, "", "", None, None, None);
        assert_eq!(
            gemini_value_projection(&[root], "true", GeminiValueOperation::Boolean),
            Ok(b"true".to_vec())
        );
        root.kind = JsonKind::False;
        assert_eq!(
            gemini_value_projection(&[root], "false", GeminiValueOperation::Boolean),
            Ok(b"false".to_vec())
        );
        root.kind = JsonKind::String;
        root.text = " \u{2003} YES \u{2003}";
        assert_eq!(
            gemini_value_projection(&[root], "", GeminiValueOperation::Boolean),
            Ok(b"true".to_vec())
        );
        root.text = "other";
        assert_eq!(
            gemini_value_projection(&[root], "", GeminiValueOperation::Boolean),
            Ok(b"null".to_vec())
        );
        root.kind = JsonKind::Number;
        root.text = "";
        for (raw, expected) in [
            ("-9223372036854775808", "true"),
            ("9223372036854775807", "true"),
            ("0", "false"),
            ("18446744073709551615", "false"),
            ("1.0", "false"),
            ("-0.0", "false"),
        ] {
            root.raw_start = 0;
            root.raw_length = raw.len();
            assert_eq!(
                gemini_value_projection(&[root], raw, GeminiValueOperation::Boolean),
                Ok(expected.as_bytes().to_vec()),
                "{raw}",
            );
        }
    }

    #[test]
    fn compiled_gemini_stream_error_reads_serde_number_lexemes_and_missing_errors() {
        let mut nodes = [
            node(JsonKind::Object, "", "", None, Some(1), None),
            node(JsonKind::Object, "error", "", Some(0), Some(2), None),
            node(JsonKind::Number, "status", "", Some(1), None, Some(3)),
            node(JsonKind::String, "message", "🚀", Some(1), None, None),
        ];
        nodes[2].raw_length = 3;
        let op = GeminiValueOperation::StreamError;
        assert_eq!(
            gemini_value_projection(&nodes, "404", op).unwrap(),
            "[\"404\",\"🚀\"]".as_bytes(),
        );
        nodes[2].raw_length = 4;
        assert_eq!(
            gemini_value_projection(&nodes, "1.25", op).unwrap(),
            "[\"provider_stream_error\",\"🚀\"]".as_bytes(),
        );
        nodes[2].raw_length = 20;
        assert_eq!(
            gemini_value_projection(&nodes, "18446744073709551615", op).unwrap(),
            "[\"provider_stream_error\",\"🚀\"]".as_bytes(),
        );
        nodes[2].kind = JsonKind::String;
        nodes[2].raw_length = 0;
        nodes[2].text = "";
        assert_eq!(
            gemini_value_projection(&nodes, "", op).unwrap(),
            "[\"\",\"🚀\"]".as_bytes(),
        );
        nodes[0].first_child = None;
        assert_eq!(
            gemini_value_projection(&nodes[..1], "", op),
            Ok(b"null".to_vec())
        );
    }

    #[test]
    fn compiled_gemini_scalar_tool_parts_preserves_unicode_whitespace_and_last_close() {
        for (input, expected) in [
            ("tool(regex)", Some(("tool", "regex"))),
            (
                "  \u{2003}tool \u{2003}( \u{2003}東京.*\u{2003} ) extra",
                Some(("tool", "東京.*")),
            ),
            ("func(a(b)c)", Some(("func", "a(b)c"))),
            ("func(a)b(c) tail", Some(("func", "a)b(c"))),
            (
                "prefix(before) trailing )",
                Some(("prefix", "before) trailing")),
            ),
            ("()", None),
            ("tool()", None),
            ("(pattern)", None),
            ("tool(pattern", None),
            ("tool)pattern(", None),
            (" \u{2003} ", None),
            ("", None),
            ("🚀(a)", Some(("🚀", "a"))),
        ] {
            assert_eq!(gemini_tool_pattern_parts(input), Ok(expected), "{input:?}");
        }
    }

    #[test]
    fn compiled_gemini_scalar_context_names_use_exact_case() {
        for (name, excluded) in [
            (".git", true),
            ("node_modules", true),
            ("target", true),
            ("dist", true),
            ("build", true),
            ("__pycache__", true),
            ("vendor", true),
            ("GIT", false),
            (".GIT", false),
            ("node-modules", false),
            (" target", false),
            ("vendor/", false),
            ("", false),
            ("🚀", false),
        ] {
            assert_eq!(gemini_skip_context_path_name(name), Ok(excluded));
        }
    }

    #[test]
    fn compiled_gemini_scalar_abi_rejects_bad_shape_before_output_writes() {
        let mut output = [71_u64; 5];
        let call = |abi, operation, address, length, output_address| unsafe {
            prodex_mojo_gemini_value_scalar_v1(abi, operation, address, length, output_address)
        };
        let address = "tool(a)".as_ptr() as usize as u64;
        let result_address = output.as_mut_ptr() as usize as u64;
        for (abi, operation, ptr, length, out, expected) in [
            (2, 0, address, 7, result_address, 4),
            (1, -1, address, 7, result_address, 1),
            (1, 3, address, 7, result_address, 1),
            (1, 0, 0, 7, result_address, 1),
            (1, 0, address, -1, result_address, 1),
            (1, 0, address, 7, 0, 1),
        ] {
            assert_eq!(call(abi, operation, ptr, length, out), expected);
            assert_eq!(output, [71; 5]);
        }
        let invalid_utf8 = [255_u8];
        assert_eq!(
            call(
                1,
                0,
                invalid_utf8.as_ptr() as usize as u64,
                1,
                result_address
            ),
            1
        );
        assert_eq!(output, [71; 5]);
    }

    #[test]
    fn compiled_gemini_value_walks_nested_arrays_and_selected_object_content() {
        let nodes = [
            node(JsonKind::Object, "", "", None, Some(1), None),
            node(JsonKind::Array, "value", "", Some(0), Some(2), None),
            node(JsonKind::String, "", "first", Some(1), None, Some(3)),
            node(JsonKind::Object, "", "", Some(1), Some(4), Some(7)),
            node(JsonKind::String, "text", "name", Some(3), None, Some(5)),
            node(JsonKind::Array, "content", "", Some(3), Some(6), None),
            node(JsonKind::String, "", "  nested 🚀  ", Some(5), None, None),
            node(JsonKind::String, "", "last", Some(1), None, None),
        ];
        assert_eq!(
            gemini_value_projection(&nodes, "", GeminiValueOperation::Strings).unwrap(),
            b"[\"first\",\"last\"]"
        );
        assert_eq!(
            gemini_value_projection(&nodes, "", GeminiValueOperation::Paths).unwrap(),
            b"[\"first\",\"last\"]"
        );
        assert_eq!(
            gemini_value_projection(&nodes, "", GeminiValueOperation::InputTexts).unwrap(),
            "[\"first\",\"name\",\"  nested 🚀  \",\"last\"]".as_bytes()
        );
    }

    #[test]
    fn compiled_gemini_value_abi_rejects_versions_and_invalid_shapes_without_writing() {
        let nodes = [
            node(JsonKind::Object, "", "", None, Some(1), None),
            node(JsonKind::String, "value", "hello", Some(0), None, None),
        ];
        let ffi = ffi_nodes(&nodes, "").unwrap();
        let mut scratch = [0_u64; 4];
        let mut metadata = [99_i64; 2];
        for (version, operation, scratch_count, expected) in
            [(2, 0, 4, 4), (1, -1, 4, 1), (1, 7, 4, 1), (1, 0, 1, 1)]
        {
            let result = unsafe {
                prodex_mojo_gemini_value_policy_v1(
                    version,
                    operation,
                    0,
                    ffi.as_ptr() as u64,
                    2,
                    0,
                    0,
                    scratch.as_mut_ptr() as u64,
                    scratch_count,
                    1,
                    0,
                    0,
                    metadata.as_mut_ptr() as u64,
                )
            };
            assert_eq!(result, expected);
            assert_eq!(metadata, [99; 2]);
        }
        assert_eq!(
            unsafe {
                prodex_mojo_gemini_value_policy_v1(
                    1,
                    0,
                    0,
                    ffi.as_ptr() as u64,
                    2,
                    0,
                    0,
                    scratch.as_mut_ptr() as u64,
                    4,
                    1,
                    0,
                    0,
                    0,
                )
            },
            1
        );
        assert_eq!(metadata, [99; 2]);
    }
}

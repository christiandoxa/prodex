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
}

unsafe extern "C" {
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
            [(2, 0, 4, 4), (1, -1, 4, 1), (1, 3, 4, 1), (1, 0, 1, 1)]
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

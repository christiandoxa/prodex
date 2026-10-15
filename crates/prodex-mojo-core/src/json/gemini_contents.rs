//! Gemini content normalization through the shared measured JSON ABI.
use super::{JsonNode, transform_json};
use crate::MojoError;

/// A semantic pass over Gemini contents; Serde retains wire ownership.
#[derive(Clone, Copy, Debug)]
#[repr(i64)]
pub enum GeminiContentOperation {
    Coalesce = 0,
}

unsafe extern "C" {
    fn prodex_mojo_gemini_content_hardening_v1(
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

pub fn gemini_content_hardening(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: GeminiContentOperation,
) -> Result<Vec<u8>, MojoError> {
    transform_json(
        nodes,
        raw,
        operation as i64,
        false,
        prodex_mojo_gemini_content_hardening_v1,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::JsonKind;

    #[test]
    fn gemini_content_abi_empty_and_invalid_tree() {
        let mut root = JsonNode {
            kind: JsonKind::Array,
            first_child: None,
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 2,
        };
        assert_eq!(
            gemini_content_hardening(&[root], "[]", GeminiContentOperation::Coalesce).unwrap(),
            b"[]"
        );
        root.first_child = Some(0);
        assert_eq!(
            gemini_content_hardening(&[root], "[]", GeminiContentOperation::Coalesce),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn gemini_content_abi_rejects_bad_version_without_output_access() {
        let status = unsafe {
            prodex_mojo_gemini_content_hardening_v1(2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0)
        };
        assert_eq!(status, 4);
    }
}

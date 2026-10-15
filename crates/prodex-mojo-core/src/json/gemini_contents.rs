//! Gemini content normalization through the shared measured JSON ABI.
use super::{JsonNode, transform_json_with_scratch};
use crate::MojoError;

/// A semantic pass over Gemini contents; Serde retains wire ownership.
#[derive(Clone, Copy, Debug)]
#[repr(i64)]
pub enum GeminiContentOperation {
    Coalesce = 0,
    PairMissingResults = 1,
    RepairOrphanResponses = 2,
    EnforceRoleShape = 3,
    OrderResponses = 4,
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
    transform_json_with_scratch(
        nodes,
        raw,
        operation as i64,
        false,
        prodex_mojo_gemini_content_hardening_v1,
        if matches!(operation, GeminiContentOperation::OrderResponses) {
            4
        } else {
            1
        },
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
        for operation in [
            GeminiContentOperation::Coalesce,
            GeminiContentOperation::PairMissingResults,
            GeminiContentOperation::RepairOrphanResponses,
            GeminiContentOperation::EnforceRoleShape,
            GeminiContentOperation::OrderResponses,
        ] {
            assert_eq!(
                gemini_content_hardening(&[root], "[]", operation).unwrap(),
                b"[]"
            );
        }
        root.first_child = Some(0);
        assert_eq!(
            gemini_content_hardening(&[root], "[]", GeminiContentOperation::Coalesce),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn gemini_order_abi_validates_scratch_and_output_capacity() {
        let root = JsonNode {
            kind: JsonKind::Array,
            first_child: None,
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 2,
        };
        let input = crate::json::ffi_nodes(&[root], "[]").unwrap();
        let mut scratch = [0_u64; 8];
        let mut output = [99_u8; 2];
        let mut metadata = [77_i64; 2];
        let mut call = |scratch_count, capacity| unsafe {
            prodex_mojo_gemini_content_hardening_v1(
                1,
                4,
                0,
                input.as_ptr() as u64,
                1,
                b"[]".as_ptr() as u64,
                2,
                scratch.as_mut_ptr() as u64,
                scratch_count,
                0,
                output.as_mut_ptr() as u64,
                capacity,
                metadata.as_mut_ptr() as u64,
            )
        };
        assert_eq!(call(3, 2), 1);
        assert_eq!(call(4, 1), 3);
        assert_eq!(metadata, [77; 2], "invalid output is never published");
    }

    #[test]
    fn gemini_content_abi_rejects_bad_version_without_output_access() {
        let status = unsafe {
            prodex_mojo_gemini_content_hardening_v1(2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0)
        };
        assert_eq!(status, 4);
    }
}

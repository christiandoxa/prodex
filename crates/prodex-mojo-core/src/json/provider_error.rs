use super::{JsonNode, transform_json};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_provider_error_message_v1(
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

/// Extracts the first nonblank provider error message/detail; Mojo owns precedence.
pub fn provider_error_message_json(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<Option<Vec<u8>>, MojoError> {
    match transform_json(nodes, raw, 0, false, prodex_mojo_provider_error_message_v1)? {
        Some(output) if output != b"null" => Ok(Some(output)),
        _ => Ok(None),
    }
}

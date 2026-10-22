use super::{JsonNode, signed, transform_json};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_copilot_model_catalog_v1(
        abi: i64,
        max_entries: i64,
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

/// Transform a Copilot payload into a serialized `Result<Vec<model>, String>`.
/// Mojo owns raw-entry limits, traversal, field precedence, projection and null pruning.
pub fn copilot_model_catalog_json(
    nodes: &[JsonNode<'_>],
    raw: &str,
    max_entries: usize,
) -> Result<Vec<u8>, MojoError> {
    transform_json(
        nodes,
        raw,
        signed(max_entries)?,
        false,
        prodex_mojo_copilot_model_catalog_v1,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "copilot_catalog_tests.rs"]
mod tests;

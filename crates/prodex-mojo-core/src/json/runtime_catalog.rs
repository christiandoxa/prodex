use super::{JsonNode, transform_json};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_runtime_model_catalog_v1(
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

/// Render the runtime model schema from typed provider and model observations.
pub fn runtime_model_catalog_json(nodes: &[JsonNode<'_>], raw: &str) -> Result<Vec<u8>, MojoError> {
    transform_json(nodes, raw, 0, false, prodex_mojo_runtime_model_catalog_v1)?
        .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "runtime_catalog_tests.rs"]
mod tests;

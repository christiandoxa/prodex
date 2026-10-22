use super::{JsonNode, transform_json_with_scratch};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_runtime_catalog_plan_v1(
        abi: i64,
        limit: i64,
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

/// Assemble runtime model entries through the bounded compiled catalog owner.
pub fn runtime_catalog_plan_json(
    nodes: &[JsonNode<'_>],
    raw: &str,
    max_entries: usize,
) -> Result<Vec<u8>, MojoError> {
    transform_json_with_scratch(
        nodes,
        raw,
        i64::try_from(max_entries).map_err(|_| MojoError::InvalidInput)?,
        false,
        prodex_mojo_runtime_catalog_plan_v1,
        4,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "runtime_catalog_plan_tests.rs"]
mod tests;

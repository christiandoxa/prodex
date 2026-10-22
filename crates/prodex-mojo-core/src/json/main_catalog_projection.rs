use super::{JsonNode, transform_json};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_main_catalog_projection_v1(
        abi: i64,
        reserved: i64,
        flags: i64,
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

/// Project main-picker fields from a source-ordered array of catalog entries.
/// The compiled owner chooses typed aliases, Unicode trim and metadata defaults.
pub fn main_catalog_projection_json(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<Vec<u8>, MojoError> {
    transform_json(nodes, raw, 0, false, prodex_mojo_main_catalog_projection_v1)?
        .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "main_catalog_projection_tests.rs"]
mod tests;

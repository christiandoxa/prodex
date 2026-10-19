//! Structured ping diagnostics after native JSONL acquisition, before secret redaction.
use crate::MojoError;
use crate::json::{JsonNode, transform_json};

unsafe extern "C" {
    fn prodex_mojo_ping_failure_detail_v1(
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

/// Select and join the first structured failure from a borrowed array of JSONL values.
/// The caller applies existing secret redaction and diagnostic byte limits afterward.
pub fn ping_structured_failure_detail(nodes: &[JsonNode<'_>]) -> Result<Option<String>, MojoError> {
    transform_json(nodes, "", 0, false, prodex_mojo_ping_failure_detail_v1)?
        .map(|text| String::from_utf8(text).map_err(|_| MojoError::InvalidOutput))
        .transpose()
}

#[cfg(test)]
#[path = "failure_detail_tests.rs"]
mod tests;

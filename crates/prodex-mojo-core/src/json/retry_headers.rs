//! Host-owned decoded JSON to the canonical Mojo retry-advice planner.
use super::{JsonNode, ffi_nodes};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_runtime_retry_after_json_v1(
        abi: i64,
        nodes: u64,
        count: i64,
        dates: u64,
        fallback_millis: i64,
    ) -> i64;
}

/// Date values are decoded HTTP dates relative to a single host clock sample.
/// All JSON paths, header validation, precedence and delay caps are Mojo-owned.
pub fn runtime_retry_after_json(
    nodes: &[JsonNode<'_>],
    date_millis: &[i64],
    fallback_millis: i64,
) -> Result<Option<u64>, MojoError> {
    if nodes.is_empty()
        || nodes.len() > 65_537
        || nodes.len() != date_millis.len()
        || date_millis.iter().any(|value| *value < -1)
    {
        return Err(MojoError::InvalidInput);
    }
    let nodes = ffi_nodes(nodes, "")?;
    let result = unsafe {
        prodex_mojo_runtime_retry_after_json_v1(
            1,
            nodes.as_ptr() as usize as u64,
            nodes.len() as i64,
            date_millis.as_ptr() as usize as u64,
            fallback_millis,
        )
    };
    match result {
        -1 => Ok(None),
        0..=300_000 => Ok(Some(result as u64)),
        -2 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

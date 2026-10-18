//! Request-wide alias ownership and reference extraction through compiled Mojo.
use super::{JsonNode, StringView, ffi_nodes, signed, status, transform_json_with_views};
use crate::MojoError;

/// Selects request references, alias declarations, or text references with supplied aliases.
#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum SmartContextArtifactProjection {
    Request,
    Aliases,
    Text,
}

unsafe extern "C" {
    fn prodex_smart_context_artifact_alias_count_v1(
        abi: i64,
        operation: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        length: i64,
        result: u64,
    ) -> i64;
    fn prodex_smart_context_artifact_projection_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Extracts aliases and reference objects as JSON; the host only acquires the tree.
pub fn smart_context_artifact_projection(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: SmartContextArtifactProjection,
) -> Result<Vec<u8>, MojoError> {
    let input = ffi_nodes(nodes, raw)?;
    let mut alias_count = -1_i64;
    status(unsafe {
        prodex_smart_context_artifact_alias_count_v1(
            1,
            operation as i64,
            input.as_ptr() as usize as u64,
            signed(input.len())?,
            raw.as_ptr() as usize as u64,
            signed(raw.len())?,
            (&mut alias_count as *mut i64) as usize as u64,
        )
    })?;
    let scratch_count = usize::try_from(alias_count)
        .ok()
        .and_then(|count| count.checked_mul(7))
        .and_then(|count| count.checked_add(nodes.len()))
        .and_then(|count| count.checked_add(1))
        .ok_or(MojoError::InvalidOutput)?;
    signed(scratch_count)?;
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(scratch_count)
        .map_err(|_| MojoError::Capacity)?;
    scratch.resize(scratch_count, StringView::default());
    transform_json_with_views(
        &input,
        raw,
        operation as i64,
        false,
        prodex_smart_context_artifact_projection_v1,
        &mut scratch,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "smart_context_artifact_projection_tests.rs"]
mod tests;

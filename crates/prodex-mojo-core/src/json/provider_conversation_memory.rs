//! Cache memory estimates from borrowed Serde shape facts; Mojo owns all arithmetic.
use super::{JsonNode, status};
use crate::MojoError;

/// Host representation sizes, acquired with `size_of` at the Serde boundary.
#[derive(Clone, Copy)]
pub struct JsonMemoryLayout {
    pub value_bytes: usize,
    pub bool_bytes: usize,
    pub number_bytes: usize,
}

#[repr(C)]
struct MemoryNode {
    kind: i64,
    key_bytes: u64,
    string_bytes: u64,
}

const _: () = {
    assert!(std::mem::size_of::<MemoryNode>() == 24);
    assert!(std::mem::align_of::<MemoryNode>() == 8);
};

unsafe extern "C" {
    fn prodex_provider_conversation_memory_v1(
        version: i64,
        nodes_address: u64,
        count: i64,
        storage_key_bytes: u64,
        value_bytes: u64,
        bool_bytes: u64,
        number_bytes: u64,
        maximum: u64,
        output_address: u64,
    ) -> i64;
}

/// Estimates a synthetic messages array without serializing or copying its payloads.
pub fn conversation_estimated_bytes(
    nodes: &[JsonNode<'_>],
    storage_key_bytes: usize,
    layout: JsonMemoryLayout,
) -> Result<usize, MojoError> {
    let count = i64::try_from(nodes.len()).map_err(|_| MojoError::InvalidInput)?;
    if count == 0 || count > i64::MAX / 24 {
        return Err(MojoError::InvalidInput);
    }
    let facts = nodes
        .iter()
        .map(|node| MemoryNode {
            kind: node.kind as i64,
            key_bytes: node.key.len() as u64,
            string_bytes: node.text.len() as u64,
        })
        .collect::<Vec<_>>();
    let mut output = 0_u64;
    status(unsafe {
        prodex_provider_conversation_memory_v1(
            1,
            facts.as_ptr() as u64,
            count,
            storage_key_bytes as u64,
            layout.value_bytes as u64,
            layout.bool_bytes as u64,
            layout.number_bytes as u64,
            usize::MAX as u64,
            (&raw mut output) as u64,
        )
    })?;
    usize::try_from(output).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "provider_conversation_memory_tests.rs"]
mod tests;

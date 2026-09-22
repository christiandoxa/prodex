//! Complete DeepSeek message normalization and adjacency ownership.
use crate::{
    MojoError,
    json::{JsonNode, transform_json},
};

#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum DeepSeekMessageOperation {
    ThinkingMessages,
    AssistantContent,
    RepairAdjacency,
    MergeMetadata,
}

unsafe extern "C" {
    fn prodex_mojo_deepseek_messages_v1(
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

pub fn transform_deepseek_messages(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: DeepSeekMessageOperation,
) -> Result<Option<Vec<u8>>, MojoError> {
    transform_json(
        nodes,
        raw,
        operation as i64,
        false,
        prodex_mojo_deepseek_messages_v1,
    )
}

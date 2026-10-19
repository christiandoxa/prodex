//! Context record projection; Serde acquisition and final redaction stay with the caller.
use super::{JsonNode, transform_json};
use crate::MojoError;

/// The context record family selected by the existing log protocol classifier.
#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum TranscriptContextKind {
    SessionMetadata,
    TurnContext,
    Reasoning,
}

unsafe extern "C" {
    fn prodex_mojo_log_transcript_context_v1(
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

/// Return `[source, text]` records with Mojo-owned field order, fallbacks and joins.
pub fn log_transcript_context(
    nodes: &[JsonNode<'_>],
    kind: TranscriptContextKind,
) -> Result<Vec<u8>, MojoError> {
    transform_json(
        nodes,
        "",
        kind as i64,
        false,
        prodex_mojo_log_transcript_context_v1,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "log_transcript_context_tests.rs"]
mod tests;

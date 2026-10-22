use super::{JsonNode, transform_json};
use crate::MojoError;

/// Presentation requested for a caller-acquired token event and rate text.
#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum LogTokenPresentation {
    Envelope = 0,
    Metadata = 1,
    Tui = 2,
    Search = 3,
}

unsafe extern "C" {
    fn prodex_log_token_presentation_v1(
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

/// Project the checked `[event, optional formatted rate, missing rate] tuple.
pub fn log_token_presentation_json(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: LogTokenPresentation,
) -> Result<Vec<u8>, MojoError> {
    transform_json(
        nodes,
        raw,
        operation as i64,
        false,
        prodex_log_token_presentation_v1,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "log_token_tests.rs"]
mod tests;

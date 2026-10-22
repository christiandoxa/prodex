//! Checks over decoded session JSON; no wire parsing or host fallback.
use super::{JsonNode, ffi_nodes, signed};
use crate::MojoError;

/// A session record check, or the payload-only visibility contract used by logs.
#[derive(Clone, Copy, Debug)]
pub enum SessionPromptWriteJsonCheck<'a> {
    RecordShape,
    VisibleUserMessage,
    ExactUserMessage { expected: &'a str },
    TranscriptPayloadVisible,
}

unsafe extern "C" {
    fn prodex_session_prompt_write_json_v1(
        version: i64,
        operation: i64,
        nodes_address: u64,
        node_count: i64,
        expected_address: u64,
        expected_length: i64,
    ) -> i64;
}

/// Lets Mojo classify and compare borrowed, Serde-decoded session fields.
pub fn session_prompt_write_json_check(
    nodes: &[JsonNode<'_>],
    check: SessionPromptWriteJsonCheck<'_>,
) -> Result<bool, MojoError> {
    if nodes.len() > 1_048_576 {
        return Err(MojoError::InvalidInput);
    }
    let (operation, expected) = match check {
        SessionPromptWriteJsonCheck::RecordShape => (0, ""),
        SessionPromptWriteJsonCheck::VisibleUserMessage => (1, ""),
        SessionPromptWriteJsonCheck::ExactUserMessage { expected } => (2, expected),
        SessionPromptWriteJsonCheck::TranscriptPayloadVisible => (3, ""),
    };
    let input = ffi_nodes(nodes, "")?;
    match unsafe {
        prodex_session_prompt_write_json_v1(
            1,
            operation,
            input.as_ptr() as u64,
            signed(input.len())?,
            expected.as_ptr() as u64,
            signed(expected.len())?,
        )
    } {
        -2 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "session_prompt_write_tests.rs"]
mod tests;

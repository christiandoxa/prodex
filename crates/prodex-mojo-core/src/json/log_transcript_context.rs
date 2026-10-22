//! Context record projection; Serde acquisition and final redaction stay with the caller.
use super::{JsonNode, StringView, ffi_nodes, transform_json, transform_json_with_views};
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

/// Classification supplied by the existing Mojo transcript classifier.
#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum TranscriptOperationKind {
    ToolProtocol = 3,
    McpProtocol = 4,
    AgentProtocol = 5,
    TerminalStatus = 6,
    ErrorStatus = 7,
    ShellCall = 8,
    ShellOutput = 9,
    FunctionCall = 10,
    CustomCall = 11,
    ToolOutput = 12,
    ResponseMessage = 13,
    UserMessage = 14,
    AssistantMessage = 15,
    ReasoningMessage = 16,
}

/// Native scalar/JSON acquisition and existing redacted display text for one node.
#[derive(Default)]
pub struct TranscriptOperationValue {
    pub scalar: String,
    /// Final safe text for protocol/status; sanitized name for calls;
    /// visible text for results/messages. Empty message text encodes no visible part.
    /// The compiled mode chooses its field.
    pub output: String,
}

/// Project operation fields from a tree and its acquired presentations.
pub fn log_transcript_operation(
    nodes: &[JsonNode<'_>],
    raw: &str,
    values: &[TranscriptOperationValue],
    kind: TranscriptOperationKind,
) -> Result<Vec<u8>, MojoError> {
    if values.len() != nodes.len() {
        return Err(MojoError::InvalidInput);
    }
    let input = ffi_nodes(nodes, raw)?;
    let mut views = values
        .iter()
        .flat_map(|value| {
            [
                StringView::from(value.scalar.as_str()),
                StringView::from(value.output.as_str()),
            ]
        })
        .collect::<Vec<_>>();
    transform_json_with_views(
        &input,
        raw,
        kind as i64,
        false,
        prodex_mojo_log_transcript_context_v1,
        &mut views,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "log_transcript_context_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "log_transcript_operation_tests.rs"]
mod operation_tests;

#[cfg(test)]
#[path = "log_transcript_shell_tests.rs"]
mod shell_tests;

#[cfg(test)]
#[path = "log_transcript_tool_tests.rs"]
mod tool_tests;

#[cfg(test)]
#[path = "log_transcript_message_tests.rs"]
mod message_tests;

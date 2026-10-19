//! Gemini normalized errors over acquired JSON and UTF-8 facts.
use super::{JsonNode, status};
use crate::MojoError;
use crate::json::transform_json;

unsafe extern "C" {
    fn prodex_gemini_normalized_error_v1(
        version: i64,
        http_status: i64,
        body_terminal: i64,
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

/// Probes eligibility before the host acquires JSON or scans buffered SSE.
pub fn gemini_error_normalization_needs_body(http_status: u16) -> Result<bool, MojoError> {
    let mut metadata = [-1_i64; 2];
    status(unsafe {
        prodex_gemini_normalized_error_v1(
            1,
            i64::from(http_status),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            0,
            metadata.as_mut_ptr() as u64,
        )
    })?;
    match metadata {
        [0, 0] => Ok(false),
        [2, 0] => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Mojo selects message scope, defaults, 429 admissibility and complete JSON.
/// The context contains acquired value/source, parse success and quota facts.
pub fn gemini_normalized_error_json(
    http_status: u16,
    body_terminal: bool,
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<Option<Vec<u8>>, MojoError> {
    transform_json(
        nodes,
        raw,
        i64::from(http_status),
        body_terminal,
        prodex_gemini_normalized_error_v1,
    )
}

#[cfg(test)]
#[path = "normalized_error_tests.rs"]
mod tests;

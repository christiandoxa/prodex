use super::{MojoError, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address};
use crate::json::{JsonNode, transform_json};

const GEMINI_COMPACT_REWRITE_ABI_VERSION: i64 = 1;
const GEMINI_COMPACT_REWRITE_REQUEST: i64 = 1;
const GEMINI_COMPACT_REWRITE_SUMMARY: i64 = 2;

unsafe extern "C" {
    fn prodex_mojo_gemini_compact_rewrite_v1(
        abi_version: i64,
        operation: i64,
        flag: i64,
        nodes_address: u64,
        nodes_count: i64,
        raw_address: u64,
        raw_length: i64,
        scratch_address: u64,
        scratch_count: i64,
        measuring: i64,
        output_address: u64,
        output_capacity: i64,
        metadata_address: u64,
    ) -> i64;
    fn prodex_mojo_gemini_compact_response_body_v1(
        abi_version: i64,
        summary_address: u64,
        summary_length: i64,
        measuring: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_mojo_gemini_compact_error_reason_v1(
        abi_version: i64,
        text_address: u64,
        text_length: i64,
        output_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiCompactErrorReason {
    Provider = 0,
    Timeout = 1,
    Unavailable = 2,
    Unsupported = 3,
    InvalidResponse = 4,
}

fn status(code: i64) -> Result<(), MojoError> {
    match code {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::InvalidOutput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn gemini_compact_request_json(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    transform_json(
        nodes,
        raw,
        GEMINI_COMPACT_REWRITE_REQUEST,
        false,
        prodex_mojo_gemini_compact_rewrite_v1,
    )?
    .ok_or(MojoError::InvalidOutput)
}

pub fn gemini_compact_summary_json(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<Option<Vec<u8>>, MojoError> {
    ensure_rich_abi()?;
    transform_json(
        nodes,
        raw,
        GEMINI_COMPACT_REWRITE_SUMMARY,
        false,
        prodex_mojo_gemini_compact_rewrite_v1,
    )
}

pub fn gemini_compact_response_body(summary: &str) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    let capacity = summary
        .len()
        .checked_add(1024)
        .and_then(|value| value.checked_mul(6))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity.max(512)];
    let mut written = 0_i64;
    status(unsafe {
        prodex_mojo_gemini_compact_response_body_v1(
            GEMINI_COMPACT_REWRITE_ABI_VERSION,
            mojo_pointer_address(summary.as_ptr()),
            i64::try_from(summary.len()).map_err(|_| MojoError::InvalidInput)?,
            0,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(output)
}

pub fn gemini_compact_error_reason(message: &str) -> Result<GeminiCompactErrorReason, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    status(unsafe {
        prodex_mojo_gemini_compact_error_reason_v1(
            GEMINI_COMPACT_REWRITE_ABI_VERSION,
            message.as_ptr() as u64,
            i64::try_from(message.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    match output {
        0 => Ok(GeminiCompactErrorReason::Provider),
        1 => Ok(GeminiCompactErrorReason::Timeout),
        2 => Ok(GeminiCompactErrorReason::Unavailable),
        3 => Ok(GeminiCompactErrorReason::Unsupported),
        4 => Ok(GeminiCompactErrorReason::InvalidResponse),
        _ => Err(MojoError::InvalidOutput),
    }
}

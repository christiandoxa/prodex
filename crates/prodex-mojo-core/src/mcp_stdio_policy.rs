use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MESSAGE_MAX_BYTES: usize = 67_108_864;

/// First-line classification uses the existing separate header/message limits.
/// Continuation headers use the caller's exact line limit.
#[derive(Debug, Clone, Copy)]
pub enum McpLineLimit {
    FirstLine,
    Fixed(usize),
}

/// Checked copy/consume plan for a borrowed raw BufRead chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpLineChunkPlan {
    pub take: usize,
    pub first_non_whitespace: Option<u8>,
    pub limit: usize,
    pub too_large: bool,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpContentLengthParse {
    Value(u64),
    MissingHeaderSeparator,
    InvalidValue,
}

unsafe extern "C" {
    fn prodex_mcp_line_chunk_v1(
        abi_version: i64,
        mode: i64,
        input_address: u64,
        input_length: i64,
        buffered: u64,
        first_byte: i64,
        fixed_limit: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_mcp_header_is_content_length_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        trim_first: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mcp_content_length_parse_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        parse_kind_address: u64,
        value_address: u64,
    ) -> i64;
    fn prodex_mcp_first_line_is_header_v1(
        abi_version: i64,
        first_byte: i64,
        output_address: u64,
    ) -> i64;
}

/// Mojo owns LF scanning, ASCII first-byte detection, line limits and overflow.
/// Raw input is bounded before borrowing; UTF-8 validation stays at line assembly.
pub fn line_chunk_plan(
    available: &[u8],
    buffered: usize,
    first: Option<u8>,
    limits: McpLineLimit,
) -> Result<McpLineChunkPlan, MojoError> {
    let available = &available[..available.len().min(MESSAGE_MAX_BYTES + 1)];
    let (mode, fixed_limit) = match limits {
        McpLineLimit::FirstLine => (0, 0),
        McpLineLimit::Fixed(limit) => (1, limit as u64),
    };
    let mut output = [u64::MAX; 5];
    status(unsafe {
        prodex_mcp_line_chunk_v1(
            ABI_VERSION,
            mode,
            available.as_ptr() as usize as u64,
            available.len() as i64,
            buffered as u64,
            first.map_or(-1, i64::from),
            fixed_limit,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let take = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let first_non_whitespace = match output[1] {
        256 => None,
        value => Some(u8::try_from(value).map_err(|_| MojoError::InvalidOutput)?),
    };
    let limit = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if take > available.len()
        || (take == 0 && !available.is_empty())
        || limit > MESSAGE_MAX_BYTES
        || output[3] > 1
        || output[4] > 1
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(McpLineChunkPlan {
        take,
        first_non_whitespace,
        limit,
        too_large: output[3] == 1,
        complete: output[4] == 1,
    })
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn header_is_content_length(line: &str, trim_first: bool) -> Result<bool, MojoError> {
    let mut output = -1_i64;
    status(unsafe {
        prodex_mcp_header_is_content_length_v1(
            ABI_VERSION,
            line.as_ptr() as usize as u64,
            i64::try_from(line.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(trim_first),
            (&mut output as *mut i64) as usize as u64,
        )
    })?;
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn parse_content_length(line: &str) -> Result<McpContentLengthParse, MojoError> {
    let mut kind = -1_i64;
    let mut value = 0_u64;
    status(unsafe {
        prodex_mcp_content_length_parse_v1(
            ABI_VERSION,
            line.as_ptr() as usize as u64,
            i64::try_from(line.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut kind as *mut i64) as usize as u64,
            (&mut value as *mut u64) as usize as u64,
        )
    })?;
    match kind {
        0 => Ok(McpContentLengthParse::Value(value)),
        1 => Ok(McpContentLengthParse::MissingHeaderSeparator),
        2 => Ok(McpContentLengthParse::InvalidValue),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Classify the first non-whitespace byte before selecting the bounded line limit.
pub fn first_line_is_header(byte: u8) -> Result<bool, MojoError> {
    let mut output = -1_i64;
    status(unsafe {
        prodex_mcp_first_line_is_header_v1(
            ABI_VERSION,
            i64::from(byte),
            (&mut output as *mut i64) as usize as u64,
        )
    })?;
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "mcp_stdio_policy/chunk_tests.rs"]
mod chunk_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_header_policy_matches_case_trim_and_integer_contracts() {
        assert!(header_is_content_length("Content-Length: 2\r\n", false).unwrap());
        assert!(header_is_content_length("  CONTENT-LENGTH: 2  ", true).unwrap());
        assert!(!header_is_content_length("  CONTENT-LENGTH: 2  ", false).unwrap());
        assert_eq!(
            parse_content_length("Content-Length: +01\r\n").unwrap(),
            McpContentLengthParse::Value(1)
        );
        assert_eq!(
            parse_content_length("Content-Length 1").unwrap(),
            McpContentLengthParse::MissingHeaderSeparator
        );
        assert_eq!(
            parse_content_length("Content-Length: 1_0").unwrap(),
            McpContentLengthParse::InvalidValue
        );
    }

    #[test]
    fn mcp_first_line_policy_keeps_only_objects_and_batches_on_json_limit() {
        assert!(!first_line_is_header(b'{').unwrap());
        assert!(!first_line_is_header(b'[').unwrap());
        assert!(first_line_is_header(b'C').unwrap());
        assert!(first_line_is_header(0).unwrap());
    }
}

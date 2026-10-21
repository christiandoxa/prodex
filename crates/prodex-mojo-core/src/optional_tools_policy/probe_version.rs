//! Borrowed UTF-8 token ranges planned by compiled Mojo for probe versions.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_INPUT_BYTES: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_optional_tool_probe_token_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        cursor: i64,
        output_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionalToolProbeToken {
    pub start: usize,
    pub end: usize,
    pub next_cursor: usize,
}

/// Return the next whitespace-delimited, punctuation-trimmed semver candidate.
/// Invalid Mojo output is an error; this adapter never scans in Rust.
pub fn optional_tool_probe_token(
    text: &str,
    cursor: usize,
) -> Result<Option<OptionalToolProbeToken>, MojoError> {
    if text.len() > MAX_INPUT_BYTES || cursor > text.len() || !text.is_char_boundary(cursor) {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-2_i64; 3];
    let status = unsafe {
        prodex_optional_tool_probe_token_v1(
            ABI_VERSION,
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(cursor).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    if output[0] == -1 && output[1] == -1 && output[2] == text.len() as i64 {
        return Ok(None);
    }
    let start = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    let next_cursor = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if start > end
        || end > next_cursor
        || next_cursor <= cursor
        || next_cursor > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
        || !text.is_char_boundary(next_cursor)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(OptionalToolProbeToken {
        start,
        end,
        next_cursor,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_probe_tokens_preserve_scan_and_ascii_trimming() {
        let line = " tool\u{2003}v1.2.3\u{00a0}[vv2.3.4] ";
        let mut cursor = 0;
        let mut tokens = Vec::new();
        while let Some(token) = optional_tool_probe_token(line, cursor).unwrap() {
            tokens.push(line[token.start..token.end].to_owned());
            cursor = token.next_cursor;
        }
        assert_eq!(tokens, ["tool", "1.2.3", "2.3.4"]);
        assert_eq!(optional_tool_probe_token("  ", 0), Ok(None));
        assert_eq!(optional_tool_probe_token("", 0), Ok(None));
    }

    #[test]
    fn compiled_probe_tokens_reject_invalid_cursor_and_size() {
        assert_eq!(
            optional_tool_probe_token("é", 1),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            optional_tool_probe_token(&"a".repeat(MAX_INPUT_BYTES + 1), 0),
            Err(MojoError::InvalidInput)
        );
    }
}

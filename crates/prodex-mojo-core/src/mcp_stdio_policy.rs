use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpContentLengthParse {
    Value(u64),
    MissingHeaderSeparator,
    InvalidValue,
}

unsafe extern "C" {
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
}

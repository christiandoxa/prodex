//! Mojo-owned validation for streamed DeepSeek tool-call arguments.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_deepseek_stream_tool_arguments_policy_v1(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
        name_present: i64,
        arguments_address: u64,
        arguments_length: i64,
    ) -> i64;
}

/// The semantic result of streamed tool-call argument validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekStreamToolArgumentsPlan {
    Valid,
    MissingName,
    InvalidArguments,
}

/// Validate a streamed function name and its JSON argument fragment.
pub fn stream_tool_arguments_policy(
    name: Option<&str>,
    arguments: &str,
) -> Result<DeepSeekStreamToolArgumentsPlan, MojoError> {
    let name_bytes = name.unwrap_or_default().as_bytes();
    let arguments_bytes = arguments.as_bytes();
    let name_length = i64::try_from(name_bytes.len()).map_err(|_| MojoError::InvalidInput)?;
    let arguments_length =
        i64::try_from(arguments_bytes.len()).map_err(|_| MojoError::InvalidInput)?;
    let result = unsafe {
        prodex_deepseek_stream_tool_arguments_policy_v1(
            ABI_VERSION,
            name_bytes.as_ptr() as usize as u64,
            name_length,
            i64::from(name.is_some()),
            arguments_bytes.as_ptr() as usize as u64,
            arguments_length,
        )
    };
    match result {
        0 => Ok(DeepSeekStreamToolArgumentsPlan::Valid),
        1 => Ok(DeepSeekStreamToolArgumentsPlan::MissingName),
        2 => Ok(DeepSeekStreamToolArgumentsPlan::InvalidArguments),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::{DeepSeekStreamToolArgumentsPlan, stream_tool_arguments_policy};

    #[test]
    fn stream_tool_arguments_policy_preserves_name_and_json_boundaries() {
        assert_eq!(
            stream_tool_arguments_policy(Some(" \u{2003}shell\u{3000} "), " {\"cmd\":1} ").unwrap(),
            DeepSeekStreamToolArgumentsPlan::Valid
        );
        assert_eq!(
            stream_tool_arguments_policy(Some(" \u{2003} \u{3000}"), "{} ").unwrap(),
            DeepSeekStreamToolArgumentsPlan::MissingName
        );
        assert_eq!(
            stream_tool_arguments_policy(Some("shell"), "{bad").unwrap(),
            DeepSeekStreamToolArgumentsPlan::InvalidArguments
        );
        assert_eq!(
            stream_tool_arguments_policy(Some("shell"), "{\"x\":\"\\uZZZZ\"}").unwrap(),
            DeepSeekStreamToolArgumentsPlan::InvalidArguments
        );
        assert_eq!(
            stream_tool_arguments_policy(Some("shell"), "01").unwrap(),
            DeepSeekStreamToolArgumentsPlan::InvalidArguments
        );
        assert_eq!(
            stream_tool_arguments_policy(Some("shell"), "   ").unwrap(),
            DeepSeekStreamToolArgumentsPlan::Valid
        );
        assert_eq!(
            stream_tool_arguments_policy(Some("shell"), " \u{2003}{} ").unwrap(),
            DeepSeekStreamToolArgumentsPlan::InvalidArguments
        );
    }
}

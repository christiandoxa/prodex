//! Mojo-owned validation for streamed DeepSeek tool-call arguments.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_deepseek_stream_tool_kind_v1(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
    ) -> i64;
    fn prodex_deepseek_stream_identifier_v1(
        abi_version: i64,
        mode: i64,
        provider_address: u64,
        provider_length: i64,
        request_id: u64,
        index: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_deepseek_stream_tool_arguments_policy_v1(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
        name_present: i64,
        arguments_address: u64,
        arguments_length: i64,
    ) -> i64;
}

/// Compiled Mojo exclusively selects the DeepSeek streamed tool kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekStreamToolKind {
    ToolSearch,
    ApplyPatch,
    FunctionCall,
}

pub fn stream_tool_kind(name: &str) -> Result<DeepSeekStreamToolKind, MojoError> {
    let result = unsafe {
        prodex_deepseek_stream_tool_kind_v1(
            ABI_VERSION,
            name.as_ptr() as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match result {
        0 => Ok(DeepSeekStreamToolKind::ToolSearch),
        1 => Ok(DeepSeekStreamToolKind::ApplyPatch),
        2 => Ok(DeepSeekStreamToolKind::FunctionCall),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Versioned fallback identifier formats for streamed DeepSeek events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum DeepSeekStreamIdKind {
    ToolCall = 0,
    OutputText = 1,
    Response = 2,
}

pub fn stream_fallback_identifier(
    kind: DeepSeekStreamIdKind,
    provider: &str,
    request_id: u64,
    index: usize,
) -> Result<String, MojoError> {
    let capacity = provider
        .len()
        .checked_add(50)
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_deepseek_stream_identifier_v1(
            ABI_VERSION,
            kind as i64,
            provider.as_ptr() as u64,
            i64::try_from(provider.len()).map_err(|_| MojoError::InvalidInput)?,
            request_id,
            u64::try_from(index).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            &mut written as *mut _ as u64,
        )
    };
    match status {
        0 => {
            let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
            if written > output.len() {
                return Err(MojoError::InvalidOutput);
            }
            output.truncate(written);
            String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
        }
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        3 => Err(MojoError::Capacity),
        _ => Err(MojoError::InvalidOutput),
    }
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

#[cfg(test)]
mod stream_item_semantics_tests {
    use super::*;

    #[test]
    fn compiled_tool_kind_discriminates_exact_names_only() {
        assert_eq!(
            stream_tool_kind("tool_search"),
            Ok(DeepSeekStreamToolKind::ToolSearch)
        );
        assert_eq!(
            stream_tool_kind("apply_patch"),
            Ok(DeepSeekStreamToolKind::ApplyPatch)
        );
        for name in ["tool_search ", "Apply_Patch", "mcp__foo", "", "🧭"] {
            assert_eq!(
                stream_tool_kind(name),
                Ok(DeepSeekStreamToolKind::FunctionCall)
            );
        }
    }

    #[test]
    fn compiled_fallback_ids_keep_unsigned_decimal_and_unicode_values() {
        for (kind, index, expected) in [
            (DeepSeekStreamIdKind::ToolCall, 7, "call_東京_42_7"),
            (DeepSeekStreamIdKind::OutputText, 7, "msg_東京_42"),
            (DeepSeekStreamIdKind::Response, 7, "resp_東京_42"),
        ] {
            assert_eq!(
                stream_fallback_identifier(kind, "東京", 42, index),
                Ok(expected.to_owned()),
            );
        }
        assert_eq!(
            stream_fallback_identifier(DeepSeekStreamIdKind::ToolCall, "a", u64::MAX, usize::MAX,),
            Ok(format!("call_a_{}_{}", u64::MAX, usize::MAX)),
        );
        assert_eq!(
            stream_fallback_identifier(DeepSeekStreamIdKind::Response, "", 0, 0),
            Ok("resp__0".to_owned()),
        );
    }

    #[test]
    fn compiled_stream_identifier_rejects_invalid_abi_and_capacity() {
        let mut output = [22_u8; 32];
        let mut written = 77_i64;
        unsafe {
            assert_eq!(
                prodex_deepseek_stream_identifier_v1(
                    ABI_VERSION + 1,
                    0,
                    b"abc".as_ptr() as u64,
                    3,
                    1,
                    2,
                    output.as_mut_ptr() as u64,
                    32,
                    &mut written as *mut _ as u64,
                ),
                -4,
            );
            assert_eq!(
                prodex_deepseek_stream_identifier_v1(
                    ABI_VERSION,
                    3,
                    b"abc".as_ptr() as u64,
                    3,
                    1,
                    2,
                    output.as_mut_ptr() as u64,
                    32,
                    &mut written as *mut _ as u64,
                ),
                -1,
            );
            assert_eq!(
                prodex_deepseek_stream_identifier_v1(
                    ABI_VERSION,
                    0,
                    b"abc".as_ptr() as u64,
                    3,
                    1,
                    2,
                    output.as_mut_ptr() as u64,
                    2,
                    &mut written as *mut _ as u64,
                ),
                3,
            );
        }
        assert_eq!(output, [22; 32]);
        assert!(written > 2);
    }
}

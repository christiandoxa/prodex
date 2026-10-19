//! Mojo-owned validation for streamed DeepSeek tool-call arguments.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_deepseek_sse_tool_event_plan_v1(
        abi_version: i64,
        already_added: i64,
        arguments_present: i64,
        name_address: u64,
        name_length: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_deepseek_sse_pending_tools_v1(
        abi_version: i64,
        completed_address: u64,
        tool_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_deepseek_stream_response_id_fallback_v1(
        abi_version: i64,
        provider: u64,
        provider_len: i64,
        current: u64,
        current_len: i64,
    ) -> i64;
    fn prodex_deepseek_stream_delta_policy_v1(
        abi_version: i64,
        mode: i64,
        text: u64,
        text_len: i64,
        text_present: i64,
        existing_tool_call: i64,
    ) -> i64;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepSeekSseToolEventPlan {
    pub add_item: bool,
    pub emit_argument_delta: bool,
}

/// Mojo exclusively decides whether an item is newly added and whether
/// the streamed name is a function that can emit argument-delta events.
pub fn deepseek_sse_tool_event_plan(
    already_added: bool,
    name: &str,
    argument_delta_present: bool,
) -> Result<DeepSeekSseToolEventPlan, MojoError> {
    let mut output = [-1_i64; 2];
    let status = unsafe {
        prodex_deepseek_sse_tool_event_plan_v1(
            ABI_VERSION,
            i64::from(already_added),
            i64::from(argument_delta_present),
            name.as_ptr() as usize as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => {}
        -1 => return Err(MojoError::InvalidInput),
        -4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let decode = |value| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let add_item = decode(output[0])?;
    let emit_argument_delta = decode(output[1])?;
    if add_item == already_added || (emit_argument_delta && !argument_delta_present) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(DeepSeekSseToolEventPlan {
        add_item,
        emit_argument_delta,
    })
}

/// Exactly-once completion positions come from the compiled DeepSeek owner.
/// Rust retains BTreeMap updates and event construction, not a done filter.
pub fn deepseek_sse_pending_tool_positions(done: &[bool]) -> Result<Vec<usize>, MojoError> {
    let flags = done
        .iter()
        .map(|value| i64::from(*value))
        .collect::<Vec<_>>();
    let mut positions = vec![u64::MAX; done.len()];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_deepseek_sse_pending_tools_v1(
            ABI_VERSION,
            flags.as_ptr() as usize as u64,
            i64::try_from(flags.len()).map_err(|_| MojoError::InvalidInput)?,
            positions.as_mut_ptr() as usize as u64,
            i64::try_from(positions.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        -1 => return Err(MojoError::InvalidInput),
        -4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > done.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut previous = None;
    let mut result = Vec::with_capacity(count);
    for raw in &positions[..count] {
        let index = usize::try_from(*raw).map_err(|_| MojoError::InvalidOutput)?;
        if done.get(index) != Some(&false) || previous.is_some_and(|prior| index <= prior) {
            return Err(MojoError::InvalidOutput);
        }
        previous = Some(index);
        result.push(index);
    }
    Ok(result)
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

/// Exact historical generated-response ID ownership check in compiled Mojo.
/// Rust may read the JSON ID but never reconstructs the fallback prefix.
pub fn stream_response_id_is_fallback(provider: &str, current: &str) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_deepseek_stream_response_id_fallback_v1(
            ABI_VERSION,
            provider.as_ptr() as u64,
            i64::try_from(provider.len()).map_err(|_| MojoError::InvalidInput)?,
            current.as_ptr() as u64,
            i64::try_from(current.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Mojo-owned eligibility of streamed tool-call fragments, function objects,
/// optional delta text and optional Unicode-trimmed signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum DeepSeekStreamDeltaMode {
    FunctionObject = 0,
    IncrementalArguments = 1,
    OptionalArgumentDelta = 2,
    ThoughtSignature = 3,
}

pub fn stream_delta_accepted(
    mode: DeepSeekStreamDeltaMode,
    value: Option<&str>,
    existing_tool_call: bool,
) -> Result<bool, MojoError> {
    let text = value.unwrap_or_default();
    let result = unsafe {
        prodex_deepseek_stream_delta_policy_v1(
            ABI_VERSION,
            mode as i64,
            text.as_ptr() as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(value.is_some()),
            i64::from(existing_tool_call),
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
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
    fn compiled_deepseek_sse_event_planner_avoids_special_argument_streams_and_repeats() {
        for (name, added, fragment, add, emit) in [
            ("shell", false, true, true, true),
            ("shell", true, true, false, true),
            ("shell", false, false, true, false),
            ("tool_search", false, true, true, false),
            ("apply_patch", true, true, false, false),
            ("tool_search ", false, true, true, true),
            ("mcp__repo__東京🧭", false, true, true, true),
        ] {
            assert_eq!(
                deepseek_sse_tool_event_plan(added, name, fragment),
                Ok(DeepSeekSseToolEventPlan {
                    add_item: add,
                    emit_argument_delta: emit,
                }),
                "{name:?}/{added}/{fragment}",
            );
        }
    }

    #[test]
    fn compiled_deepseek_sse_completion_planner_retains_pending_order_and_no_duplicates() {
        for (flags, expected) in [
            (vec![], vec![]),
            (vec![true], vec![]),
            (vec![false], vec![0]),
            (vec![true, false, true, false], vec![1, 3]),
            (vec![false, false, false], vec![0, 1, 2]),
        ] {
            assert_eq!(deepseek_sse_pending_tool_positions(&flags), Ok(expected));
        }
    }

    #[test]
    fn compiled_deepseek_sse_planning_abi_rejects_corrupt_flags_without_output_writes() {
        let name = b"shell";
        let mut output = [97_i64; 2];
        let name_ptr = name.as_ptr() as usize as u64;
        let out = output.as_mut_ptr() as usize as u64;
        let event = |version, added, fragment, name, size, out, cap| unsafe {
            prodex_deepseek_sse_tool_event_plan_v1(version, added, fragment, name, size, out, cap)
        };
        assert_eq!(event(2, 0, 1, name_ptr, 5, out, 2), -4);
        assert_eq!(event(1, 2, 1, name_ptr, 5, out, 2), -1);
        assert_eq!(event(1, 0, -1, name_ptr, 5, out, 2), -1);
        assert_eq!(event(1, 0, 1, 0, 5, out, 2), -1);
        assert_eq!(event(1, 0, 1, name_ptr, 5, out, 1), -1);
        assert_eq!(event(1, 0, 1, name_ptr, 5, 0, 2), -1);
        let bad = [0xff_u8];
        assert_eq!(event(1, 0, 1, bad.as_ptr() as usize as u64, 1, out, 2), -1);
        assert_eq!(output, [97; 2]);
        let flags = [0_i64, 2_i64];
        let mut indices = [71_u64; 2];
        let mut count = 83_i64;
        let ptr = flags.as_ptr() as usize as u64;
        let dst = indices.as_mut_ptr() as usize as u64;
        let written = (&mut count as *mut i64) as usize as u64;
        let pending = |version, src, size, dst, cap, count| unsafe {
            prodex_deepseek_sse_pending_tools_v1(version, src, size, dst, cap, count)
        };
        assert_eq!(pending(2, ptr, 2, dst, 2, written), -4);
        assert_eq!(pending(1, 0, 2, dst, 2, written), -1);
        assert_eq!(pending(1, ptr, -1, dst, 2, written), -1);
        assert_eq!(pending(1, ptr, 2, dst, 1, written), -1);
        assert_eq!(pending(1, ptr, 2, dst, 2, 0), -1);
        assert_eq!(pending(1, ptr, 2, dst, 2, written), -1);
        assert_eq!(indices, [71; 2]);
        assert_eq!(count, 83);
    }

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

#[cfg(test)]
mod stream_delta_semantics_tests {
    use super::*;

    #[test]
    fn compiled_response_id_fallback_only_accepts_exact_provider_prefix() {
        for (provider, current, expected) in [
            ("deepseek", "resp_deepseek_7", true),
            ("deepseek", "resp_deepseek_", true),
            ("deepseek", "resp_deepseek", false),
            ("deepseek", "resp_deepseek2_7", false),
            ("deepseek", "chatcmpl_1", false),
            ("東京", "resp_東京_42", true),
            ("東京", "resp_東京42", false),
            ("", "resp__1", true),
        ] {
            assert_eq!(
                stream_response_id_is_fallback(provider, current),
                Ok(expected)
            );
        }
    }

    #[test]
    fn compiled_deepseek_delta_policy_keeps_only_safe_segments() {
        use DeepSeekStreamDeltaMode::*;
        assert_eq!(
            stream_delta_accepted(FunctionObject, Some(""), false),
            Ok(true)
        );
        assert_eq!(
            stream_delta_accepted(FunctionObject, None, false),
            Ok(false)
        );
        assert_eq!(stream_delta_accepted(FunctionObject, None, true), Ok(true));
        assert_eq!(
            stream_delta_accepted(IncrementalArguments, Some(""), false),
            Ok(false)
        );
        assert_eq!(
            stream_delta_accepted(IncrementalArguments, Some("{}"), false),
            Ok(true)
        );
        assert_eq!(
            stream_delta_accepted(OptionalArgumentDelta, None, false),
            Ok(false)
        );
        assert_eq!(
            stream_delta_accepted(OptionalArgumentDelta, Some(""), false),
            Ok(false)
        );
        assert_eq!(
            stream_delta_accepted(OptionalArgumentDelta, Some("🧭"), false),
            Ok(true)
        );
        assert_eq!(
            stream_delta_accepted(ThoughtSignature, Some("  \u{3000}"), false),
            Ok(false)
        );
        assert_eq!(
            stream_delta_accepted(ThoughtSignature, Some(" sig_🧭 "), false),
            Ok(true)
        );
        assert_eq!(
            stream_delta_accepted(ThoughtSignature, None, false),
            Ok(false)
        );
    }

    #[test]
    fn compiled_deepseek_delta_policy_rejects_invalid_abi_and_mode() {
        let data = "🧭".as_bytes();
        unsafe {
            assert_eq!(
                prodex_deepseek_stream_delta_policy_v1(
                    ABI_VERSION + 1,
                    3,
                    data.as_ptr() as u64,
                    data.len() as i64,
                    1,
                    0,
                ),
                -4,
            );
            assert_eq!(
                prodex_deepseek_stream_delta_policy_v1(
                    ABI_VERSION,
                    5,
                    data.as_ptr() as u64,
                    data.len() as i64,
                    1,
                    0,
                ),
                -1,
            );
            assert_eq!(
                prodex_deepseek_stream_delta_policy_v1(
                    ABI_VERSION,
                    3,
                    data.as_ptr() as u64,
                    data.len() as i64,
                    2,
                    0,
                ),
                -1,
            );
            assert_eq!(
                prodex_deepseek_stream_response_id_fallback_v1(ABI_VERSION + 1, 0, 0, 0, 0),
                -4,
            );
        }
    }
}

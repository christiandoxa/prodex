//! Typed ABI for the compiled Mojo SSE chunk framing kernel.
//! The host owns transport buffers, but Mojo alone locates LF boundaries.

use crate::MojoError;

const MAX_BOUNDARIES: usize = 256;

#[repr(C)]
#[derive(Clone, Copy)]
struct SseUtf8Span {
    address: u64,
    len: u64,
}

unsafe extern "C" {
    fn prodex_runtime_sse_event_state_v1(
        abi_version: i64,
        operation: i64,
        line_count: i64,
        first_address: u64,
        first_length: i64,
        utf8_valid: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_runtime_sse_json_frame_plan_v1(
        abi_version: i64,
        joined_json_valid: i64,
        separate_json_valid: i64,
        accumulated_bytes: u64,
        fragment_bytes: u64,
        accumulated_max_bytes: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_runtime_sse_failure_flags_v1(
        abi_version: i64,
        error_class: i64,
        error_action: i64,
        output: u64,
    ) -> i64;
    fn prodex_runtime_sse_payload_join_v1(
        abi_version: i64,
        views: u64,
        line_count: i64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_runtime_sse_chunk_line_offsets_v1(
        abi_version: i64,
        input: u64,
        input_length: i64,
        offsets: u64,
        offsets_capacity: i64,
        result: u64,
    ) -> i64;
}

/// The Mojo-owned event-buffer decision requested by the host.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseEventStateOperation {
    Emit = 0,
    IncomingData = 1,
    ParsePayload = 2,
}

/// Actions to apply to the original host-owned data-line buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseEventStateAction {
    Empty,
    Discard,
    Emit,
    MarkInvalid,
    IgnoreIncoming,
    AppendIncoming,
}

pub fn sse_event_state_action(
    data_lines: &[String],
    operation: SseEventStateOperation,
    incoming_utf8_valid: bool,
) -> Result<SseEventStateAction, MojoError> {
    let first = data_lines.first().map(String::as_str);
    let (address, length) = first.map_or(Ok((0, 0)), |first| {
        Ok((
            first.as_ptr() as usize as u64,
            i64::try_from(first.len()).map_err(|_| MojoError::InvalidInput)?,
        ))
    })?;
    let mut code = -1_i64;
    match unsafe {
        prodex_runtime_sse_event_state_v1(
            1,
            operation as i64,
            i64::try_from(data_lines.len()).map_err(|_| MojoError::InvalidInput)?,
            address,
            length,
            i64::from(incoming_utf8_valid),
            (&mut code as *mut i64) as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match (operation, code) {
        (SseEventStateOperation::Emit | SseEventStateOperation::ParsePayload, 0) => {
            Ok(SseEventStateAction::Empty)
        }
        (SseEventStateOperation::Emit | SseEventStateOperation::ParsePayload, 1) => {
            Ok(SseEventStateAction::Discard)
        }
        (SseEventStateOperation::Emit | SseEventStateOperation::ParsePayload, 2) => {
            Ok(SseEventStateAction::Emit)
        }
        (SseEventStateOperation::IncomingData, 3) => Ok(SseEventStateAction::MarkInvalid),
        (SseEventStateOperation::IncomingData, 4) => Ok(SseEventStateAction::IgnoreIncoming),
        (SseEventStateOperation::IncomingData, 5) => Ok(SseEventStateAction::AppendIncoming),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseChunkLinePlan {
    pub newlines: Vec<usize>,
    pub consumed: usize,
}

/// Returns at most 256 ordered newline offsets with a positive consumed
/// byte count. The caller applies the plan without rescanning SSE bytes.
pub fn newline_plan(chunk: &[u8]) -> Result<SseChunkLinePlan, MojoError> {
    if chunk.is_empty() {
        return Err(MojoError::InvalidInput);
    }
    let mut offsets = [u64::MAX; MAX_BOUNDARIES];
    let mut result = [u64::MAX; 2];
    let status = unsafe {
        prodex_runtime_sse_chunk_line_offsets_v1(
            1,
            chunk.as_ptr() as u64,
            i64::try_from(chunk.len()).map_err(|_| MojoError::InvalidInput)?,
            offsets.as_mut_ptr() as u64,
            MAX_BOUNDARIES as i64,
            result.as_mut_ptr() as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let consumed = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
    let count = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
    if consumed == 0 || consumed > chunk.len() || count > MAX_BOUNDARIES {
        return Err(MojoError::InvalidOutput);
    }
    let mut previous = None;
    let newlines = offsets[..count]
        .iter()
        .map(|value| {
            let offset = usize::try_from(*value).map_err(|_| MojoError::InvalidOutput)?;
            if offset >= consumed || previous.is_some_and(|last| last >= offset) {
                return Err(MojoError::InvalidOutput);
            }
            previous = Some(offset);
            Ok(offset)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SseChunkLinePlan { newlines, consumed })
}

/// Join all SSE data bodies and remove repeated leading UTF-8 BOM in Mojo.
///
/// Rust owns the individual String allocations and serde_json consumption
/// but never formats an alternative SSE payload or strips a BOM itself.
pub fn payload_text(data_lines: &[String]) -> Result<String, MojoError> {
    if data_lines.is_empty() {
        return Err(MojoError::InvalidInput);
    }
    let capacity = data_lines
        .iter()
        .try_fold(0usize, |sum, line| {
            sum.checked_add(line.len())?.checked_add(1)
        })
        .ok_or(MojoError::InvalidInput)?;
    let views = data_lines
        .iter()
        .map(|line| {
            Ok(SseUtf8Span {
                address: line.as_ptr() as u64,
                len: u64::try_from(line.len()).map_err(|_| MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_runtime_sse_payload_join_v1(
            1,
            views.as_ptr() as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            &mut written as *mut i64 as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

/// Shared provider SSE JSON frame strategy, selected by the compiled
/// framing kernel after Serde has classified joined and separate JSON input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseJsonFrameDecision {
    Joined,
    Separate,
    Accumulate,
    LimitExceeded,
}

pub fn sse_json_frame_decision(
    joined_valid: bool,
    separate_valid: bool,
    accumulated_bytes: usize,
    fragment_bytes: usize,
    max_bytes: usize,
) -> Result<SseJsonFrameDecision, MojoError> {
    let mut tag = -1_i64;
    let status = unsafe {
        prodex_runtime_sse_json_frame_plan_v1(
            1,
            i64::from(joined_valid),
            i64::from(separate_valid),
            u64::try_from(accumulated_bytes).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(fragment_bytes).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
            (&mut tag as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match tag {
        0 => Ok(SseJsonFrameDecision::Joined),
        1 => Ok(SseJsonFrameDecision::Separate),
        2 => Ok(SseJsonFrameDecision::Accumulate),
        3 => Ok(SseJsonFrameDecision::LimitExceeded),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Flags derived only from compiled Mojo SSE pre-commit error policy.
/// Caller maps its stable HTTP error classes/actions to ABI tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SseFailureFlags {
    pub quota_blocked: bool,
    pub rate_limited: bool,
    pub overloaded: bool,
}

pub fn failure_flags(class_tag: i64, action_tag: i64) -> Result<SseFailureFlags, MojoError> {
    let mut output = u64::MAX;
    let status = unsafe {
        prodex_runtime_sse_failure_flags_v1(
            1,
            class_tag,
            action_tag,
            &mut output as *mut u64 as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    if output & !7 != 0 || output.count_ones() > 1 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(SseFailureFlags {
        quota_blocked: output & 1 != 0,
        rate_limited: output & 2 != 0,
        overloaded: output & 4 != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_sse_json_frame_plan_preserves_joined_separate_and_partial_precedence() {
        use SseJsonFrameDecision::*;
        for (joined, separate, accumulated, fragment, max, expected) in [
            (true, true, 10, 400, 12, Joined),
            (true, false, 10, 400, 12, Joined),
            (false, true, 10, 400, 12, Separate),
            (false, false, 0, 11, 12, Accumulate),
            (false, false, 1, 11, 12, Accumulate),
            (false, false, 2, 11, 12, LimitExceeded),
            (false, false, usize::MAX, 1, usize::MAX, LimitExceeded),
            (false, false, 0, 0, 0, Accumulate),
        ] {
            assert_eq!(
                sse_json_frame_decision(joined, separate, accumulated, fragment, max),
                Ok(expected),
                "{joined}/{separate}/{accumulated}/{fragment}/{max}",
            );
        }
    }

    #[test]
    fn compiled_sse_json_frame_abi_rejects_invalid_shapes_without_writing() {
        let mut result = 77_i64;
        let pointer = (&mut result as *mut i64) as usize as u64;
        let call = |version, joined, separate, output| unsafe {
            prodex_runtime_sse_json_frame_plan_v1(version, joined, separate, 2, 3, 9, output)
        };
        assert_eq!(call(2, 0, 0, pointer), 4);
        assert_eq!(call(1, 2, 0, pointer), 1);
        assert_eq!(call(1, 0, -1, pointer), 1);
        assert_eq!(call(1, 0, 0, 0), 1);
        assert_eq!(result, 77);
    }

    #[test]
    fn compiled_sse_failure_flags_cover_all_error_class_action_combinations() {
        for class_tag in 0..=5 {
            for action_tag in 0..=2 {
                let plan = failure_flags(class_tag, action_tag).unwrap();
                assert_eq!(
                    plan.quota_blocked,
                    class_tag == 1 && action_tag == 1,
                    "{class_tag}/{action_tag}"
                );
                assert_eq!(
                    plan.rate_limited,
                    class_tag == 2 && action_tag == 2,
                    "{class_tag}/{action_tag}"
                );
                assert_eq!(
                    plan.overloaded,
                    action_tag == 2 && matches!(class_tag, 4 | 5)
                        || class_tag == 3 && action_tag == 1,
                    "{class_tag}/{action_tag}"
                );
            }
        }
        for class_tag in [-1, 6] {
            assert_eq!(failure_flags(class_tag, 1), Err(MojoError::InvalidInput));
        }
        assert_eq!(failure_flags(1, 3), Err(MojoError::InvalidInput));
    }

    #[test]
    fn compiled_sse_failure_flags_reject_invalid_abi_without_writes() {
        let mut output = 99_u64;
        unsafe {
            assert_eq!(
                prodex_runtime_sse_failure_flags_v1(2, 1, 1, &mut output as *mut u64 as u64,),
                4
            );
            assert_eq!(prodex_runtime_sse_failure_flags_v1(1, 1, 1, 0), 1);
            assert_eq!(
                prodex_runtime_sse_failure_flags_v1(1, 5, -1, &mut output as *mut u64 as u64,),
                1
            );
        }
        assert_eq!(output, 99);
    }

    #[test]
    fn compiled_sse_data_payload_exact_bom_join_and_unicode_cases() {
        for (input, expected) in [
            (vec!["{}".to_owned()], "{}"),
            (vec!["".to_owned()], ""),
            (vec!["\u{feff}\u{feff}{\"x\":1}".to_owned()], "{\"x\":1}"),
            (vec!["\u{feff}".to_owned(), "next".to_owned()], "\nnext"),
            (
                vec!["a".to_owned(), "".to_owned(), "b".to_owned()],
                "a\n\nb",
            ),
            (
                vec!["".to_owned(), "\u{feff}next".to_owned()],
                "\n\u{feff}next",
            ),
            (
                vec!["東京🧭".to_owned(), "line2".to_owned()],
                "東京🧭\nline2",
            ),
        ] {
            assert_eq!(payload_text(&input).unwrap(), expected);
        }
        assert_eq!(payload_text(&[]), Err(MojoError::InvalidInput));
        let lines = vec!["x".to_owned(); 600];
        assert_eq!(payload_text(&lines).unwrap(), lines.join("\n"));
    }

    #[test]
    fn compiled_sse_payload_join_rejects_abi_and_capacity_without_writes() {
        let first = SseUtf8Span {
            address: b"abc".as_ptr() as u64,
            len: 3,
        };
        let mut output = [37_u8; 6];
        let mut written = 88_i64;
        unsafe {
            assert_eq!(
                prodex_runtime_sse_payload_join_v1(
                    2,
                    &first as *const _ as u64,
                    1,
                    output.as_mut_ptr() as u64,
                    6,
                    &mut written as *mut _ as u64
                ),
                4,
            );
            assert_eq!(
                prodex_runtime_sse_payload_join_v1(
                    1,
                    &first as *const _ as u64,
                    1,
                    output.as_mut_ptr() as u64,
                    2,
                    &mut written as *mut _ as u64
                ),
                3,
            );
            assert_eq!(
                prodex_runtime_sse_payload_join_v1(
                    1,
                    0,
                    1,
                    output.as_mut_ptr() as u64,
                    6,
                    &mut written as *mut _ as u64
                ),
                1,
            );
        }
        assert_eq!(output, [37; 6]);
        assert_eq!(written, 88);
    }

    #[test]
    fn compiled_chunk_scanner_returns_bounded_sorted_offsets_across_batches() {
        let body = vec![b'\n'; 600];
        let first = newline_plan(&body).unwrap();
        assert_eq!(first.consumed, 256);
        assert_eq!(first.newlines, (0..256).collect::<Vec<_>>());
        let second = newline_plan(&body[256..]).unwrap();
        assert_eq!(second.consumed, 256);
        assert_eq!(second.newlines, (0..256).collect::<Vec<_>>());
        let last = newline_plan(&body[512..]).unwrap();
        assert_eq!(last.consumed, 88);
        assert_eq!(last.newlines, (0..88).collect::<Vec<_>>());
        assert_eq!(
            newline_plan(b"unterminated"),
            Ok(SseChunkLinePlan {
                newlines: vec![],
                consumed: "unterminated".len(),
            })
        );
        assert_eq!(
            newline_plan(b"\r\ndata:x\n"),
            Ok(SseChunkLinePlan {
                newlines: vec![1, 8],
                consumed: 9,
            })
        );
    }

    #[test]
    fn compiled_chunk_scanner_rejects_bad_abi_before_mutating_output() {
        assert_eq!(newline_plan(b""), Err(MojoError::InvalidInput));
        let bytes = b"\n\n";
        let mut offsets = [55_u64; MAX_BOUNDARIES];
        let mut result = [77_u64; 2];
        unsafe {
            assert_eq!(
                prodex_runtime_sse_chunk_line_offsets_v1(
                    2,
                    bytes.as_ptr() as u64,
                    2,
                    offsets.as_mut_ptr() as u64,
                    256,
                    result.as_mut_ptr() as u64,
                ),
                4,
            );
            assert_eq!(
                prodex_runtime_sse_chunk_line_offsets_v1(
                    1,
                    0,
                    2,
                    offsets.as_mut_ptr() as u64,
                    256,
                    result.as_mut_ptr() as u64,
                ),
                1,
            );
            assert_eq!(
                prodex_runtime_sse_chunk_line_offsets_v1(
                    1,
                    bytes.as_ptr() as u64,
                    2,
                    offsets.as_mut_ptr() as u64,
                    257,
                    result.as_mut_ptr() as u64,
                ),
                1,
            );
        }
        assert_eq!(offsets, [55; MAX_BOUNDARIES]);
        assert_eq!(result, [77; 2]);
    }
}

#[cfg(test)]
mod sse_event_state_tests {
    use super::*;

    #[test]
    fn compiled_sse_event_state_selects_only_valid_buffer_transitions() {
        let empty = Vec::<String>::new();
        let present = vec!["first line".to_string(), "second line".to_string()];
        let poisoned = vec!["\0prodex-invalid-sse-data".to_string()];
        let almost = vec!["\0prodex-invalid-sse-data-extra".to_string()];
        let empty_first = vec![String::new(), "next".to_string()];
        for (data, expected) in [
            (&empty, SseEventStateAction::Empty),
            (&present, SseEventStateAction::Emit),
            (&poisoned, SseEventStateAction::Discard),
            (&almost, SseEventStateAction::Emit),
            (&empty_first, SseEventStateAction::Emit),
        ] {
            for operation in [
                SseEventStateOperation::Emit,
                SseEventStateOperation::ParsePayload,
            ] {
                assert_eq!(sse_event_state_action(data, operation, true), Ok(expected));
            }
        }
        assert_eq!(
            sse_event_state_action(&empty, SseEventStateOperation::IncomingData, true),
            Ok(SseEventStateAction::AppendIncoming),
        );
        assert_eq!(
            sse_event_state_action(&present, SseEventStateOperation::IncomingData, true),
            Ok(SseEventStateAction::AppendIncoming),
        );
        assert_eq!(
            sse_event_state_action(&poisoned, SseEventStateOperation::IncomingData, true),
            Ok(SseEventStateAction::IgnoreIncoming),
        );
        assert_eq!(
            sse_event_state_action(&poisoned, SseEventStateOperation::IncomingData, false),
            Ok(SseEventStateAction::MarkInvalid),
        );
        assert_eq!(
            sse_event_state_action(&present, SseEventStateOperation::IncomingData, false),
            Ok(SseEventStateAction::MarkInvalid),
        );
    }

    #[test]
    fn compiled_sse_event_state_abi_rejects_invalid_shapes_without_output_write() {
        let marker = b"\0prodex-invalid-sse-data";
        let mut result = 91_i64;
        let address = marker.as_ptr() as usize as u64;
        let output = (&mut result as *mut i64) as usize as u64;
        for (abi, operation, count, first, length, utf8, dest, expected) in [
            (2, 0, 1, address, marker.len() as i64, 1, output, 4),
            (1, -1, 1, address, marker.len() as i64, 1, output, 1),
            (1, 3, 1, address, marker.len() as i64, 1, output, 1),
            (1, 0, -1, address, marker.len() as i64, 1, output, 1),
            (1, 0, 1, 0, marker.len() as i64, 1, output, 1),
            (1, 0, 0, address, marker.len() as i64, 1, output, 1),
            (1, 0, 1, address, -1, 1, output, 1),
            (1, 0, 1, address, marker.len() as i64, -1, output, 1),
            (1, 0, 1, address, marker.len() as i64, 2, output, 1),
            (1, 0, 1, address, marker.len() as i64, 1, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_runtime_sse_event_state_v1(
                        abi, operation, count, first, length, utf8, dest,
                    )
                },
                expected,
            );
            assert_eq!(result, 91);
        }
        assert_eq!(
            unsafe {
                prodex_runtime_sse_event_state_v1(1, 0, 1, address, marker.len() as i64, 1, output)
            },
            0,
        );
        assert_eq!(result, 1, "poisoned event must be discarded");
    }
}

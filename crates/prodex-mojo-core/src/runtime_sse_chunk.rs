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

#[cfg(test)]
mod tests {
    use super::*;

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

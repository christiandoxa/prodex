//! Typed ABI for the compiled Mojo SSE chunk framing kernel.
//! The host owns transport buffers, but Mojo alone locates LF boundaries.

use crate::MojoError;

const MAX_BOUNDARIES: usize = 256;

unsafe extern "C" {
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

#[cfg(test)]
mod tests {
    use super::*;

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

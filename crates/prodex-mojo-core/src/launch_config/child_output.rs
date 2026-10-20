//! Borrowed stdout/stderr line separator selection for child process launch.
//! The host owns Read, timing callbacks and the original Vec<u8>. Compiled
//! Mojo is the sole owner of the LF scanner and line endpoint ordering.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_BYTES: usize = 16 * 1024 * 1024;

unsafe extern "C" {
    fn prodex_launch_process_line_ends_v1(
        abi_version: i64,
        buffer_address: u64,
        buffer_length: i64,
        start: i64,
        output_address: u64,
        output_capacity: i64,
        measuring: i64,
        count_address: u64,
    ) -> i64;
}

fn status(code: i64) -> Result<(), MojoError> {
    match code {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Select original absolute LF positions in a binary child output buffer.
/// A second call materializes the same exact count measured by Mojo.
pub fn launch_process_line_ends(buffer: &[u8], start: usize) -> Result<Vec<usize>, MojoError> {
    if buffer.len() > MAX_BYTES || start > buffer.len() {
        return Err(MojoError::InvalidInput);
    }
    let address = buffer.as_ptr() as usize as u64;
    let len = i64::try_from(buffer.len()).map_err(|_| MojoError::InvalidInput)?;
    let start = i64::try_from(start).map_err(|_| MojoError::InvalidInput)?;
    let mut count = -1_i64;
    let count_address = (&mut count as *mut i64) as usize as u64;
    status(unsafe {
        prodex_launch_process_line_ends_v1(ABI_VERSION, address, len, start, 0, 0, 1, count_address)
    })?;
    let count = usize::try_from(count).map_err(|_| MojoError::InvalidOutput)?;
    if count > buffer.len() - start as usize {
        return Err(MojoError::InvalidOutput);
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut output = Vec::<i64>::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(count, -1);
    let mut written = -1_i64;
    status(unsafe {
        prodex_launch_process_line_ends_v1(
            ABI_VERSION,
            address,
            len,
            start,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            0,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    if usize::try_from(written).map_err(|_| MojoError::InvalidOutput)? != count {
        return Err(MojoError::InvalidOutput);
    }
    let mut previous = start as usize;
    let mut ends = Vec::with_capacity(count);
    for end in output {
        let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
        if end < previous || end >= buffer.len() || buffer[end] != b'\n' {
            return Err(MojoError::InvalidOutput);
        }
        previous = end + 1;
        ends.push(end);
    }
    Ok(ends)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_launch_process_line_ends_handles_binary_and_partial_reads() {
        let bytes = b"one\n\xff\nthree\npartial";
        assert_eq!(launch_process_line_ends(bytes, 0), Ok(vec![3, 5, 11]));
        assert_eq!(launch_process_line_ends(bytes, 4), Ok(vec![5, 11]));
        assert_eq!(launch_process_line_ends(bytes, 5), Ok(vec![5, 11]));
        assert_eq!(launch_process_line_ends(bytes, 6), Ok(vec![11]));
        assert_eq!(launch_process_line_ends(bytes, bytes.len()), Ok(vec![]));
        assert_eq!(launch_process_line_ends(b"", 0), Ok(vec![]));
        assert_eq!(launch_process_line_ends(b"no newline", 0), Ok(vec![]));
        assert_eq!(launch_process_line_ends(b"\n\n", 0), Ok(vec![0, 1]));
        assert_eq!(
            launch_process_line_ends(b"short", 6),
            Err(MojoError::InvalidInput)
        );
        let many = b"line\n".repeat(1025);
        let ends = launch_process_line_ends(&many, 0).unwrap();
        assert_eq!(ends.len(), 1025);
        assert_eq!(ends[0], 4);
        assert_eq!(ends[1024], many.len() - 1);
    }

    #[test]
    fn compiled_launch_process_line_ends_abi_rejects_bad_capacity_without_writes() {
        let bytes = b"one\ntwo\n";
        let mut output = [71_i64; 2];
        let mut count = 83_i64;
        let source = bytes.as_ptr() as usize as u64;
        let result = output.as_mut_ptr() as usize as u64;
        let count_ptr = (&mut count as *mut i64) as usize as u64;
        for (abi, ptr, len, start, capacity, mode, dest, expected) in [
            (2, source, 8, 0, 2, 0, count_ptr, 4),
            (1, 0, 8, 0, 2, 0, count_ptr, 1),
            (1, source, -1, 0, 2, 0, count_ptr, 1),
            (1, source, 8, -1, 2, 0, count_ptr, 1),
            (1, source, 8, 9, 2, 0, count_ptr, 1),
            (1, source, 8, 0, 1, 0, count_ptr, 3),
            (1, source, 8, 0, 2, 2, count_ptr, 1),
            (1, source, 8, 0, 2, 0, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_launch_process_line_ends_v1(
                        abi, ptr, len, start, result, capacity, mode, dest,
                    )
                },
                expected,
            );
            assert_eq!(output, [71; 2]);
            assert_eq!(count, 83);
        }
        assert_eq!(
            unsafe { prodex_launch_process_line_ends_v1(1, source, 8, 0, 0, 0, 1, count_ptr,) },
            0,
        );
        assert_eq!(output, [71; 2]);
        assert_eq!(count, 2);
    }
}

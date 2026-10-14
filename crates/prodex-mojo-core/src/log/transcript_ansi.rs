//! Mandatory compiled Mojo implementation of transcript ANSI normalization.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_transcript_ansi_strip_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Normalize ANSI CSI, OSC, arbitrary ESC scalars, CRLF and lone CR in
/// persisted transcript tool output without a Rust semantic fallback.
pub fn strip_transcript_ansi_codes(input: &str) -> Result<String, MojoError> {
    let capacity = i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = vec![0u8; input.len()];
    let mut written = -1i64;
    let status = unsafe {
        prodex_mojo_transcript_ansi_strip_v1(
            1,
            input.as_ptr() as usize as u64,
            capacity,
            output.as_mut_ptr() as usize as u64,
            capacity,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(count);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_mojo_preserves_utf8_and_escape_boundary_semantics() {
        for (input, expected) in [
            ("", ""),
            ("plain ASCII", "plain ASCII"),
            ("\u{1b}[31mred\u{1b}[0m", "red"),
            ("\u{1b}]0;caption\u{7}ready", "ready"),
            ("\u{1b}]0;caption\u{1b}\\done", "done"),
            ("x\r\ny\rz", "x\ny\nz"),
            ("表🙂 \u{1b}🙂safe", "表🙂 safe"),
            ("abc\u{1b}", "abc"),
            ("\u{1b}[unfinished", "nfinished"),
            ("\u{1b}[123", ""),
            ("\u{1b}]unfinished", ""),
            ("a\u{1b}[8msecret", "asecret"),
            ("a\u{1b}]text\u{1b}\\b", "ab"),
        ] {
            assert_eq!(
                strip_transcript_ansi_codes(input).unwrap(),
                expected,
                "{input:?}"
            );
        }
    }

    #[test]
    fn compiled_mojo_rejects_bad_abi_pointers_and_partial_output() {
        let mut output = [0u8; 2];
        let mut written = 123i64;
        let source = b"normal";
        let addr = output.as_mut_ptr() as usize as u64;
        let written_addr = (&mut written as *mut i64) as usize as u64;
        assert_eq!(
            unsafe { prodex_mojo_transcript_ansi_strip_v1(2, 0, 6, addr, 2, written_addr) },
            4
        );
        assert_eq!(
            unsafe { prodex_mojo_transcript_ansi_strip_v1(1, 0, 6, addr, 2, written_addr) },
            1
        );
        assert_eq!(
            unsafe {
                prodex_mojo_transcript_ansi_strip_v1(
                    1,
                    source.as_ptr() as usize as u64,
                    6,
                    addr,
                    2,
                    written_addr,
                )
            },
            2
        );
        assert_eq!(
            written, 123,
            "failed normalization must not publish a byte count"
        );
    }
}

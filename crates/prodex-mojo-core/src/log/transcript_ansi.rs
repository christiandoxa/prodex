//! Mandatory compiled Mojo implementation of transcript ANSI normalization.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_transcript_binary_policy_v1(
        abi_version: i64,
        mode: i64,
        records_address: u64,
        record_count: i64,
        result_address: u64,
    ) -> i64;
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

/// Rust supplies Unicode table facts, while compiled-in Mojo exclusively
/// classifies suspicious terminal output, text controls, and mojibake.
pub fn transcript_binary_like(text: &str, line: bool) -> Result<bool, MojoError> {
    let packed = text
        .chars()
        .take(4096)
        .map(|ch| {
            u64::from(ch as u32)
                | (u64::from(ch.is_whitespace()) << 21)
                | (u64::from(ch.is_alphanumeric()) << 22)
                | (u64::from(ch.is_control()) << 23)
        })
        .collect::<Vec<_>>();
    let mut result = u64::MAX;
    let status = unsafe {
        prodex_mojo_transcript_binary_policy_v1(
            1,
            i64::from(line),
            packed.as_ptr() as usize as u64,
            i64::try_from(packed.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut result as *mut u64) as usize as u64,
        )
    };
    match status {
        0 => match result {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        },
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
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
    fn compiled_mojo_owns_binary_mojibake_unicode_threshold_policy() {
        for (input, line, bad) in [
            ("", true, false),
            ("", false, false),
            ("ASCII text 123 + !!", true, false),
            ("表🙂", true, true),
            ("hello 表🙂", true, false),
            ("hello 表🙂", false, false),
            ("\u{fffd}", true, true),
            ("\u{fffd}", false, true),
            ("\u{fffd}\u{fffd}\u{fffd}", false, true),
            ("abc\u{0}def", true, true),
            ("abc\u{0}def", false, true),
            ("💥👍😊", true, true),
            ("💥👍😊", false, true),
            ("éééééééé", true, false),
            ("a1234xyz", false, false),
        ] {
            assert_eq!(
                transcript_binary_like(input, line).unwrap(),
                bad,
                "{input:?}/{line}",
            );
        }
    }

    #[test]
    fn binary_policy_rejects_bad_abi_unknown_flags_and_record_count() {
        let mut result = 17u64;
        let ptr = (&mut result as *mut u64) as usize as u64;
        let records = [0u64, 0x1_000_000u64, 0xD800u64];
        let address = records.as_ptr() as usize as u64;
        assert_eq!(
            unsafe { prodex_mojo_transcript_binary_policy_v1(2, 1, address, 1, ptr) },
            4
        );
        assert_eq!(
            unsafe { prodex_mojo_transcript_binary_policy_v1(1, 2, address, 1, ptr) },
            1
        );
        assert_eq!(
            unsafe { prodex_mojo_transcript_binary_policy_v1(1, 1, 0, 1, ptr) },
            1
        );
        assert_eq!(
            unsafe { prodex_mojo_transcript_binary_policy_v1(1, 1, address, 4097, ptr) },
            1
        );
        assert_eq!(
            unsafe { prodex_mojo_transcript_binary_policy_v1(1, 1, address, 2, ptr) },
            1
        );
        assert_eq!(
            unsafe {
                prodex_mojo_transcript_binary_policy_v1(
                    1,
                    1,
                    records[2..].as_ptr() as usize as u64,
                    1,
                    ptr,
                )
            },
            1
        );
        assert_eq!(result, 17);
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

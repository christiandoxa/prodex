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
    fn prodex_mojo_transcript_visible_text_v1(
        abi_version: i64,
        mode: i64,
        input_address: u64,
        input_length: i64,
        facts_address: u64,
        fact_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_mojo_transcript_safe_text_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        facts_address: u64,
        fact_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_mojo_transcript_join_user_parts_v1(
        abi_version: i64,
        addresses_address: u64,
        lengths_address: u64,
        part_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

const TRANSCRIPT_ABI_VERSION: i64 = 1;
const TRANSCRIPT_VISIBLE_MODE_MESSAGE: i64 = 0;
const TRANSCRIPT_VISIBLE_MODE_TOOL: i64 = 1;
const TRANSCRIPT_SAFE_MAX_BYTES: usize = 64 * 1024;

fn transcript_output_string(output: Vec<u8>, written: i64) -> Result<String, MojoError> {
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    String::from_utf8(
        output
            .get(..written)
            .ok_or(MojoError::InvalidOutput)?
            .to_vec(),
    )
    .map_err(|_| MojoError::InvalidOutput)
}

fn transcript_unicode_facts(text: &str) -> Vec<u64> {
    text.chars()
        .map(|ch| {
            u64::from(ch as u32)
                | (u64::from(ch.is_whitespace()) << 21)
                | (u64::from(ch.is_alphanumeric()) << 22)
                | (u64::from(ch.is_control()) << 23)
        })
        .collect()
}

fn transcript_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 | 2 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Apply Mojo-owned line visibility and blank-line projection after Rust has
/// supplied the standard library's Unicode facts.
pub fn normalize_transcript_visible_text(
    input: &str,
    tool_output: bool,
) -> Result<Option<String>, MojoError> {
    let facts = transcript_unicode_facts(input);
    let mut output = vec![0_u8; input.len().max(1)];
    let mut written = -1_i64;
    transcript_status(unsafe {
        prodex_mojo_transcript_visible_text_v1(
            TRANSCRIPT_ABI_VERSION,
            if tool_output {
                TRANSCRIPT_VISIBLE_MODE_TOOL
            } else {
                TRANSCRIPT_VISIBLE_MODE_MESSAGE
            },
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            facts.as_ptr() as usize as u64,
            i64::try_from(facts.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let output = transcript_output_string(output, written)?;
    Ok((!output.is_empty()).then_some(output))
}

/// Remove unsafe controls and apply the transcript's 64 KiB UTF-8 byte bound
/// after secret masking has already happened in the redaction owner.
pub fn normalize_transcript_safe_text(input: &str) -> Result<String, MojoError> {
    let facts = transcript_unicode_facts(input);
    let mut output = vec![0_u8; input.len().clamp(1, TRANSCRIPT_SAFE_MAX_BYTES)];
    let mut written = -1_i64;
    transcript_status(unsafe {
        prodex_mojo_transcript_safe_text_v1(
            TRANSCRIPT_ABI_VERSION,
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            facts.as_ptr() as usize as u64,
            i64::try_from(facts.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    transcript_output_string(output, written)
}

/// Join content parts with the exact newline separator used by Codex message
/// delivery verification. No whitespace or redaction normalization is applied.
pub fn join_transcript_user_parts(parts: &[&str]) -> Result<String, MojoError> {
    let addresses = parts
        .iter()
        .map(|part| part.as_ptr() as usize as u64)
        .collect::<Vec<_>>();
    let lengths = parts
        .iter()
        .map(|part| i64::try_from(part.len()).map_err(|_| MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let capacity = parts
        .iter()
        .enumerate()
        .try_fold(0_usize, |total, (index, part)| {
            total
                .checked_add(part.len())
                .and_then(|value| value.checked_add(usize::from(index > 0)))
                .ok_or(MojoError::Capacity)
        })?;
    let mut output = vec![0_u8; capacity.max(1)];
    let mut written = -1_i64;
    transcript_status(unsafe {
        prodex_mojo_transcript_join_user_parts_v1(
            TRANSCRIPT_ABI_VERSION,
            addresses.as_ptr() as usize as u64,
            lengths.as_ptr() as usize as u64,
            i64::try_from(parts.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    transcript_output_string(output, written)
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
    fn compiled_mojo_projects_visible_lines_and_exact_user_parts() {
        assert_eq!(
            normalize_transcript_visible_text(
                "please compare\n\nimage file: /home/test-user/.prodex/profiles/.prodex-overlay-123/attachments/a.png\nthen summarize\n",
                false,
            )
            .unwrap(),
            Some("please compare\n\nthen summarize".to_string()),
        );
        assert_eq!(
            normalize_transcript_visible_text("\u{fffd}\u{fffd}\u{fffd}\u{0}bad", true).unwrap(),
            None,
        );
        assert_eq!(
            join_transcript_user_parts(&[" first \n", "second\t"]).unwrap(),
            " first \n\nsecond\t",
        );
    }

    #[test]
    fn compiled_mojo_safe_text_preserves_controls_and_unicode_byte_bound() {
        assert_eq!(
            normalize_transcript_safe_text("a\u{0}b\n\t表🙂").unwrap(),
            "a b\n\t表🙂",
        );
        let text = normalize_transcript_safe_text(&"é".repeat(40_000)).unwrap();
        assert!(text.ends_with(" …[truncated]"));
        assert!(text.len() <= 64 * 1024);
        assert!(text.is_char_boundary(text.len()));
    }

    #[test]
    fn compiled_mojo_visibility_preserves_the_binary_scalar_sample_boundary() {
        for prefix in ["a".repeat(4096), format!("表{}", "a".repeat(4095))] {
            let text = format!("{prefix}\u{0}tail");
            assert!(!transcript_binary_like(&text, true).unwrap());
            assert_eq!(
                normalize_transcript_visible_text(&text, true).unwrap(),
                Some(text),
            );
        }
        let text = format!("{}\u{0}tail", "a".repeat(4095));
        assert!(transcript_binary_like(&text, true).unwrap());
        assert_eq!(
            normalize_transcript_visible_text(&text, true).unwrap(),
            None
        );
    }

    #[test]
    fn compiled_mojo_accepts_large_parts_and_sources_before_output_truncation() {
        let parts = vec!["x"; 65_537];
        assert_eq!(
            join_transcript_user_parts(&parts).unwrap(),
            parts.join("\n")
        );
        assert_eq!(
            normalize_transcript_safe_text(&"x".repeat(4_000_001)).unwrap(),
            format!("{} …[truncated]", "x".repeat(64 * 1024 - 16)),
        );
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
    fn transcript_projection_abi_rejects_bad_version_and_fact_shape() {
        let input = "ok";
        let facts = transcript_unicode_facts(input);
        let mut output = [91_u8; 8];
        let mut written = 77_i64;
        let mut args = |version, mode, facts_address, fact_count, output_capacity| unsafe {
            prodex_mojo_transcript_visible_text_v1(
                version,
                mode,
                input.as_ptr() as usize as u64,
                input.len() as i64,
                facts_address,
                fact_count,
                output.as_mut_ptr() as usize as u64,
                output_capacity,
                (&mut written as *mut i64) as usize as u64,
            )
        };
        let facts_address = facts.as_ptr() as usize as u64;
        assert_eq!(args(2, 0, facts_address, 2, 8), 4);
        assert_eq!(args(1, 2, facts_address, 2, 8), 1);
        assert_eq!(args(1, 0, facts_address, 1, 8), 2);
        assert_eq!(args(1, 0, 0, 2, 8), 2);
        assert_eq!(args(1, 0, facts_address, 2, 1), 3);
        assert_eq!(output, [91_u8; 8]);
        assert_eq!(written, 77);
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

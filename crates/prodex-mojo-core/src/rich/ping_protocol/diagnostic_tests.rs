use super::*;
use crate::rich::{
    PING_STATUS_AUTH_FAILED, PING_STATUS_PROCESS_FAILED, PING_STATUS_SPAWN_FAILED,
    PING_STATUS_TIMEOUT,
};

#[test]
fn compiled_ping_details_preserve_normalization_utf8_prefixes_and_unbounded_acquisition() {
    for (text, expected) in [
        ("", ""),
        (" \r\n\u{2003}", ""),
        ("\u{2003}a\r\nb\u{3000}", "a  b"),
        ("a\tb\0", "a\tb\0"),
        ("\u{feff}x", "\u{feff}x"),
    ] {
        assert_eq!(ping_bounded_detail(text), Ok(expected.to_string()));
    }
    for count in 4092..=4096 {
        let text = "x".repeat(count) + "🧭more";
        let prefix = if count == 4092 {
            "x".repeat(count) + "🧭"
        } else {
            "x".repeat(count)
        };
        assert_eq!(ping_bounded_detail(&text), Ok(prefix));
    }
    assert_eq!(
        ping_bounded_detail(&"x".repeat(4 * 1024 * 1024 + 1)),
        Ok("x".repeat(PING_ERROR_DETAIL_MAX_BYTES))
    );
}

#[test]
fn compiled_ping_process_details_keep_raw_classification_and_redacted_output_separate() {
    for (raw, redacted, status, expected) in [
        (
            "",
            "",
            PING_STATUS_PROCESS_FAILED,
            "OpenAI application ping process failed",
        ),
        (
            "timed out",
            "safe",
            PING_STATUS_TIMEOUT,
            "OpenAI diagnostic timed out: safe",
        ),
        (
            "failed to start",
            "safe",
            PING_STATUS_SPAWN_FAILED,
            "OpenAI application ping could not start: safe",
        ),
        (
            "clean the diagnostic directory",
            "safe",
            PING_STATUS_PROCESS_FAILED,
            "OpenAI application ping cleanup failed: safe",
        ),
        (
            "CLEAN THE DIAGNOSTIC DIRECTORY",
            "safe",
            PING_STATUS_PROCESS_FAILED,
            "OpenAI application ping process failed: safe",
        ),
        (
            "authentication clean the diagnostic directory",
            "safe",
            PING_STATUS_AUTH_FAILED,
            "OpenAI authentication failed: safe",
        ),
        (
            "SENSITIVE_RAW authentication",
            "\u{2003}safe\r\ntext\u{3000}",
            PING_STATUS_AUTH_FAILED,
            "OpenAI authentication failed: safe  text",
        ),
        (
            "timeout",
            "OpenAI diagnostic timed out",
            PING_STATUS_TIMEOUT,
            "OpenAI diagnostic timed out",
        ),
    ] {
        assert_eq!(
            ping_process_error(raw, redacted),
            Ok((status, expected.to_string()))
        );
    }
    assert_eq!(
        ping_process_error(&"x".repeat(4 * 1024 * 1024 + 1), "safe"),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_ping_output_details_preserve_native_exit_codes_and_large_base_labels() {
    for (exit, stderr, expected) in [
        (PingChildExit::Success, "", "base"),
        (PingChildExit::Success, " \r\n ", "base"),
        (
            PingChildExit::Success,
            "  detail\r\nmore ",
            "base: stderr: detail  more",
        ),
        (PingChildExit::Failed(1), "", "base (exit code 1)"),
        (
            PingChildExit::Failed(-1),
            "safe",
            "base (exit code -1): stderr: safe",
        ),
        (
            PingChildExit::Failed(i32::MIN),
            "safe",
            "base (exit code -2147483648): stderr: safe",
        ),
    ] {
        assert_eq!(
            ping_output_failure_detail("base", exit, stderr),
            Ok(expected.to_string())
        );
    }
    let base = "🧭".repeat(65_537);
    assert_eq!(
        ping_output_failure_detail(&base, PingChildExit::Success, ""),
        Ok(base)
    );
}

#[test]
fn ping_diagnostic_abi_rejects_bad_fields_utf8_and_capacity_before_writes() {
    let raw = "timed out";
    let redacted = "safe";
    let mut output = [77_u8; 64];
    let mut metadata = [71_i64; 2];
    let destination = output.as_mut_ptr() as u64;
    let meta = metadata.as_mut_ptr() as u64;
    let call = |version,
                operation,
                source,
                raw_len,
                safe,
                safe_len,
                failed,
                code,
                capacity,
                meta| unsafe {
        prodex_mojo_ping_diagnostic_v1(
            version,
            operation,
            source,
            raw_len,
            safe,
            safe_len,
            failed,
            code,
            destination,
            capacity,
            meta,
        )
    };
    let source = raw.as_ptr() as u64;
    let safe = redacted.as_ptr() as u64;
    assert_eq!(call(2, 1, source, 9, safe, 4, 0, 0, 64, meta), 4);
    for (operation, raw_len, safe_len, failed, code) in [
        (3, 9, 4, 0, 0),
        (1, -1, 4, 0, 0),
        (1, 9, -1, 0, 0),
        (1, 9, 4, 1, 1),
        (2, 9, 4, 0, 1),
        (2, 9, 4, 2, 0),
        (2, 9, 4, 1, i64::MAX),
    ] {
        assert_eq!(
            call(
                1, operation, source, raw_len, safe, safe_len, failed, code, 64, meta
            ),
            1
        );
    }
    assert_eq!(call(1, 1, source, 9, safe, 4, 0, 0, 1, meta), 3);
    assert_eq!(call(1, 1, source, 9, safe, 4, 0, 0, 64, 1), 1);
    let invalid = [0xff_u8];
    assert_eq!(
        call(1, 1, source, 9, invalid.as_ptr() as u64, 1, 0, 0, 64, meta),
        2
    );
    assert_eq!(output, [77; 64]);
    assert_eq!(metadata, [71; 2]);
    assert_eq!(call(1, 1, source, 9, safe, 4, 0, 0, 64, meta), 0);
    assert_eq!(metadata[0], PING_STATUS_TIMEOUT);
    assert_eq!(
        &output[..metadata[1] as usize],
        b"OpenAI diagnostic timed out: safe"
    );
}

use super::super::{PingStatus, ping_process};

#[test]
fn process_error_diagnostics_preserve_precedence_cleanup_case_and_native_error_chains() {
    for (message, status, expected) in [
        (
            "",
            PingStatus::ProcessFailed,
            "OpenAI application ping process failed",
        ),
        (
            "timed out",
            PingStatus::Timeout,
            "OpenAI diagnostic timed out: timed out",
        ),
        (
            "failed to start",
            PingStatus::SpawnFailed,
            "OpenAI application ping could not start: failed to start",
        ),
        (
            "clean the diagnostic directory",
            PingStatus::ProcessFailed,
            "OpenAI application ping cleanup failed: clean the diagnostic directory",
        ),
        (
            "CLEAN THE DIAGNOSTIC DIRECTORY",
            PingStatus::ProcessFailed,
            "OpenAI application ping process failed: CLEAN THE DIAGNOSTIC DIRECTORY",
        ),
        (
            "OpenAI application ping process failed",
            PingStatus::ProcessFailed,
            "OpenAI application ping process failed",
        ),
    ] {
        let error = anyhow::anyhow!(message.to_string());
        assert_eq!(
            ping_process::classify_ping_process_error(&error),
            (status, expected.to_string())
        );
    }
    let error = anyhow::anyhow!("timed out").context("wrapper");
    assert_eq!(
        ping_process::classify_ping_process_error(&error),
        (
            PingStatus::Timeout,
            "OpenAI diagnostic timed out: wrapper: timed out".to_string()
        )
    );
}

#[test]
fn ping_diagnostics_redact_before_normalization_and_keep_utf8_byte_bounds() {
    let secret = "Bearer FAKE";
    let error = anyhow::anyhow!("clean the diagnostic directory: {secret}\r\nmore");
    let (status, detail) = ping_process::classify_ping_process_error(&error);
    assert_eq!(status, PingStatus::ProcessFailed);
    assert!(detail.starts_with("OpenAI application ping cleanup failed: "));
    assert!(!detail.contains(secret));
    assert!(!detail.contains(['\r', '\n']));
    let text = " \u{2003}".to_string() + &"x".repeat(4095) + "🧭tail\u{3000}";
    assert_eq!(ping_process::bounded_ping_detail(&text), "x".repeat(4095));
}

#[test]
fn ping_output_diagnostics_keep_exit_stderr_and_lossy_native_output_acquisition() {
    let output = super::output_with_stderr("", "  detail\r\nmore  ", false);
    assert_eq!(
        ping_process::ping_output_failure_detail("base", &output),
        "base (exit code 1): stderr: detail  more"
    );
    let mut output = super::output_with_stderr("", "", true);
    output.stderr = vec![0xff, 0];
    assert_eq!(
        ping_process::ping_output_failure_detail("base", &output),
        "base: stderr: �\0"
    );
}

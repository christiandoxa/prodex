#[cfg(unix)]
use super::ping_process::run_ping_child;
use super::ping_process::{PING_ERROR_DETAIL_MAX_BYTES, ping_command_args};
use super::{
    PING_PROMPT, PingOpenaiArgs, PingProbeOptions, PingStatus, classify_failure_text,
    ping_result_from_output, validate_ping_output,
};
#[cfg(unix)]
use crate::ChildProcessPlan;
#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::fs;
use std::process::Output;
#[cfg(unix)]
use std::time::Duration;
use std::time::Instant;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;

fn output(stdout: &str, success: bool) -> Output {
    output_with_stderr(stdout, "", success)
}

fn output_with_stderr(stdout: &str, stderr: &str, success: bool) -> Output {
    let status = if success {
        std::process::ExitStatus::from_raw(0)
    } else {
        #[cfg(unix)]
        {
            std::process::ExitStatus::from_raw(1 << 8)
        }
        #[cfg(windows)]
        {
            std::process::ExitStatus::from_raw(1)
        }
    };
    Output {
        status,
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

#[test]
fn completed_model_response_is_required_for_pass() {
    let result = validate_ping_output(&output(
        r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"item.completed","item":{"type":"agent_message","text":"Hello"}}
{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#,
        true,
    ));
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn exact_ping_text_uses_the_canonical_codex_exec_path() {
    let args = PingOpenaiArgs {
        profile: Some("profile".to_string()),
        model: Some("gpt-5.6-luna".to_string()),
        base_url: Some("https://example.com".to_string()),
        no_proxy: true,
        json: false,
    };
    let command = ping_command_args(&PingProbeOptions {
        model: args.model,
        base_url: args.base_url,
        no_proxy: args.no_proxy,
    });
    let values = command
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(values.first().map(String::as_str), Some("exec"));
    assert_eq!(values.last().map(String::as_str), Some(PING_PROMPT));
    assert!(!values.iter().any(|value| value == "run"));
    assert!(!values.iter().any(|value| value == "--profile"));
    assert!(!values.iter().any(|value| value == "--base-url"));
    assert!(
        values
            .windows(2)
            .any(|pair| pair == ["--model", "gpt-5.6-luna"])
    );
    assert!(!values.iter().any(|value| value == "--no-proxy"));
    assert!(
        values
            .windows(2)
            .any(|pair| pair == ["-c", "model_provider=\"openai\""])
    );
    assert!(
        values
            .windows(2)
            .any(|pair| pair == ["-c", "chatgpt_base_url=\"https://example.com\""])
    );
    assert!(values.iter().any(|value| value == "exec"));
    let exec_index = values.iter().position(|value| value == "exec").unwrap();
    let ephemeral_index = values
        .iter()
        .position(|value| value == "--ephemeral")
        .unwrap();
    let json_index = values.iter().position(|value| value == "--json").unwrap();
    assert!(exec_index < ephemeral_index);
    assert!(exec_index < json_index);
    assert!(values.windows(2).any(|pair| pair == ["--color", "never"]));
}

#[test]
fn exit_zero_without_completed_turn_is_not_success() {
    let result = validate_ping_output(&output(
        r#"{"type":"thread.started","thread_id":"t"}"#,
        true,
    ));
    assert_eq!(result.unwrap_err().status, PingStatus::ProtocolFailed);
}

#[test]
fn tool_activity_is_rejected_even_with_a_model_response() {
    let result = validate_ping_output(&output(
        r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"item.completed","item":{"type":"command_execution","command":"touch file"}}
{"type":"item.completed","item":{"type":"agent_message","text":"Hello"}}
{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#,
        true,
    ));
    assert_eq!(result.unwrap_err().status, PingStatus::ProtocolFailed);
}

#[test]
fn failure_taxonomy_does_not_turn_503_into_quota() {
    assert_eq!(
        classify_failure_text("HTTP 503 upstream unavailable").0,
        PingStatus::UpstreamOverloaded
    );
    assert_eq!(
        classify_failure_text("HTTP 429 rate limit").0,
        PingStatus::RateLimited
    );
    assert_eq!(
        classify_failure_text("usage_limit_reached").0,
        PingStatus::QuotaExhausted
    );
    assert_eq!(
        classify_failure_text("Codex did not complete a structured turn").0,
        PingStatus::ProtocolFailed
    );
    assert_eq!(
        classify_failure_text("failed to start codex child").0,
        PingStatus::SpawnFailed
    );
}

#[test]
fn authoritative_error_is_classified_when_child_emits_no_json() {
    let result = ping_result_from_output(
        &output_with_stderr("", "HTTP 503 upstream unavailable", false),
        None,
        std::time::Instant::now(),
    );
    assert_eq!(result.status, PingStatus::UpstreamOverloaded);
}

#[test]
fn structured_turn_failure_preserves_quota_classification() {
    let result = validate_ping_output(&output(
        r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"turn.failed","error":{"message":"usage_limit_reached"}}"#,
        false,
    ));
    assert_eq!(result.unwrap_err().status, PingStatus::QuotaExhausted);
}

#[test]
fn structured_failure_detail_preserves_authoritative_message() {
    let result = ping_result_from_output(
        &output(
            r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"turn.failed","error":{"message":"usage_limit_reached"}}"#,
            false,
        ),
        None,
        Instant::now(),
    );

    assert_eq!(result.status, PingStatus::QuotaExhausted);
    assert!(
        result.detail.contains("usage_limit_reached"),
        "{}",
        result.detail
    );
}

#[test]
fn fast_nonzero_exit_preserves_bounded_redacted_stderr_detail() {
    let secret = "Bearer ping-secret-token";
    let result = ping_result_from_output(
        &output_with_stderr(
            "",
            &format!("fast child failure {secret}\n{}", "x".repeat(8_000)),
            false,
        ),
        None,
        Instant::now(),
    );

    assert_eq!(result.status, PingStatus::ProcessFailed);
    assert!(
        result.detail.contains("fast child failure"),
        "{}",
        result.detail
    );
    assert!(result.detail.contains("exit code 1"), "{}", result.detail);
    assert!(!result.detail.contains(secret), "{}", result.detail);
    assert!(
        result.detail.len() <= PING_ERROR_DETAIL_MAX_BYTES + 100,
        "{}",
        result.detail.len()
    );
}

#[test]
fn generic_429_is_rate_limited_and_503_is_temporary_even_with_quota_words() {
    assert_eq!(
        classify_failure_text("HTTP 429 Too Many Requests").0,
        PingStatus::RateLimited
    );
    assert_eq!(
        classify_failure_text("HTTP 429 insufficient_quota").0,
        PingStatus::QuotaExhausted
    );
    assert_eq!(
        classify_failure_text("HTTP 429 quota exceeded").0,
        PingStatus::RateLimited
    );
    assert_eq!(
        classify_failure_text("HTTP 503 quota exceeded").0,
        PingStatus::UpstreamOverloaded
    );
}

#[cfg(unix)]
#[test]
fn ping_timeout_cleans_diagnostic_directory_and_child_process() {
    let root = std::env::temp_dir().join(format!(
        "prodex-ping-timeout-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).unwrap();
    let plan = ChildProcessPlan::new(OsString::from("sh"), root.clone())
        .with_args(vec![OsString::from("-c"), OsString::from("sleep 30")]);
    let temp_entries = || {
        std::env::temp_dir()
            .read_dir()
            .unwrap()
            .flatten()
            .filter_map(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .filter(|name| name.starts_with("prodex-ping-"))
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>()
    };
    let before = temp_entries();
    let started = Instant::now();
    let result = run_ping_child(&plan, Duration::from_millis(100));

    assert!(result.unwrap_err().to_string().contains("timed out"));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(temp_entries(), before);
    fs::remove_dir_all(root).unwrap();
}

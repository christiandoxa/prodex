use super::{
    PING_PROMPT, PingOpenaiArgs, PingStatus, classify_failure_text, ping_command_args,
    validate_ping_output,
};
use std::process::Output;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;

fn output(stdout: &str, success: bool) -> Output {
    Output {
        status: if success {
            std::process::ExitStatus::from_raw(0)
        } else {
            std::process::ExitStatus::from_raw(256)
        },
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
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
fn exact_ping_text_uses_the_canonical_run_path() {
    let args = PingOpenaiArgs {
        profile: Some("profile".to_string()),
        model: Some("gpt-5.6-luna".to_string()),
        base_url: Some("https://example.com".to_string()),
        no_proxy: true,
        json: false,
    };
    let command = ping_command_args(&args);
    let values = command
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(values.first().map(String::as_str), Some("run"));
    assert_eq!(values.last().map(String::as_str), Some(PING_PROMPT));
    assert!(
        values
            .windows(2)
            .any(|pair| pair == ["--profile", "profile"])
    );
    assert!(
        values
            .windows(2)
            .any(|pair| pair == ["--model", "gpt-5.6-luna"])
    );
    assert!(
        values
            .windows(2)
            .any(|pair| pair == ["--no-proxy", "--sandbox"])
    );
    assert!(
        values
            .windows(2)
            .any(|pair| pair == ["-c", "model_provider=\"openai\""])
    );
    assert!(values.iter().any(|value| value == "exec"));
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
}

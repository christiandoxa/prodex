use super::{
    ChildProcessPlan, PING_PROMPT, PingStatus, UsageResponse, classify_failure_text,
    ping_openai_child_plan, run_ping_child, usage_main_quota_is_authoritatively_exhausted,
    validate_ping_output,
};
use prodex_quota::{AdditionalRateLimit, UsageWindow, WindowPair};
use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;
use std::process::Output;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn structured_completed_pong_is_required_for_pass() {
    let result = validate_ping_output(&output(
        r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"item.completed","item":{"type":"agent_message","text":"PONG"}}
{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#,
        true,
    ));
    assert!(result.is_ok(), "{result:?}");
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
fn wrong_completed_text_is_not_success() {
    let result = validate_ping_output(&output(
        r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"item.completed","item":{"type":"agent_message","text":"NOT_PONG"}}
{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#,
        true,
    ));
    assert_eq!(result.unwrap_err().status, PingStatus::UnexpectedResponse);
}

#[test]
fn tool_activity_is_rejected_even_with_pong() {
    let result = validate_ping_output(&output(
        r#"{"type":"thread.started","thread_id":"t"}
{"type":"turn.started"}
{"type":"item.completed","item":{"type":"command_execution","command":"touch file"}}
{"type":"item.completed","item":{"type":"agent_message","text":"PONG"}}
{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#,
        true,
    ));
    assert_eq!(result.unwrap_err().status, PingStatus::ProtocolFailed);
}

#[test]
fn child_plan_uses_ephemeral_read_only_json_without_bypass() {
    let plan =
        ping_openai_child_plan("/tmp/codex-home".into(), Some("gpt-5.6-luna"), false).unwrap();
    assert!(plan.args.iter().any(|arg| arg == "--sandbox"));
    assert!(plan.args.iter().any(|arg| arg == "read-only"));
    assert!(plan.args.iter().any(|arg| arg == "--ephemeral"));
    assert!(plan.args.iter().any(|arg| arg == "--ignore-user-config"));
    assert!(plan.args.iter().any(|arg| arg == "--json"));
    assert!(
        plan.args
            .iter()
            .all(|arg| arg != "--dangerously-bypass-approvals-and-sandbox")
    );
    assert!(plan.args.iter().any(|arg| arg == PING_PROMPT));
}

#[test]
fn fake_codex_child_must_emit_the_complete_pong_lifecycle() {
    let root = crate::test_temp_root().join(format!(
        "prodex-ping-fake-codex-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let script = crate::write_test_python_executable(
        &root,
        "fake-codex",
        r#"#!/usr/bin/env python3
import json
print(json.dumps({"type": "thread.started", "thread_id": "diagnostic"}), flush=True)
print(json.dumps({"type": "turn.started"}), flush=True)
print(json.dumps({"type": "item.completed", "item": {"type": "agent_message", "text": "PONG"}}), flush=True)
print(json.dumps({"type": "turn.completed", "usage": {"input_tokens": 1, "output_tokens": 1}}), flush=True)
"#,
    );
    let plan = ChildProcessPlan::new(script.into_os_string(), root.clone())
        .with_args(vec![OsString::from("exec"), OsString::from(PING_PROMPT)]);
    let output = run_ping_child(&plan).unwrap();
    assert!(output.status.success());
    assert!(validate_ping_output(&output).is_ok());
    let expected_files = if cfg!(windows) { 2 } else { 1 };
    assert_eq!(fs::read_dir(&root).unwrap().count(), expected_files);
    fs::remove_dir_all(root).unwrap();
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

#[test]
fn low_remaining_quota_is_not_skipped_before_authoritative_exhaustion() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            allowed: None,
            limit_reached: None,
            extra: std::collections::BTreeMap::new(),
            primary_window: Some(UsageWindow {
                used_percent: Some(99),
                reset_at: None,
                limit_window_seconds: None,
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(99),
                reset_at: None,
                limit_window_seconds: None,
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };
    assert!(!usage_main_quota_is_authoritatively_exhausted(&usage));
}

#[test]
fn authoritative_exhaustion_is_skipped() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            allowed: None,
            limit_reached: None,
            extra: std::collections::BTreeMap::new(),
            primary_window: Some(UsageWindow {
                used_percent: Some(100),
                reset_at: None,
                limit_window_seconds: None,
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(100),
                reset_at: None,
                limit_window_seconds: None,
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };
    assert!(usage_main_quota_is_authoritatively_exhausted(&usage));
}

#[test]
fn one_exhausted_window_does_not_skip_a_profile_with_other_capacity() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            allowed: None,
            limit_reached: None,
            extra: std::collections::BTreeMap::new(),
            primary_window: Some(UsageWindow {
                used_percent: Some(100),
                reset_at: None,
                limit_window_seconds: None,
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: None,
                limit_window_seconds: None,
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };
    assert!(!usage_main_quota_is_authoritatively_exhausted(&usage));
}

#[test]
fn usable_spark_pool_prevents_skipping_a_main_exhausted_profile() {
    let mut usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            allowed: None,
            limit_reached: None,
            extra: std::collections::BTreeMap::new(),
            primary_window: Some(UsageWindow {
                used_percent: Some(100),
                reset_at: None,
                limit_window_seconds: None,
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(100),
                reset_at: None,
                limit_window_seconds: None,
            }),
        }),
        code_review_rate_limit: None,
        rate_limit_reset_credits: None,
        additional_rate_limits: Vec::new(),
    };
    usage.additional_rate_limits.push(AdditionalRateLimit {
        limit_id: None,
        limit_name: Some("GPT-5.3-Codex-Spark".to_string()),
        metered_feature: Some("codex_bengalfox".to_string()),
        rate_limit: WindowPair {
            allowed: None,
            limit_reached: None,
            extra: std::collections::BTreeMap::new(),
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: None,
                limit_window_seconds: None,
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(30),
                reset_at: None,
                limit_window_seconds: None,
            }),
        },
        allowed: None,
        limit_reached: None,
        extra: std::collections::BTreeMap::new(),
    });
    assert!(!usage_main_quota_is_authoritatively_exhausted(&usage));
}

use super::{
    MAX_ACTIVE_RUNS, MAX_QUEUED_RUNS, MAX_RUN_EVENT_TEXT_BYTES, OUTPUT_MAX_BYTES, RunRecord,
    RunState, append_output, build_child_args, events_page, new_run_id, push_event, summary_json,
};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub(super) fn record() -> RunRecord {
    RunRecord {
        state: RunState::Queued,
        created_at: 1,
        started_at: None,
        finished_at: None,
        exit_status: None,
        output: String::new(),
        output_truncated: false,
        events: VecDeque::new(),
        next_seq: 0,
        cancel: Arc::new(AtomicBool::new(false)),
        child: Arc::new(Mutex::new(None)),
        provider: Some("openai".to_string()),
        model: Some("gpt-test".to_string()),
        reasoning_effort: Some("high".to_string()),
    }
}

#[test]
fn child_args_use_normal_super_exec_lifecycle() {
    let prodex_cli::Commands::Super(args) =
        prodex_cli::parse_cli_command_from(["prodex", "s", "--no-presidio"]).unwrap()
    else {
        panic!("expected super args");
    };
    let child = build_child_args(&args).expect("child argument plan should succeed");
    assert_eq!(child.first().and_then(|value| value.to_str()), Some("s"));
    assert!(child.iter().any(|value| value == "--full-access"));
    assert_eq!(
        child
            .iter()
            .rev()
            .take(2)
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["-".to_string(), "exec".to_string()]
    );
}

#[test]
fn native_agy_child_args_do_not_receive_codex_exec_or_generated_config() {
    let prodex_cli::Commands::Super(mut args) = prodex_cli::parse_cli_command_from([
        "prodex",
        "s",
        "gemini",
        "--cli",
        "agy",
        "--no-presidio",
    ])
    .unwrap() else {
        panic!("expected super args");
    };
    args.codex_args = vec![
        OsString::from("-c"),
        OsString::from("model_provider=\"gemini\""),
        OsString::from("--config"),
        OsString::from("model_reasoning_effort=\"max\""),
        OsString::from("--prompt"),
        OsString::from("review"),
    ];
    let child = build_child_args(&args).expect("child argument plan should succeed");
    assert!(child.iter().any(|value| value == "--cli"));
    assert!(child.iter().any(|value| value == "agy"));
    assert!(child.iter().any(|value| value == "--prompt"));
    assert!(child.iter().any(|value| value == "review"));
    assert!(!child.iter().any(|value| value == "exec" || value == "-"));
    assert!(!child.iter().any(|value| {
        let value = value.to_string_lossy();
        value.contains("model_provider") || value.contains("model_reasoning_effort")
    }));
}

#[test]
fn sub_agent_url_and_concurrency_are_forwarded() {
    let prodex_cli::Commands::Super(args) = prodex_cli::parse_cli_command_from([
        "prodex",
        "s",
        "--sub-agent",
        "--sub-agent-provider",
        "openai",
        "--sub-agent-url",
        "http://127.0.0.1:8787/v1",
        "--sub-agent-max-concurrency",
        "3",
        "--no-presidio",
    ])
    .unwrap() else {
        panic!("expected super args");
    };
    let child = build_child_args(&args).expect("child argument plan should succeed");
    let rendered = child
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(
        rendered
            .windows(2)
            .any(|pair| pair == ["--sub-agent-url", "http://127.0.0.1:8787/v1"])
    );
    assert!(
        rendered
            .windows(2)
            .any(|pair| pair == ["--sub-agent-max-concurrency", "3"])
    );
}

#[test]
fn run_ids_match_the_04294_opaque_shape() {
    let one = new_run_id().expect("first run id");
    let two = new_run_id().expect("second run id");
    assert_ne!(one, two);
    assert!(one.starts_with("spr_"));
    assert_eq!(one.len(), 26);
    assert!(
        one.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    );
}

#[test]
fn run_limits_match_04294() {
    assert_eq!(MAX_ACTIVE_RUNS, 4);
    assert_eq!(MAX_QUEUED_RUNS, 16);
    assert_eq!(MAX_RUN_EVENT_TEXT_BYTES, 8 * 1024);
    assert_eq!(OUTPUT_MAX_BYTES, 256 * 1024);
}

#[test]
fn event_retention_and_pagination_match_04294() {
    let mut record = record();
    for index in 0..300 {
        push_event(&mut record, "stdout", &format!("event-{index}"));
    }
    assert_eq!(record.events.len(), 256);
    assert_eq!(record.events.front().map(|event| event.seq), Some(44));
    assert_eq!(record.next_seq, 300);

    let page = events_page(&record, 0, 64).unwrap();
    assert_eq!(page.events.len(), 64);
    assert_eq!(page.events.first().map(|event| event.seq), Some(44));
    assert_eq!(page.events.last().map(|event| event.seq), Some(107));
    assert_eq!(page.next_seq, 300);
    assert!(page.truncated);

    let next = events_page(&record, 107, 64).unwrap();
    assert_eq!(next.events.first().map(|event| event.seq), Some(108));
    assert!(!next.truncated);
}

#[test]
fn stdout_events_continue_after_aggregate_output_is_truncated() {
    let mut record = record();
    record.output = "x".repeat(OUTPUT_MAX_BYTES);
    append_output(&mut record, "stdout", b"later-output");
    assert_eq!(record.output.len(), OUTPUT_MAX_BYTES);
    assert!(record.output_truncated);
    assert!(
        record
            .events
            .iter()
            .any(|event| event.event_type == "stdout" && event.text == "later-output")
    );
    assert_eq!(
        record
            .events
            .iter()
            .filter(|event| event.event_type == "output_truncated")
            .count(),
        1
    );
}

#[test]
fn summary_keeps_provider_model_and_effort_metadata() {
    let record = record();
    let summary = summary_json("spr_fixture", &record);
    assert_eq!(summary["provider"], "openai");
    assert_eq!(summary["model"], "gpt-test");
    assert_eq!(summary["reasoning_effort"], "high");
    assert_eq!(summary["state"], "queued");
}

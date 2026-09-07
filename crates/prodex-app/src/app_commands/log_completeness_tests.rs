use super::log_stream::collect_runtime_log_line;
use super::{
    FollowedLog, LogStreamItem, TranscriptEvent, collect_new_transcript_events,
    local_log_timestamp, transcript_events_from_session_line,
};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

#[test]
fn parses_status_exit_and_exposed_output_events() {
    let status = r#"{"timestamp":"2026-07-01T13:10:33.292Z","type":"event_msg","payload":{"type":"command_execution_completed","status":"failed","exit_code":7,"stderr":"command failed"}}"#;

    assert_eq!(
        transcript_events_from_session_line(status),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-07-01T13:10:33.292Z"),
            source: "error".to_string(),
            text: "status=failed exit_code=7 stderr:\ncommand failed".to_string(),
        }]
    );
}

#[test]
fn transcript_output_is_redacted_and_bounded() {
    let assistant = format!(
        r#"{{"timestamp":"2026-07-01T13:10:32.292Z","type":"event_msg","payload":{{"type":"agent_message","message":"Authorization: Bearer fixture-token {}"}}}}"#,
        "x".repeat(70 * 1024)
    );

    let events = transcript_events_from_session_line(&assistant);
    let [event] = events.as_slice() else {
        panic!("assistant event should be retained");
    };
    assert!(!event.text.contains("fixture-token"));
    assert!(event.text.contains("<redacted>"));
    assert!(event.text.contains("[truncated]"));
    assert!(event.text.len() <= 64 * 1024);
}

#[test]
fn unknown_runtime_events_remain_visible_without_raw_payload_dumping() {
    let items = collect_runtime_log_line(
        std::path::Path::new("/home/test-user/runtime-unknown-event.log"),
        "[2026-07-01 13:10:32.292 +00:00] runtime_proxy_queue_recovered request=7 profile=main reason=capacity_restored",
        true,
        None,
        false,
    )
    .unwrap();

    let [LogStreamItem::Transcript(event)] = items.as_slice() else {
        panic!("unknown runtime event should remain one transcript item");
    };
    assert_eq!(event.source, "event");
    assert!(event.text.contains("runtime proxy queue recovered"));
    assert!(event.text.contains("profile=main"));
}

#[test]
fn structured_runtime_fields_decode_scalar_values_once() {
    let line = serde_json::json!({
        "timestamp": "2026-07-01 13:10:32.292 +00:00",
        "event": "terminal_event",
        "fields": {
            "request": 7,
            "profile": "main",
            "status": 200,
        },
    })
    .to_string();
    let items = collect_runtime_log_line(
        std::path::Path::new("/home/test-user/runtime-structured.log"),
        &line,
        true,
        None,
        false,
    )
    .unwrap();

    assert_eq!(items.len(), 1);
    let json = super::log_stream::log_stream_item_json(&items[0]).unwrap();
    assert_eq!(json.matches("terminal event").count(), 1);
    assert!(json.contains("r0007"));
    assert!(json.contains("profile=main"));
    assert!(json.contains("status=200"));
}

#[test]
fn keeps_repeated_transcript_text_when_timestamps_differ() {
    let first = r#"{"timestamp":"2026-07-01T13:08:43.923Z","type":"event_msg","payload":{"type":"user_message","message":"same text"}}"#;
    let second = r#"{"timestamp":"2026-07-01T13:08:44.923Z","type":"event_msg","payload":{"type":"user_message","message":"same text"}}"#;
    let root = env::temp_dir().join(format!(
        "prodex-transcript-repeat-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("session.jsonl");
    fs::write(&path, format!("{first}\n{second}\n")).unwrap();

    let events = collect_new_transcript_events(&path, &mut FollowedLog::default()).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0].timestamp,
        local_log_timestamp("2026-07-01T13:08:43.923Z")
    );
    assert_eq!(
        events[1].timestamp,
        local_log_timestamp("2026-07-01T13:08:44.923Z")
    );
    fs::remove_dir_all(root).unwrap();
}

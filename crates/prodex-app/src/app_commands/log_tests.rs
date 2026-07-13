use super::log_stream::read_new_token_usage_events;
use super::*;

#[test]
fn follows_only_complete_token_usage_lines() {
    let root = env::temp_dir().join(format!(
        "prodex-log-follow-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("runtime.log");
    fs::write(
        &path,
        "[2026-06-19 20:00:00.000 +07:00] token_usage request=7 transport=http profile=main source=responses input_tokens=12 cached_input_tokens=3 output_tokens=4 reasoning_tokens=1",
    )
    .unwrap();
    let mut state = FollowedLog::default();
    read_new_token_usage_events(&path, &mut state, true).unwrap();
    assert!(!state.pending.is_empty());
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"\n")
        .unwrap();
    read_new_token_usage_events(&path, &mut state, true).unwrap();
    assert!(state.pending.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn parses_session_transcript_text_events() {
    let meta = r#"{"timestamp":"2026-06-20T01:00:00Z","type":"session_meta","payload":{"base_instructions":{"text":"System prompt."},"model_provider":"openai","source":"cli","originator":"codex-tui","cwd":"/repo"}}"#;
    let user = r#"{"timestamp":"2026-06-20T01:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Hello model."}]}}"#;
    let assistant = r#"{"timestamp":"2026-06-20T01:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hello user."}]}}"#;
    let tool = r#"{"timestamp":"2026-06-20T01:00:03Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"pwd\"}"}}"#;

    assert_eq!(
        transcript_events_from_session_line(meta),
        vec![
            TranscriptEvent {
                timestamp: local_log_timestamp("2026-06-20T01:00:00Z"),
                source: "prompt-engineering".to_string(),
                text: "System prompt.".to_string(),
            },
            TranscriptEvent {
                timestamp: local_log_timestamp("2026-06-20T01:00:00Z"),
                source: "session-context".to_string(),
                text: "provider=openai source=cli originator=codex-tui cwd=/repo".to_string(),
            }
        ]
    );
    assert_eq!(
        transcript_events_from_session_line(user),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-06-20T01:00:01Z"),
            source: "user".to_string(),
            text: "Hello model.".to_string(),
        }]
    );
    assert_eq!(
        transcript_events_from_session_line(assistant),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-06-20T01:00:02Z"),
            source: "assistant".to_string(),
            text: "Hello user.".to_string(),
        }]
    );
    assert_eq!(
        transcript_events_from_session_line(tool),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-06-20T01:00:03Z"),
            source: "tool-call:exec_command".to_string(),
            text: "{\"cmd\":\"pwd\"}".to_string(),
        }]
    );
}

#[test]
fn parses_turn_context_reasoning_and_custom_tool_events() {
    let turn_context = r#"{"timestamp":"2026-01-10T11:52:46.163Z","type":"turn_context","payload":{"cwd":"/repo","approval_policy":"on-request","model":"gpt-5.2-codex","effort":"medium","summary":"auto"}}"#;
    let reasoning = r#"{"timestamp":"2026-01-10T11:53:34.029Z","type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"**Planning server bill synchronization**"}]}}"#;
    let event_reasoning = r#"{"timestamp":"2026-01-10T11:53:34.029Z","type":"event_msg","payload":{"type":"agent_reasoning","text":"**Planning server bill synchronization**"}}"#;
    let custom_tool = r#"{"timestamp":"2026-01-10T11:53:51.257Z","type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch"}}"#;
    let custom_tool_output = r#"{"timestamp":"2026-01-10T11:53:51.287Z","type":"response_item","payload":{"type":"custom_tool_call_output","output":"{\"output\":\"Success\"}"}}"#;

    assert_eq!(
        transcript_events_from_session_line(turn_context),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-01-10T11:52:46.163Z"),
            source: "turn-context".to_string(),
            text: "model=gpt-5.2-codex effort=medium summary=auto approval=on-request cwd=/repo"
                .to_string(),
        }]
    );
    assert_eq!(
        transcript_events_from_session_line(reasoning),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-01-10T11:53:34.029Z"),
            source: "reasoning".to_string(),
            text: "**Planning server bill synchronization**".to_string(),
        }]
    );
    assert_eq!(
        transcript_events_from_session_line(event_reasoning),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-01-10T11:53:34.029Z"),
            source: "reasoning".to_string(),
            text: "**Planning server bill synchronization**".to_string(),
        }]
    );
    assert_eq!(
        transcript_events_from_session_line(custom_tool),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-01-10T11:53:51.257Z"),
            source: "tool-call:apply_patch".to_string(),
            text: "*** Begin Patch".to_string(),
        }]
    );
    assert_eq!(
        transcript_events_from_session_line(custom_tool_output),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-01-10T11:53:51.287Z"),
            source: "tool-output".to_string(),
            text: "{\"output\":\"Success\"}".to_string(),
        }]
    );
}

#[test]
fn skips_internal_overlay_attachment_paths_from_message_content() {
    let user = r#"{"timestamp":"2026-07-03T09:26:44.748Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"image file: /home/test-user/.prodex/profiles/.prodex-overlay-1234-1700000000000-0/attachments/00000000-0000-4000-8000-000000000001/image-1.png"}]}}"#;

    assert!(
        transcript_events_from_session_line(user).is_empty(),
        "internal overlay attachment paths should be hidden from prodex log stream"
    );
}

#[test]
fn keeps_readable_message_lines_while_dropping_internal_attachment_paths() {
    let user = r#"{"timestamp":"2026-07-03T09:26:44.748Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"please compare these screenshots\nimage file: /home/test-user/.prodex/profiles/.prodex-overlay-1234-1700000000000-0/attachments/00000000-0000-4000-8000-000000000001/image-1.png\nthen summarize the bug"}]}}"#;

    assert_eq!(
        transcript_events_from_session_line(user),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-07-03T09:26:44.748Z"),
            source: "user".to_string(),
            text: "please compare these screenshots\nthen summarize the bug".to_string(),
        }]
    );
}

#[test]
fn skips_binary_like_tool_output_from_log_stream() {
    let noisy_tool_output = "{\"timestamp\":\"2026-07-03T02:26:44.748Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call_output\",\"output\":\"\\uFFFD\\uFFFD\\uFFFD\\u0000\\u0001\\uFFFD\\uFFFD\\uFFFDabc\"}}";

    assert!(
        transcript_events_from_session_line(noisy_tool_output).is_empty(),
        "binary-like tool output should be hidden from prodex log stream"
    );
}

#[test]
fn keeps_readable_lines_when_tool_output_starts_with_binary_garbage() {
    let mixed_tool_output = "{\"timestamp\":\"2026-07-03T02:26:44.748Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"output\":\"\\uFFFD\\uFFFD\\uFFFD\\u0000junk\\nChunk ID: d103c4\\nWall time: 1.0007 seconds\\nOutput:\\nForwarding from 127.0.0.1:19080 -> 8080\\r\\nForwarding from [::1]:19080 -> 8080\\r\\n\"}}";

    assert_eq!(
        transcript_events_from_session_line(mixed_tool_output),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-07-03T02:26:44.748Z"),
            source: "tool-output".to_string(),
            text: "Chunk ID: d103c4\nWall time: 1.0007 seconds\nOutput:\nForwarding from 127.0.0.1:19080 -> 8080\nForwarding from [::1]:19080 -> 8080".to_string(),
        }]
    );
}

#[test]
fn drops_multiline_binary_garbage_prefix_from_tool_output() {
    let mixed_tool_output = "{\"timestamp\":\"2026-07-03T02:26:44.748Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"output\":\"\\uFFFDabcz\\nxy\\uFFFD12\\n\\uFFFDqwe9\\nChunk ID: d103c4\\nWall time: 1.0007 seconds\\nOutput:\\nForwarding from 127.0.0.1:19080 -> 8080\\r\\nForwarding from [::1]:19080 -> 8080\\r\\n\"}}";

    assert_eq!(
        transcript_events_from_session_line(mixed_tool_output),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-07-03T02:26:44.748Z"),
            source: "tool-output".to_string(),
            text: "Chunk ID: d103c4\nWall time: 1.0007 seconds\nOutput:\nForwarding from 127.0.0.1:19080 -> 8080\nForwarding from [::1]:19080 -> 8080".to_string(),
        }]
    );
}

#[test]
fn drops_mojibake_like_tool_output_lines_from_log_stream() {
    let mixed_tool_output = "{\"timestamp\":\"2026-07-03T02:26:44.748Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"output\":\"Â�5◊c��I1���n@U◊U���hFz����|p�◊I8◊#◊EJ◊<◊B4����|p�\\nChunk ID: d103c4\\nWall time: 1.0007 seconds\\nOutput:\\nForwarding from 127.0.0.1:19080 -> 8080\\r\\nForwarding from [::1]:19080 -> 8080\\r\\n\"}}";

    assert_eq!(
        transcript_events_from_session_line(mixed_tool_output),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-07-03T02:26:44.748Z"),
            source: "tool-output".to_string(),
            text: "Chunk ID: d103c4\nWall time: 1.0007 seconds\nOutput:\nForwarding from 127.0.0.1:19080 -> 8080\nForwarding from [::1]:19080 -> 8080".to_string(),
        }]
    );
}

#[test]
fn strips_ansi_from_tool_output_before_rendering() {
    let ansi_tool_output = "{\"timestamp\":\"2026-07-03T02:26:44.748Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"output\":\"\\u001b[35mForwarding\\u001b[0m from 127.0.0.1:19080 -> 8080\\r\\n\"}}";

    assert_eq!(
        transcript_events_from_session_line(ansi_tool_output),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-07-03T02:26:44.748Z"),
            source: "tool-output".to_string(),
            text: "Forwarding from 127.0.0.1:19080 -> 8080".to_string(),
        }]
    );
}

#[test]
fn preserves_real_provider_and_effort_values() {
    for provider in [
        "openai",
        "prodex-gemini",
        "prodex-deepseek",
        "prodex-local",
        "prodex-test",
        "mock",
    ] {
        let session_meta = format!(
            "{{\"timestamp\":\"2026-07-01T13:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"model_provider\":\"{provider}\",\"source\":\"cli\",\"cwd\":\"/repo\"}}}}"
        );
        assert_eq!(
            transcript_events_from_session_line(&session_meta),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-07-01T13:00:00Z"),
                source: "session-context".to_string(),
                text: format!("provider={provider} source=cli cwd=/repo"),
            }]
        );
    }

    for effort in ["medium", "high", "xhigh"] {
        for model in [
            "auto",
            "deepseek-v4-pro",
            "gemini-2.5-flash",
            "gemini-2.5-pro",
            "gemini-3-pro-preview",
            "gemini-3.1-pro-preview",
            "gpt-5.2-codex",
            "gpt-5.3-codex",
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.5",
            "mock-model",
            "pro",
            "unsloth/qwen3.5-35b-a3b",
        ] {
            let turn_context = format!(
                "{{\"timestamp\":\"2026-07-01T13:00:01Z\",\"type\":\"turn_context\",\"payload\":{{\"model\":\"{model}\",\"effort\":\"{effort}\",\"summary\":\"auto\",\"approval_policy\":\"never\",\"cwd\":\"/repo\"}}}}"
            );
            assert_eq!(
                transcript_events_from_session_line(&turn_context),
                vec![TranscriptEvent {
                    timestamp: local_log_timestamp("2026-07-01T13:00:01Z"),
                    source: "turn-context".to_string(),
                    text: format!(
                        "model={model} effort={effort} summary=auto approval=never cwd=/repo"
                    ),
                }]
            );
        }
    }
}

#[test]
fn log_snapshot_items_preserve_transcript_and_usage_order() {
    let transcript = TranscriptEvent {
        timestamp: "2026-06-20 08:00:01".to_string(),
        source: "assistant".to_string(),
        text: "Hello user.".to_string(),
    };
    let usage = InfoTokenUsageEvent {
        timestamp: "2026-06-20 08:00:02".to_string(),
        request: Some(7),
        transport: "http".to_string(),
        profile: "main".to_string(),
        source: "responses".to_string(),
        input_tokens: 12,
        cached_input_tokens: 3,
        output_tokens: 4,
        reasoning_tokens: 1,
    };

    let items = log_snapshot_items(Some(&transcript), None, Some(&usage));

    assert_eq!(items.len(), 2);
    assert!(matches!(items[0], LogStreamItem::Transcript(_)));
    assert!(matches!(items[1], LogStreamItem::TokenUsage(_)));
}

#[test]
fn log_snapshot_tui_text_handles_empty_state() {
    let items = VecDeque::new();
    let text = log_stream_tui_text(&items, 10, 80);
    let rendered = text
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Waiting for transcript, upstream payload, or token usage"));
}

#[test]
fn parses_event_msg_transcript_text_events() {
    let user = r#"{"timestamp":"2026-07-01T13:40:05.000Z","type":"event_msg","payload":{"type":"user_message","message":"hello from user"}}"#;
    let assistant = r#"{"timestamp":"2026-07-01T13:10:32.292Z","type":"event_msg","payload":{"type":"agent_message","message":"hello from assistant","phase":"commentary"}}"#;

    assert_eq!(
        transcript_events_from_session_line(user),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-07-01T13:40:05.000Z"),
            source: "user".to_string(),
            text: "hello from user".to_string(),
        }]
    );
    assert_eq!(
        transcript_events_from_session_line(assistant),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-07-01T13:10:32.292Z"),
            source: "assistant".to_string(),
            text: "hello from assistant".to_string(),
        }]
    );
}

#[test]
fn collects_websocket_payload_and_usage_from_runtime_log() {
    let root = env::temp_dir().join(format!(
        "prodex-runtime-payload-follow-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("runtime.log");
    fs::write(
        &path,
        concat!(
            "[2026-07-01 21:52:36.700 +07:00] upstream_payload request=28 transport=websocket route=websocket profile=main bytes=35 logged_bytes=35 truncated=false payload_b64=eyJpbnB1dCI6ImhlbGxvIn0=\n",
            "[2026-07-01 21:52:36.729 +07:00] token_usage request=28 route=websocket transport=websocket profile=main source=responses_websocket prompt_cache_key=present prompt_cache_key_hash=sc:abc prompt_cache_owner=no_cached_tokens input_tokens=9741 uncached_input_tokens=9741 cached_input_tokens=0 output_tokens=0 reasoning_tokens=0\n"
        ),
    )
    .unwrap();

    let items = collect_new_runtime_log_stream_items(&path, &mut FollowedLog::default()).unwrap();

    assert_eq!(items.len(), 2);
    assert!(matches!(items[0], LogStreamItem::UpstreamPayload(_)));
    assert!(matches!(items[1], LogStreamItem::TokenUsage(_)));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dedupes_consecutive_equivalent_transcript_events() {
    let root = env::temp_dir().join(format!(
        "prodex-transcript-dedupe-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("session.jsonl");
    fs::write(
        &path,
        concat!(
            "{\"timestamp\":\"2026-07-01T13:08:43.923Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"same text\"}]}}\n",
            "{\"timestamp\":\"2026-07-01T13:08:43.923Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"same text\"}}\n"
        ),
    )
    .unwrap();

    let events = collect_new_transcript_events(&path, &mut FollowedLog::default()).unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].source, "user");
    assert_eq!(events[0].text, "same text");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn follows_only_complete_transcript_lines() {
    let root = env::temp_dir().join(format!(
        "prodex-transcript-follow-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("session.jsonl");
    fs::write(
        &path,
        r#"{"timestamp":"2026-06-20T01:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Hello"}]}}"#,
    )
    .unwrap();
    let mut state = FollowedLog::default();
    read_new_transcript_events(&path, &mut state).unwrap();
    assert!(!state.pending.is_empty());
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"\n")
        .unwrap();
    read_new_transcript_events(&path, &mut state).unwrap();
    assert!(state.pending.is_empty());
    fs::remove_dir_all(root).unwrap();
}

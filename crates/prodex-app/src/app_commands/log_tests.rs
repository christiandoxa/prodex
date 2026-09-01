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
    drop(state);
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
fn labels_mcp_and_sub_agent_events_without_logging_arguments() {
    let mcp = r#"{"timestamp":"2026-07-01T13:00:00Z","type":"response_item","payload":{"type":"mcp_call","server_label":"workspace","tool_name":"search","arguments":{"query":"secret"},"status":"completed"}}"#;
    let sub_agent = r#"{"timestamp":"2026-07-01T13:00:01Z","type":"event_msg","payload":{"type":"subagent_started","name":"reviewer","message":"started"}}"#;

    assert_eq!(
        transcript_events_from_session_line(mcp),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-07-01T13:00:00Z"),
            source: "mcp".to_string(),
            text: "server=workspace tool=search status=completed".to_string(),
        }]
    );
    assert_eq!(
        transcript_events_from_session_line(sub_agent),
        vec![TranscriptEvent {
            timestamp: local_log_timestamp("2026-07-01T13:00:01Z"),
            source: "agent".to_string(),
            text: "name=reviewer status=started".to_string(),
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
fn log_snapshot_json_preserves_complete_event_order() {
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
        generation_ms: None,
        output_tokens_per_second: None,
    };
    let upstream = crate::app_commands::log_upstream_payload::UpstreamPayloadEvent {
        timestamp: "2026-06-20 08:00:01".to_string(),
        request: Some(7),
        transport: "http".to_string(),
        route: "responses".to_string(),
        profile: "main".to_string(),
        bytes: 17,
        logged_bytes: 17,
        truncated: false,
        payload: r#"{"input":"hello"}"#.to_string(),
    };

    let items = log_snapshot_items(Some(&transcript), Some(&upstream), Some(&usage));

    assert_eq!(items.len(), 3);
    assert!(matches!(items[0], LogStreamItem::Transcript(_)));
    assert!(matches!(items[1], LogStreamItem::UpstreamPayload(_)));
    assert!(matches!(items[2], LogStreamItem::TokenUsage(_)));

    let json = items
        .iter()
        .map(log_stream_item_json)
        .collect::<Result<Vec<_>>>()
        .unwrap();
    assert_eq!(json.len(), 3);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json[0]).unwrap()["text"],
        "Hello user."
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json[1]).unwrap()["payload"],
        r#"{"input":"hello"}"#
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json[2]).unwrap()["profile"],
        "main"
    );
    assert!(log_snapshot_items(None, None, None).is_empty());
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
            "[2026-07-01 21:52:36.729 +07:00] token_usage request=28 route=websocket transport=websocket profile=main source=responses_websocket prompt_cache_key=present prompt_cache_key_hash=sc:abc prompt_cache_owner=no_cached_tokens input_tokens=9741 uncached_input_tokens=9741 cached_input_tokens=0 output_tokens=100 reasoning_tokens=0 generation_ms=1000 output_tokens_per_second=100.0\n"
        ),
    )
    .unwrap();

    let items =
        collect_new_runtime_log_stream_items(&path, &mut FollowedLog::default(), false).unwrap();

    assert_eq!(items.len(), 2);
    assert!(matches!(items[0], LogStreamItem::UpstreamPayload(_)));
    assert!(matches!(items[1], LogStreamItem::TokenUsage(_)));
    let LogStreamItem::TokenUsage(event) = &items[1] else {
        panic!("expected token usage event");
    };
    assert_eq!(event.generation_ms, Some(1000));
    assert_eq!(event.output_tokens_per_second, Some(100.0));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn runtime_token_sample_reaches_header_state_through_shared_collector() {
    let path = std::path::Path::new("/tmp/runtime-throughput.log");
    let mut throughput = crate::app_commands::log_throughput::OutputThroughput::default();
    let items = super::log_stream::collect_runtime_log_line(
        path,
        "[2026-07-01 21:52:36.729 +07:00] token_usage request=28 route=responses transport=http profile=main source=responses_sse input_tokens=10 cached_input_tokens=0 output_tokens=100 reasoning_tokens=0 generation_ms=1000 output_tokens_per_second=100.0",
        false,
        Some(&mut throughput),
        true,
    )
    .unwrap();

    assert!(matches!(items.as_slice(), [LogStreamItem::TokenUsage(_)]));
    assert_eq!(
        throughput.display_rate_for_profile(std::time::Instant::now(), None),
        Some(100.0)
    );
}

#[test]
fn untimed_terminal_token_usage_closes_live_throughput_stream() {
    let path = std::path::Path::new("/tmp/runtime-untimed-final.log");
    let start = std::time::Instant::now();
    let mut throughput = crate::app_commands::log_throughput::OutputThroughput::default();
    for (output_tokens, observed_at) in [
        (100, start),
        (200, start + std::time::Duration::from_secs(1)),
    ] {
        throughput.observe_token_usage(
            path,
            &InfoTokenUsageEvent {
                profile: "main".to_string(),
                request: Some(29),
                output_tokens,
                ..InfoTokenUsageEvent::default()
            },
            observed_at,
        );
    }
    assert_eq!(
        throughput.active_rate_for_profile(start + std::time::Duration::from_secs(1), Some("main")),
        Some(100.0)
    );

    let items = super::log_stream::collect_runtime_log_line(
        path,
        "[2026-07-01 21:52:37.729 +07:00] token_usage request=29 route=responses transport=http profile=main source=responses_unary input_tokens=10 output_tokens=200 reasoning_tokens=0",
        false,
        Some(&mut throughput),
        true,
    )
    .unwrap();

    assert!(matches!(items.as_slice(), [LogStreamItem::TokenUsage(_)]));
    assert!(
        throughput
            .active_rate_for_profile(std::time::Instant::now(), Some("main"))
            .is_none()
    );
    assert_eq!(
        throughput.display_rate_for_profile(std::time::Instant::now(), Some("main")),
        Some(100.0)
    );
}

#[test]
fn live_progress_selects_current_profile_over_historical_rate() {
    let history = std::path::Path::new("/tmp/runtime-history-profile-a.log");
    let live = std::path::Path::new("broker:runtime-profile-b:instance");
    let start = std::time::Instant::now();
    let mut throughput = crate::app_commands::log_throughput::OutputThroughput::default();
    throughput.observe_historical(
        history,
        &InfoTokenUsageEvent {
            profile: "profile-a".to_string(),
            request: Some(1),
            output_tokens: 200,
            generation_ms: Some(2_500),
            output_tokens_per_second: Some(80.0),
            ..InfoTokenUsageEvent::default()
        },
    );
    for (output_tokens, observed_at) in [
        (100, start),
        (200, start + std::time::Duration::from_secs(1)),
    ] {
        throughput.observe_token_usage(
            live,
            &InfoTokenUsageEvent {
                profile: "profile-b".to_string(),
                request: Some(2),
                output_tokens,
                ..InfoTokenUsageEvent::default()
            },
            observed_at,
        );
    }

    let current_profile = throughput.active_profile();
    assert_eq!(current_profile.as_deref(), Some("profile-b"));
    assert_eq!(
        throughput.display_rate_for_profile(
            start + std::time::Duration::from_secs(1),
            current_profile.as_deref(),
        ),
        Some(100.0)
    );
}

#[test]
fn collects_websocket_stream_payload_as_plain_tool_call_text() {
    let root = env::temp_dir().join(format!(
        "prodex-runtime-stream-payload-follow-{}-{}",
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
        r#"[2026-07-01 21:52:36.710 +07:00] stream_payload request=28 route=websocket transport=websocket profile=main source=tool-call:exec stream="const r = await tools.web__run({}); text(r);"
"#,
    )
    .unwrap();

    let items =
        collect_new_runtime_log_stream_items(&path, &mut FollowedLog::default(), false).unwrap();

    assert_eq!(items.len(), 1);
    let LogStreamItem::Transcript(event) = &items[0] else {
        panic!("stream payload should render as transcript");
    };
    assert_eq!(event.source, "tool-call:exec");
    assert_eq!(event.text, "const r = await tools.web__run({}); text(r);");
    let rendered = log_stream_tui_text(&VecDeque::from(items), 10, 100)
        .lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .map(|span| span.content.as_ref())
        .collect::<String>();
    assert!(rendered.contains("TOOL CALL"));
    assert!(rendered.contains("tools.web__run"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn human_stream_adds_correlated_operational_events_without_exposing_urls() {
    let root = env::temp_dir().join(format!(
        "prodex-log-operational-events-{}-{}",
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
            "[2026-07-01 21:52:36.700 +07:00] selection_pick request=42 route=responses profile=main mode=ready quota_band=quota_healthy\n",
            "[2026-07-01 21:52:36.701 +07:00] upstream_start request=42 transport=http profile=main method=POST url=https://example.test/v1/responses?token=URL_SECRET\n",
            "[2026-07-01 21:52:36.702 +07:00] upstream_response request=42 transport=http profile=main status=200 elapsed_ms=12\n",
        ),
    )
    .unwrap();

    let items =
        collect_new_runtime_log_stream_items(&path, &mut FollowedLog::default(), true).unwrap();
    let rendered = log_stream_tui_text(&VecDeque::from(items), 20, 120)
        .lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .map(|span| span.content.as_ref())
        .collect::<String>();

    assert!(rendered.contains("ROUTE"));
    assert!(rendered.contains("UPSTREAM"));
    assert!(rendered.contains("r002a"));
    assert!(rendered.contains("/v1/responses"));
    assert!(!rendered.contains("URL_SECRET"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn json_stream_keeps_repeated_load_observations_raw() {
    let root = env::temp_dir().join(format!(
        "prodex-log-load-json-{}-{}",
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
            "[2026-07-01 21:52:36.700 +07:00] profile_inflight_saturated request=1 route=responses profile=main active=8 hard_limit=8\n",
            "[2026-07-01 21:52:36.701 +07:00] profile_inflight_saturated request=2 route=responses profile=main active=8 hard_limit=8\n"
        ),
    )
    .unwrap();

    let items =
        collect_new_runtime_log_stream_items(&path, &mut FollowedLog::default(), true).unwrap();
    assert_eq!(items.len(), 2);
    assert!(
        items
            .iter()
            .all(|item| matches!(item, LogStreamItem::Transcript(_)))
    );
    let json = items
        .iter()
        .map(log_stream_item_json)
        .collect::<Result<Vec<_>>>()
        .unwrap();
    assert_eq!(json.len(), 2);
    assert!(json.iter().any(|line| line.contains("r0001")));
    assert!(json.iter().any(|line| line.contains("r0002")));
    let tui_items = collect_new_runtime_log_stream_items_for_tui_with_throughput(
        &path,
        &mut FollowedLog::default(),
        true,
        None,
    )
    .unwrap();
    assert!(
        tui_items
            .iter()
            .all(|item| matches!(item, LogStreamItem::LoadObservation(_)))
    );
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
    drop(state);
    fs::remove_dir_all(root).unwrap();
}

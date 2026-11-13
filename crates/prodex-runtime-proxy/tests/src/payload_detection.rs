use super::*;

#[path = "payload_detection_mojo_sse_payload.rs"]
mod mojo_sse_payload_tests;
use crate::{
    RuntimePreviousResponseFreshFallbackShape, RuntimeProxyRequest,
    runtime_request_previous_response_fresh_fallback_shape, runtime_request_session_id,
    runtime_request_value_previous_response_fresh_fallback_shape,
    runtime_request_value_requires_previous_response_affinity,
};

#[derive(Debug, Clone)]
struct GeneratedRuntimeSseEvent {
    response_id: String,
    turn_state: Option<String>,
    shape: u8,
}

impl GeneratedRuntimeSseEvent {
    fn event_type(&self) -> &'static str {
        match self.shape % 3 {
            0 => "response.created",
            1 => "response.in_progress",
            _ => "response.completed",
        }
    }
}

fn generated_runtime_sse_event(
    response_id: &str,
    turn_state: Option<&str>,
    shape: u8,
) -> GeneratedRuntimeSseEvent {
    GeneratedRuntimeSseEvent {
        response_id: response_id.to_string(),
        turn_state: turn_state.map(str::to_string),
        shape,
    }
}

fn collect_runtime_sse_events(chunks: &[&[u8]]) -> Vec<RuntimeParsedSseEvent> {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut events = Vec::new();

    for chunk in chunks {
        runtime_sse_consume_chunk(&mut line, &mut data_lines, chunk, |event| {
            events.push(event)
        });
    }
    runtime_sse_finish_pending(&mut line, &mut data_lines, |event| events.push(event));

    events
}

#[test]
fn compiled_mojo_sse_scanning_preserves_events_across_more_than_256_lines() {
    let mut body = ": keep-alive\r\n".repeat(600);
    body.push_str(
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-bulk\"}}\r\n\r\n",
    );
    let whole = collect_runtime_sse_events(&[body.as_bytes()]);
    assert_eq!(whole.len(), 1, "{whole:?}");
    assert_eq!(whole[0].event_type.as_deref(), Some("response.created"));
    assert_eq!(whole[0].response_ids, ["resp-bulk"]);
    for widths in [vec![0], vec![1; 1_024], (0..701).collect::<Vec<_>>()] {
        let split = collect_runtime_sse_events_for_chunk_sizes(body.as_bytes(), &widths);
        assert_eq!(
            runtime_sse_event_signatures(&split),
            runtime_sse_event_signatures(&whole),
            "width count {}",
            widths.len(),
        );
    }
}

fn collect_runtime_sse_events_for_chunk_sizes(
    body: &[u8],
    chunk_sizes: &[usize],
) -> Vec<RuntimeParsedSseEvent> {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut events = Vec::new();
    let mut offset = 0usize;

    for size in chunk_sizes {
        if offset >= body.len() {
            break;
        }
        let chunk_len = (1 + (size % 31)).min(body.len() - offset);
        runtime_sse_consume_chunk(
            &mut line,
            &mut data_lines,
            &body[offset..offset + chunk_len],
            |event| events.push(event),
        );
        offset += chunk_len;
    }

    if offset < body.len() {
        runtime_sse_consume_chunk(&mut line, &mut data_lines, &body[offset..], |event| {
            events.push(event)
        });
    }
    runtime_sse_finish_pending(&mut line, &mut data_lines, |event| events.push(event));

    events
}

type RuntimeSseEventSignature = (bool, bool, Vec<String>, Option<String>, Option<String>);

fn runtime_sse_event_signatures(events: &[RuntimeParsedSseEvent]) -> Vec<RuntimeSseEventSignature> {
    events
        .iter()
        .map(|event| {
            (
                event.quota_blocked,
                event.previous_response_not_found,
                event.response_ids.clone(),
                event.event_type.clone(),
                event.turn_state.clone(),
            )
        })
        .collect()
}

fn build_runtime_sse_body(events: &[GeneratedRuntimeSseEvent], crlf: bool) -> Vec<u8> {
    let line_end = if crlf { "\r\n" } else { "\n" };
    let mut body = String::new();

    for event in events {
        body.push_str(": keep-alive");
        body.push_str(line_end);
        body.push_str("event: ");
        body.push_str(event.event_type());
        body.push_str(line_end);

        let payload = match event.shape % 3 {
            0 => serde_json::json!({
                "type": event.event_type(),
                "response_id": event.response_id,
                "turn_state": event.turn_state,
            }),
            1 => serde_json::json!({
                "type": event.event_type(),
                "response": {
                    "id": event.response_id,
                    "headers": {
                        "x-codex-turn-state": event.turn_state,
                    },
                },
            }),
            _ => serde_json::json!({
                "type": event.event_type(),
                "response": {
                    "id": event.response_id,
                    "turnState": event.turn_state,
                },
            }),
        };

        body.push_str("data: ");
        body.push_str(&payload.to_string());
        body.push_str(line_end);
        body.push_str(line_end);
    }

    body.into_bytes()
}

#[test]
fn runtime_sse_helpers_chunking_corpus_matches_single_pass() {
    let event_sets = [
        vec![generated_runtime_sse_event("resp-a", None, 0)],
        vec![
            generated_runtime_sse_event("resp-a", Some("turn-a"), 1),
            generated_runtime_sse_event("resp-b", None, 2),
        ],
        vec![
            generated_runtime_sse_event("resp-a", Some("turn-a"), 0),
            generated_runtime_sse_event("resp-b", Some("turn-b"), 1),
            generated_runtime_sse_event("resp-c", None, 2),
            generated_runtime_sse_event("resp-d", Some("turn-d"), 5),
        ],
    ];
    let chunk_size_sets: &[&[usize]] = &[
        &[],
        &[0],
        &[1, 1, 1, 1, 1, 1, 1, 1],
        &[2, 3, 5, 8, 13, 21, 34, 55],
        &[31, 0, 7, 128, 4, 64, 16],
    ];

    for events in event_sets {
        for crlf in [false, true] {
            let body = build_runtime_sse_body(&events, crlf);
            let expected = runtime_sse_event_signatures(&collect_runtime_sse_events(&[&body]));

            for chunk_sizes in chunk_size_sets {
                let actual = runtime_sse_event_signatures(
                    &collect_runtime_sse_events_for_chunk_sizes(&body, chunk_sizes),
                );

                assert_eq!(actual, expected, "crlf={crlf} chunks={chunk_sizes:?}");
            }
        }
    }
}

#[test]
fn runtime_sse_helpers_ignore_comments_and_handle_crlf_boundaries() {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut events = Vec::new();

    runtime_sse_consume_chunk(
            &mut line,
            &mut data_lines,
            concat!(
                ": keep-alive\r\n",
                "event: response.created\r\n",
                "data: {\"type\":\"response.created\",\"response_id\":\"resp-1\",\"turn_state\":\"ts-1\"}\r\n",
                "\r\n",
            )
            .as_bytes(),
            |event| events.push(event),
        );

    assert!(line.is_empty());
    assert!(data_lines.is_empty());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].response_ids, vec!["resp-1".to_string()]);
    assert_eq!(events[0].turn_state.as_deref(), Some("ts-1"));
    assert_eq!(events[0].event_type.as_deref(), Some("response.created"));
}

#[test]
fn runtime_sse_helpers_flush_partial_event_at_finish() {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut events = Vec::new();

    runtime_sse_consume_chunk(
        &mut line,
        &mut data_lines,
        b"data: {\"type\":\"response.in_progress\",\"response_id\":\"resp-2\"}",
        |event| events.push(event),
    );
    assert!(events.is_empty());

    runtime_sse_finish_pending(&mut line, &mut data_lines, |event| events.push(event));

    assert!(line.is_empty());
    assert!(data_lines.is_empty());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].response_ids, vec!["resp-2".to_string()]);
    assert_eq!(
        events[0].event_type.as_deref(),
        Some("response.in_progress")
    );
}

#[test]
fn native_response_incomplete_0163_preserves_usage_without_rotation() {
    for reason in [
        "max_output_tokens",
        "context_length_exceeded",
        "content_filter",
        "interrupted",
    ] {
        let payload = serde_json::json!({
            "type": "response.incomplete",
            "response": {
                "id": "resp-native-incomplete-fixture",
                "status": "incomplete",
                "incomplete_details": {"reason": reason},
                "usage": {"input_tokens": 120, "output_tokens": 7, "total_tokens": 127},
                "native_future_field": {"keep": true}
            }
        });
        let lines = vec![payload.to_string()];
        assert_eq!(parse_runtime_sse_payload(&lines), Some(payload));
        let event = parse_runtime_sse_event(&lines);
        assert_eq!(event.event_type.as_deref(), Some("response.incomplete"));
        assert!(!event.quota_blocked && !event.rate_limited && !event.overloaded);
        assert!(!event.previous_response_not_found && !event.invalid_previous_response_id);
        let usage = event
            .token_usage
            .expect("native incomplete usage must not be dropped");
        assert_eq!(usage.input_tokens, 120);
        assert_eq!(usage.output_tokens, 7);
        let wire = format!("data: {}\n\n", lines[0]);
        assert!(matches!(
            inspect_runtime_sse_buffer(wire.as_bytes()),
            RuntimeSseInspectionProgress::Commit { .. }
        ));
    }
}

#[test]
fn runtime_sse_event_extracts_response_token_usage() {
    let events = collect_runtime_sse_events(&[concat!(
        "data: {",
        "\"type\":\"response.completed\",",
        "\"response\":{",
        "\"id\":\"resp-token\",",
        "\"usage\":{",
        "\"input_tokens\":120,",
        "\"input_tokens_details\":{\"cached_tokens\":40},",
        "\"output_tokens\":32,",
        "\"output_tokens_details\":{\"reasoning_tokens\":7}",
        "}",
        "}",
        "}\n\n"
    )
    .as_bytes()]);

    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].token_usage,
        Some(RuntimeTokenUsage {
            input_tokens: 120,
            cached_input_tokens: 40,
            output_tokens: 32,
            reasoning_tokens: 7,
        })
    );
}

#[test]
fn responses_stream_compaction_v2_request_shape_is_context_dependent() {
    let value = serde_json::json!({
        "previous_response_id": "resp-before-compact",
        "session_id": "sess-compact-v2",
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "compact now"}],
            },
            {"type": "compaction_trigger"},
        ],
    });

    assert!(!runtime_request_value_requires_previous_response_affinity(
        &value
    ));
    assert_eq!(
        runtime_request_value_previous_response_fresh_fallback_shape(&value),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("session-id".to_string(), " sess-compact-v2 ".to_string())],
        body: value.to_string().into_bytes(),
    };

    assert_eq!(
        runtime_request_session_id(&request).as_deref(),
        Some("sess-compact-v2")
    );
    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
}

#[test]
fn responses_stream_compaction_v2_sse_is_not_retry_failure() {
    let body = concat!(
        "event: response.created\r\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-compact-v2\",\"headers\":{\"x-codex-turn-state\":\"turn-compact-v2\"}}}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"compaction\",\"encrypted_content\":\"enc-compact-v2\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-compact-v2\"}}\r\n",
        "\r\n",
    );

    let events = collect_runtime_sse_events(&[body.as_bytes()]);
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|event| !event.quota_blocked));
    assert!(
        events
            .iter()
            .all(|event| !event.previous_response_not_found)
    );
    assert_eq!(
        events
            .iter()
            .filter_map(|event| event.event_type.as_deref())
            .collect::<Vec<_>>(),
        vec![
            "response.created",
            "response.output_item.done",
            "response.completed"
        ]
    );
    assert_eq!(events[0].response_ids, vec!["resp-compact-v2".to_string()]);
    assert_eq!(events[0].turn_state.as_deref(), Some("turn-compact-v2"));
    assert_eq!(events[2].response_ids, vec!["resp-compact-v2".to_string()]);

    match inspect_runtime_sse_buffer(body.as_bytes()) {
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        } => {
            assert_eq!(response_ids, vec!["resp-compact-v2".to_string()]);
            assert_eq!(turn_state.as_deref(), Some("turn-compact-v2"));
        }
        other => panic!("compaction v2 stream should be commit-ready, got {other:?}"),
    }
}

#[test]
fn inspect_sse_buffer_extracts_turn_state_from_headers_array() {
    let body = concat!(
        "event: response.created\r\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second\",\"headers\":[[\"x-codex-turn-state\",\"turn-second\"]]}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-second\"}}\r\n",
        "\r\n",
    );

    match inspect_runtime_sse_buffer(body.as_bytes()) {
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        } => {
            assert_eq!(response_ids, vec!["resp-second".to_string()]);
            assert_eq!(turn_state.as_deref(), Some("turn-second"));
        }
        other => panic!("headers-array turn_state stream should be commit-ready, got {other:?}"),
    }
}

#[test]
fn runtime_sse_helpers_preserve_events_across_split_boundaries() {
    let body = concat!(
            ": keep-alive\r\n",
            "data: {\"type\":\"response.created\",\"response_id\":\"resp-1\"}\r\n",
            "\r\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-2\",\"headers\":{\"x-codex-turn-state\":\"ts-2\"}}}\n",
            "\n",
        )
        .as_bytes();
    let expected = runtime_sse_event_signatures(&collect_runtime_sse_events(&[body]));

    for first in 0..=body.len() {
        for second in first..=body.len() {
            let actual = collect_runtime_sse_events(&[
                &body[..first],
                &body[first..second],
                &body[second..],
            ]);
            assert_eq!(
                runtime_sse_event_signatures(&actual),
                expected,
                "unexpected SSE parse for split {first}/{second}"
            );
        }
    }
}

#[test]
fn runtime_sse_inspection_waits_for_complete_event_across_every_split() {
    let cases = [
        (
            "insufficient_quota",
            "data: {\"error\":{\"code\":\"insufficient_quota\"}}\r\n\r\n",
            RuntimeSseInspectionProgress::QuotaBlocked,
        ),
        (
            "rate_limit_exceeded",
            "data: {\"error\":{\"code\":\"rate_limit_exceeded\"}}\n\n",
            RuntimeSseInspectionProgress::RateLimited { retry_after: None },
        ),
        (
            "server_is_overloaded",
            "data: {\"error\":{\"code\":\"server_is_overloaded\"}}\n\n",
            RuntimeSseInspectionProgress::Overloaded,
        ),
        (
            "previous_response_not_found",
            "data: {\"error\":{\"code\":\"previous_response_not_found\"}}\n\n",
            RuntimeSseInspectionProgress::PreviousResponseNotFound,
        ),
        (
            "response.created-response-id-turn-state",
            "event: response.created\r\ndata: { \"type\": \"response.created\", \"response\": { \"id\": \"resp-created-é\", \"headers\": [[\"x-codex-turn-state\", \"turn-created\"]] } }\r\n\r\n",
            RuntimeSseInspectionProgress::Hold {
                response_ids: vec!["resp-created-é".to_string()],
                turn_state: Some("turn-created".to_string()),
            },
        ),
        (
            "escaped-json-string",
            "data: {\"type\":\"response.completed\",\"response_id\":\"resp-\\\"quoted\\\"\"}\n\n",
            RuntimeSseInspectionProgress::Commit {
                response_ids: vec!["resp-\"quoted\"".to_string()],
                turn_state: None,
            },
        ),
    ];

    for (name, body, expected) in cases {
        let bytes = body.as_bytes();
        for split in 0..bytes.len() {
            assert!(
                matches!(
                    inspect_runtime_sse_buffer(&bytes[..split]),
                    RuntimeSseInspectionProgress::Hold { .. }
                ),
                "{name} became terminal before its event separator at split {split}"
            );
        }
        assert_eq!(inspect_runtime_sse_buffer(bytes), expected, "{name}");
    }
}

#[test]
fn runtime_sse_inspection_holds_partial_data_and_response_created_events() {
    for partial in [
        b"data:".as_slice(),
        b"data:\n".as_slice(),
        b"data: {\"type\":\"response.created\",\"response_id\":\"resp-created\"}\n".as_slice(),
    ] {
        assert!(matches!(
            inspect_runtime_sse_buffer(partial),
            RuntimeSseInspectionProgress::Hold { .. }
        ));
    }

    assert_eq!(
        inspect_runtime_sse_buffer(
            b"data: {\"type\":\"response.created\",\"response_id\":\"resp-created\",\"turn_state\":\"turn-created\"}\n\n"
        ),
        RuntimeSseInspectionProgress::Hold {
            response_ids: vec!["resp-created".to_string()],
            turn_state: Some("turn-created".to_string()),
        }
    );
}

#[test]
fn runtime_sse_helpers_drop_invalid_utf8_event_and_recover_next_event() {
    let body = &b"data: {\"type\":\"response.created\",\"response_id\":\"resp-\xff\"}\n\n\
data: {\"type\":\"response.completed\",\"response_id\":\"resp-2\"}\n\n"[..];

    let events = collect_runtime_sse_events(&[body]);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].response_ids, vec!["resp-2".to_string()]);
    assert_eq!(events[0].event_type.as_deref(), Some("response.completed"));
}

include!("payload_detection/error_classification.rs");

#[test]
fn inspect_sse_buffer_keeps_partial_tail_until_eof() {
    let body = concat!(
        ": keep-alive\r\n",
        "data: {\"type\":\"response.completed\",\"response_id\":\"resp-1\",\"turn_state\":\"ts-1\"}\r\n",
        "\r\n",
        "data: {\"type\":\"response.in_progress\",\"response_id\":\"resp-2\"}"
    );
    let progress = inspect_runtime_sse_buffer(body.as_bytes());

    match progress {
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        } => {
            assert_eq!(response_ids, vec!["resp-1".to_string()]);
            assert_eq!(turn_state.as_deref(), Some("ts-1"));
        }
        other => panic!("expected commit progress, got {other:?}"),
    }

    assert_eq!(
        inspect_runtime_sse_buffer_at_eof(body.as_bytes()),
        RuntimeSseInspectionProgress::Commit {
            response_ids: vec!["resp-1".to_string(), "resp-2".to_string()],
            turn_state: Some("ts-1".to_string()),
        }
    );
}

#[test]
fn inspect_sse_buffer_detects_previous_response_not_found_from_partial_event() {
    let progress = inspect_runtime_sse_buffer_at_eof(
        b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"previous_response_not_found\",\"message\":\"missing\"}}}",
    );

    assert!(matches!(
        progress,
        RuntimeSseInspectionProgress::PreviousResponseNotFound
    ));
}

#[test]
fn sse_body_detects_invalid_previous_response_id_inside_data_event() {
    let body = br#"event: response.failed
data: {"type":"error","status":400,"error":{"type":"invalid_request_error","message":"Invalid `previous_response_id`."}}

"#;

    assert!(runtime_sse_body_is_invalid_previous_response_id(body));
    assert!(!runtime_proxy_body_is_invalid_previous_response_id(body));
}

#[test]
fn inspect_sse_buffer_detects_usage_not_included_before_commit() {
    let progress = inspect_runtime_sse_buffer_at_eof(
        b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"usage_not_included\",\"message\":\"Your workspace is out of credits.\"}}}",
    );

    assert!(matches!(
        progress,
        RuntimeSseInspectionProgress::QuotaBlocked
    ));
}

#[test]
fn inspect_sse_buffer_detects_overload_before_commit() {
    let progress = inspect_runtime_sse_buffer_at_eof(
        b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"server_is_overloaded\",\"message\":\"Server is overloaded\"}}}",
    );

    assert!(matches!(progress, RuntimeSseInspectionProgress::Overloaded));
}

#[test]
fn runtime_sse_helpers_preserve_utf8_across_every_split_point() {
    let body = "data: {\"type\":\"response.completed\",\"response_id\":\"resp-é\"}\r\n\r\n";
    let bytes = body.as_bytes();
    for split in 0..=bytes.len() {
        let events = collect_runtime_sse_events(&[&bytes[..split], &bytes[split..]]);
        assert_eq!(events.len(), 1, "unexpected event count at split {split}");
        assert_eq!(events[0].response_ids, vec!["resp-é".to_string()]);
    }
}

include!("payload_detection/previous_response_search.rs");

#[test]
fn compiled_sse_event_state_ignores_data_after_invalid_utf8_until_event_boundary() {
    let before = b"data: {\"type\":\"response.created\",\"response_id\":\"resp-invalid\"}\n";
    let bad_line = b"data: \xff\n";
    let later_in_same_event =
        b"data: {\"type\":\"response.completed\",\"response_id\":\"resp-not-allowed\"}\n";
    let next_event = b"data: {\"type\":\"response.completed\",\"response_id\":\"resp-valid\"}\n\n";
    let mut line = Vec::new();
    let mut pending = Vec::new();
    let mut events = Vec::new();
    for part in [before.as_slice(), bad_line, later_in_same_event] {
        runtime_sse_consume_chunk(&mut line, &mut pending, part, |event| events.push(event));
    }
    assert_eq!(
        pending.len(),
        1,
        "an invalid event must remain marked until EOF/blank"
    );
    assert!(events.is_empty());
    runtime_sse_consume_chunk(&mut line, &mut pending, b"\n", |event| events.push(event));
    assert!(events.is_empty(), "the entire poisoned event is discarded");
    assert!(pending.is_empty());
    runtime_sse_consume_chunk(&mut line, &mut pending, next_event, |event| {
        events.push(event)
    });
    runtime_sse_finish_pending(&mut line, &mut pending, |event| events.push(event));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].response_ids, vec!["resp-valid".to_string()]);
    assert_eq!(events[0].event_type.as_deref(), Some("response.completed"));
}

#[test]
fn compiled_sse_event_state_discards_poisoned_pending_at_eof_and_empty_payloads() {
    let mut line = Vec::new();
    let mut pending = Vec::new();
    let mut events = Vec::new();
    runtime_sse_consume_chunk(
        &mut line,
        &mut pending,
        b"data: {\"type\":\"response.created\"}\ndata: \xff",
        |event| events.push(event),
    );
    runtime_sse_finish_pending(&mut line, &mut pending, |event| events.push(event));
    assert!(events.is_empty());
    assert!(line.is_empty() && pending.is_empty());
    assert_eq!(
        parse_runtime_sse_payload(&["\0prodex-invalid-sse-data".to_string()]),
        None,
    );
    assert_eq!(parse_runtime_sse_payload(&[]), None);
    assert_eq!(
        parse_runtime_sse_payload(&["{\"type\":\"response.created\"}".into()]),
        Some(serde_json::json!({"type":"response.created"})),
    );
}

#[test]
fn compiled_sse_inspection_response_ids_are_sorted_unique_across_unicode_events() {
    let body = [
        "resp-雪",
        "resp-z",
        "resp-é",
        "resp-a",
        "resp-z",
        "resp-🦀",
        "resp-a",
    ]
    .into_iter()
    .map(|id| {
        let event = serde_json::json!({
            "type": "response.created",
            "response_id": id,
        });
        format!("data: {event}\n\n")
    })
    .collect::<String>();
    let expected = vec!["resp-a", "resp-z", "resp-é", "resp-雪", "resp-🦀"];
    for chunks in [
        vec![body.as_bytes()],
        body.as_bytes().chunks(17).collect::<Vec<_>>(),
    ] {
        let mut line = Vec::new();
        let mut pending = Vec::new();
        let mut events = Vec::new();
        for chunk in chunks {
            runtime_sse_consume_chunk(&mut line, &mut pending, chunk, |event| events.push(event));
        }
        runtime_sse_finish_pending(&mut line, &mut pending, |event| events.push(event));
        assert_eq!(events.len(), 7);
    }
    match inspect_runtime_sse_buffer_at_eof(body.as_bytes()) {
        RuntimeSseInspectionProgress::Hold { response_ids, .. }
        | RuntimeSseInspectionProgress::Commit { response_ids, .. } => {
            assert_eq!(response_ids, expected);
        }
        other => panic!("ordinary response events must preserve IDs, got {other:?}"),
    }
}

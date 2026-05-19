use super::*;
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
fn runtime_sse_helpers_drop_invalid_utf8_event_and_recover_next_event() {
    let body = &b"data: {\"type\":\"response.created\",\"response_id\":\"resp-\xff\"}\n\n\
data: {\"type\":\"response.completed\",\"response_id\":\"resp-2\"}\n\n"[..];

    let events = collect_runtime_sse_events(&[body]);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].response_ids, vec!["resp-2".to_string()]);
    assert_eq!(events[0].event_type.as_deref(), Some("response.completed"));
}

#[test]
fn previous_response_message_detects_top_level_json_shape() {
    let payload = serde_json::json!({
        "type": "invalid_request_error",
        "code": "previous_response_not_found",
        "message": "Previous response with id 'resp-123' not found.",
    });

    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&payload),
        Some("Previous response with id 'resp-123' not found.".to_string())
    );
}

#[test]
fn previous_response_message_detects_plain_text_tool_context_signature() {
    let body = b"invalid_request_error: No tool call found for output item call_123";

    assert_eq!(
        extract_runtime_proxy_previous_response_message(body),
        Some("invalid_request_error: No tool call found for output item call_123".to_string())
    );
}

#[test]
fn previous_response_message_ignores_non_error_content_text() {
    let payload = serde_json::json!({
        "type": "response.output_text.delta",
        "delta": "The docs mention previous_response_not_found and No tool call found as examples.",
    });

    assert_eq!(
        extract_runtime_proxy_previous_response_message_from_value(&payload),
        None
    );
}

#[test]
fn quota_message_detects_nested_error_payloads() {
    let payload = serde_json::json!({
        "items": [
            {
                "wrapper": {
                    "error": {
                        "code": "rate_limit_exceeded",
                        "message": "You've hit your usage limit. Try again at 8pm."
                    }
                }
            }
        ]
    });

    assert_eq!(
        extract_runtime_proxy_quota_message_from_value(&payload),
        Some("You've hit your usage limit. Try again at 8pm.".to_string())
    );
}

#[test]
fn quota_http_body_detection_requires_explicit_error_code() {
    assert_eq!(
        extract_runtime_proxy_quota_message(br#"{"error":{"message":"Too Many Requests"}}"#),
        None
    );
    assert_eq!(
        extract_runtime_proxy_quota_message(
            br#"{"error":{"message":"The usage limit has been reached"}}"#
        ),
        None
    );

    for code in ["insufficient_quota", "rate_limit_exceeded"] {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": code,
                "message": "Quota exhausted"
            }
        }))
        .expect("quota body should serialize");

        assert_eq!(
            extract_runtime_proxy_quota_message(&body),
            Some("Quota exhausted".to_string()),
            "{code}"
        );
    }
}

#[test]
fn overload_message_detects_top_level_and_nested_error_payloads() {
    let top_level = serde_json::json!({
        "code": "server_is_overloaded",
        "message": "Upstream Codex backend is currently overloaded.",
    });
    let nested = serde_json::json!({
        "meta": [
            {
                "response": {
                    "error": {
                        "message": "Selected model is at capacity. Please try again."
                    }
                }
            }
        ]
    });

    assert_eq!(
        extract_runtime_proxy_overload_message_from_value(&top_level),
        Some("Upstream Codex backend is currently overloaded.".to_string())
    );
    assert_eq!(
        extract_runtime_proxy_overload_message_from_value(&nested),
        Some("Selected model is at capacity. Please try again.".to_string())
    );
}

#[test]
fn invalid_utf8_bodies_do_not_trigger_lossy_retry_classification() {
    assert_eq!(
        extract_runtime_proxy_quota_message(b"\xffYou've hit your usage limit. Try again at 8pm."),
        None
    );
    assert_eq!(
        extract_runtime_proxy_previous_response_message(
            b"\xffprevious_response_not_found: missing"
        ),
        None
    );
    assert_eq!(
        extract_runtime_proxy_overload_message(
            503,
            b"\xffSelected model is at capacity. Please try again."
        ),
        Some("Upstream Codex backend returned transient HTTP 503.".to_string())
    );
    assert_eq!(
        extract_runtime_proxy_overload_message(500, b"\xff"),
        Some("Upstream Codex backend is currently experiencing high demand.".to_string())
    );
}

#[test]
fn inspect_sse_buffer_handles_comments_crlf_and_partial_tail() {
    let progress = inspect_runtime_sse_buffer(
            concat!(
                ": keep-alive\r\n",
                "data: {\"type\":\"response.completed\",\"response_id\":\"resp-1\",\"turn_state\":\"ts-1\"}\r\n",
                "\r\n",
                "data: {\"type\":\"response.in_progress\",\"response_id\":\"resp-2\"}"
            )
            .as_bytes(),
        );

    match progress {
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        } => {
            assert_eq!(
                response_ids,
                vec!["resp-1".to_string(), "resp-2".to_string()]
            );
            assert_eq!(turn_state.as_deref(), Some("ts-1"));
        }
        other => panic!("expected commit progress, got {other:?}"),
    }
}

#[test]
fn inspect_sse_buffer_detects_previous_response_not_found_from_partial_event() {
    let progress = inspect_runtime_sse_buffer(
            b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"previous_response_not_found\",\"message\":\"missing\"}}}",
        );

    assert!(matches!(
        progress,
        RuntimeSseInspectionProgress::PreviousResponseNotFound
    ));
}

#[test]
fn body_snippet_normalizes_whitespace_and_truncates() {
    assert_eq!(
        runtime_proxy_body_snippet(b" one\n two\tthree ", 7),
        "one two..."
    );
    assert_eq!(runtime_proxy_body_snippet(b"  \n\t  ", 7), "-");
    assert_eq!(
        runtime_proxy_body_snippet(b"bad-\xff bytes", 64),
        "bad-\u{fffd} bytes"
    );
}

#[test]
fn response_ids_from_payload_matches_body_bytes_and_ignores_invalid_json() {
    let payload = r#"{
        "response": {"id": "resp-a"},
        "response_id": "resp-b",
        "object": "response",
        "id": "resp-c"
    }"#;

    assert_eq!(
        extract_runtime_response_ids_from_payload(payload),
        extract_runtime_response_ids_from_body_bytes(payload.as_bytes())
    );
    assert_eq!(
        extract_runtime_response_ids_from_payload(payload),
        vec![
            "resp-a".to_string(),
            "resp-b".to_string(),
            "resp-c".to_string()
        ]
    );
    assert!(extract_runtime_response_ids_from_payload("{").is_empty());
}

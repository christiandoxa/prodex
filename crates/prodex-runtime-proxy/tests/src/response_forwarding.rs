use super::*;

#[test]
fn skips_runtime_response_headers_case_insensitively() {
    assert!(should_skip_runtime_response_header("Connection"));
    assert!(should_skip_runtime_response_header("content-length"));
    assert!(should_skip_runtime_response_header("Keep-Alive"));
    assert!(should_skip_runtime_response_header("Proxy-Authenticate"));
    assert!(should_skip_runtime_response_header("Proxy-Authorization"));
    assert!(should_skip_runtime_response_header("TE"));
    assert!(should_skip_runtime_response_header("Trailer"));
    assert!(should_skip_runtime_response_header("TRANSFER-ENCODING"));
    assert!(should_skip_runtime_response_header("Upgrade"));
    assert!(!should_skip_runtime_response_header("content-encoding"));
    assert!(!should_skip_runtime_response_header("x-codex-turn-state"));
    assert!(!should_skip_runtime_response_header("content-type"));
}

#[test]
fn filters_text_response_headers_without_rewriting_values() {
    let headers = runtime_forward_text_response_headers([
        ("Content-Type", "text/event-stream"),
        ("Content-Encoding", "gzip"),
        ("Server", "upstream"),
        ("x-codex-turn-state", " ts-1 "),
    ]);

    assert_eq!(
        headers,
        vec![
            ("Content-Type".to_string(), "text/event-stream".to_string()),
            ("Content-Encoding".to_string(), "gzip".to_string()),
            ("x-codex-turn-state".to_string(), " ts-1 ".to_string()),
        ]
    );
}

#[test]
fn filters_connection_named_text_response_headers() {
    let headers = runtime_forward_text_response_headers([
        ("Connection", "keep-alive, X-Local-Hop"),
        ("X-Local-Hop", "strip-me"),
        ("x-codex-turn-state", "keep-me"),
    ]);

    assert_eq!(
        headers,
        vec![("x-codex-turn-state".to_string(), "keep-me".to_string())]
    );
}

#[test]
fn preserves_codex_0142_backend_response_headers() {
    let headers = runtime_forward_text_response_headers([
        ("x-request-id", "req-0142"),
        ("openai-model", "gpt-5"),
        ("x-models-etag", "models-etag-0142"),
        ("x-reasoning-included", "true"),
        ("x-codex-primary-used-percent", "12.5"),
        ("x-codex-primary-window-minutes", "60"),
        ("x-codex-credits-has-credits", "true"),
        ("x-codex-turn-state", "turn-state-0142"),
        ("x-codex-safety-buffering-enabled", "true"),
        ("x-codex-safety-buffering-faster-model", "gpt-5-mini"),
        ("Content-Length", "999"),
    ]);

    assert_eq!(
        headers,
        vec![
            ("x-request-id".to_string(), "req-0142".to_string()),
            ("openai-model".to_string(), "gpt-5".to_string()),
            ("x-models-etag".to_string(), "models-etag-0142".to_string()),
            ("x-reasoning-included".to_string(), "true".to_string()),
            (
                "x-codex-primary-used-percent".to_string(),
                "12.5".to_string()
            ),
            (
                "x-codex-primary-window-minutes".to_string(),
                "60".to_string()
            ),
            (
                "x-codex-credits-has-credits".to_string(),
                "true".to_string()
            ),
            (
                "x-codex-turn-state".to_string(),
                "turn-state-0142".to_string()
            ),
            (
                "x-codex-safety-buffering-enabled".to_string(),
                "true".to_string()
            ),
            (
                "x-codex-safety-buffering-faster-model".to_string(),
                "gpt-5-mini".to_string()
            ),
        ]
    );
}

#[test]
fn filters_binary_response_headers_without_utf8_requirement() {
    let headers = runtime_forward_binary_response_headers([
        ("Date", b"today".as_slice()),
        ("x-binary", b"\xff\x00".as_slice()),
    ]);

    assert_eq!(
        headers,
        vec![("x-binary".to_string(), b"\xff\x00".to_vec())]
    );
}

#[test]
fn filters_connection_named_binary_response_headers() {
    let headers = runtime_forward_binary_response_headers([
        ("Connection", b"X-Local-Hop".as_slice()),
        ("X-Local-Hop", b"strip-me".as_slice()),
        ("x-binary", b"\xff\x00".as_slice()),
    ]);

    assert_eq!(
        headers,
        vec![("x-binary".to_string(), b"\xff\x00".to_vec())]
    );
}

#[test]
fn extracts_buffered_response_metadata_from_binary_headers() {
    let headers = [
        ("x-extra", b"ignored".as_slice()),
        ("content-type", b" application/json ".as_slice()),
    ];

    let metadata = runtime_buffered_response_metadata(201, headers, 42);

    assert_eq!(
        metadata,
        RuntimeBufferedResponseMetadata {
            status: 201,
            content_type: Some("application/json"),
            body_bytes: 42,
        }
    );
}

#[test]
fn classifies_sse_content_type_case_insensitively() {
    assert_eq!(
        runtime_response_forwarding_body_kind(Some("text/event-stream; charset=utf-8")),
        RuntimeResponseForwardingBodyKind::Sse
    );
    assert_eq!(
        runtime_response_forwarding_body_kind(Some("TEXT/EVENT-STREAM")),
        RuntimeResponseForwardingBodyKind::Sse
    );
    assert_eq!(
        runtime_response_forwarding_body_kind(None),
        RuntimeResponseForwardingBodyKind::Unary
    );
}

#[test]
fn extracts_response_header_values_case_insensitively_and_trims() {
    let value = runtime_response_header_value(
        [
            ("x-empty", "   "),
            ("X-Codex-Turn-State", " ts-1 "),
            ("x-codex-turn-state", "ts-2"),
        ],
        "x-codex-turn-state",
    );

    assert_eq!(value.as_deref(), Some("ts-1"));
    assert_eq!(
        runtime_response_header_value([("x-empty", "   ")], "x-empty"),
        None
    );
}

#[test]
fn detects_sse_stream_headers_for_chunk_flush_policy() {
    assert!(runtime_stream_response_should_flush_each_chunk([(
        "Content-Type",
        "TEXT/EVENT-STREAM; charset=utf-8"
    ),]));
    assert!(!runtime_stream_response_should_flush_each_chunk([(
        "Content-Type",
        "application/json"
    ),]));
}

#[test]
fn builds_sse_commit_detail_for_logs() {
    assert_eq!(
        runtime_sse_forwarding_commit_detail(17, 2),
        RuntimeSseForwardingCommitDetail {
            prelude_bytes: 17,
            response_id_count: 2,
        }
    );
}

#[test]
fn token_usage_logging_accepts_terminal_and_completed_events() {
    assert!(runtime_token_usage_event_is_loggable(None));
    assert!(runtime_token_usage_event_is_loggable(Some(
        "response.completed"
    )));
    assert!(runtime_token_usage_event_is_loggable(Some(
        "tool.completed"
    )));
    assert!(!runtime_token_usage_event_is_loggable(Some(
        "response.delta"
    )));
}

#[test]
fn sse_tap_state_rebinds_remembered_response_ids_when_turn_state_arrives() {
    let mut state = RuntimeSseTapState::new(RuntimeSseTapStateInit {
        remembered_response_ids: &["resp-1".to_string()],
        request_previous_response_id: None,
        turn_state: None,
    });

    let effects =
        state.observe_chunk(br#"data: {"type":"response.in_progress","turn_state":"ts-1"}"#);
    assert!(effects.is_empty());

    let effects = state.observe_chunk(b"\r\n\r\n");
    assert_eq!(
        effects,
        vec![RuntimeSseTapEffect::RememberResponseIds {
            response_ids: vec!["resp-1".to_string()],
            turn_state: Some("ts-1".to_string()),
        }]
    );
}

#[test]
fn sse_tap_state_reports_dead_chain_after_previous_response_error() {
    let mut state = RuntimeSseTapState::new(RuntimeSseTapStateInit {
        remembered_response_ids: &["resp-live".to_string()],
        request_previous_response_id: Some("resp-prev"),
        turn_state: None,
    });

    let effects = state.observe_chunk(
            br#"data: {"type":"response.failed","response":{"error":{"code":"previous_response_not_found"}}}"#,
        );
    assert!(effects.is_empty());

    let effects = state.observe_chunk(b"\r\n\r\n");
    assert_eq!(
        effects,
        vec![RuntimeSseTapEffect::ClearDeadResponseBindings {
            response_ids: vec!["resp-live".to_string(), "resp-prev".to_string()],
        }]
    );
}

#[test]
fn sse_tap_state_deduplicates_token_usage_logs() {
    let usage_event = br#"data: {"type":"response.completed","response":{"usage":{"input_tokens":3,"output_tokens":5}}}"#;
    let mut state = RuntimeSseTapState::default();

    let mut effects = state.observe_chunk(usage_event);
    effects.extend(state.observe_chunk(b"\r\n\r\n"));
    effects.extend(state.observe_chunk(usage_event));
    effects.extend(state.observe_chunk(b"\r\n\r\n"));

    assert_eq!(
        effects,
        vec![RuntimeSseTapEffect::LogTokenUsage(RuntimeTokenUsage {
            input_tokens: 3,
            output_tokens: 5,
            ..RuntimeTokenUsage::default()
        })]
    );
}

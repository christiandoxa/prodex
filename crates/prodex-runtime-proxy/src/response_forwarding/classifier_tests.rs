use super::*;

#[test]
fn response_header_skip_expected_values_include_unicode_trimming() {
    for (name, expected) in [
        ("Connection", true),
        (" content-length ", true),
        ("\u{00a0}Transfer-Encoding\u{2003}", true),
        ("\u{2003}x-codex-turn-state\u{00a0}", false),
        ("Keep-Alive", true),
        ("TRANSFER-ENCODING", true),
        ("content length", false),
        ("\u{200b}Upgrade", false),
        ("", false),
    ] {
        assert_eq!(
            should_skip_runtime_response_header(name),
            expected,
            "header {name:?}"
        );
    }

    assert_eq!(
        runtime_forward_text_response_header("\u{00a0}Content-Length\u{2003}", "99"),
        None
    );
    assert_eq!(
        runtime_forward_text_response_header("x-codex-turn-state", " turn-state "),
        Some(("x-codex-turn-state".to_string(), " turn-state ".to_string()))
    );
}

#[test]
fn response_header_classifier_rejects_invalid_utf8_at_abi_boundary() {
    let malformed = [0xc3_u8, 0x28];
    assert_eq!(
        unsafe {
            prodex_runtime_response_forwarding_classify_v1(
                0,
                malformed.as_ptr() as usize as u64,
                malformed.len() as i64,
                1,
                0,
            )
        },
        -1
    );
}

#[test]
fn required_mojo_header_and_content_type_abis_fail_closed_on_malformed_input() {
    let malformed = [0xc3_u8, 0x28];
    assert_eq!(
        unsafe {
            prodex_runtime_response_forwarding_header_v1(
                malformed.as_ptr() as usize as u64,
                malformed.len() as i64,
                0,
                0,
                0,
            )
        },
        -1
    );
    assert_eq!(
        unsafe {
            prodex_runtime_response_forwarding_content_type_v1(
                b"Content-Type".as_ptr() as usize as u64,
                12,
                malformed.as_ptr() as usize as u64,
                malformed.len() as i64,
            )
        },
        0
    );
    assert_eq!(
        unsafe {
            prodex_runtime_response_forwarding_header_v1(
                b"x-local-hop".as_ptr() as usize as u64,
                b"x-local-hop".len() as i64,
                b"keep-alive, X-LOCAL-HOP".as_ptr() as usize as u64,
                b"keep-alive, X-LOCAL-HOP".len() as i64,
                1,
            )
        },
        2
    );
    assert_eq!(
        unsafe {
            prodex_runtime_response_forwarding_header_v1(
                b" Content-Length ".as_ptr() as usize as u64,
                b" Content-Length ".len() as i64,
                0,
                0,
                0,
            )
        },
        1
    );
}

#[test]
fn required_mojo_attempt_abi_rejects_invalid_tags() {
    assert_eq!(
        unsafe { prodex_runtime_response_forwarding_attempt_v1(200, 6, 0, 0, 0, 0) },
        -1
    );
    assert_eq!(
        unsafe { prodex_runtime_response_forwarding_attempt_v1(200, 0, 0, 0, 0, 2) },
        -1
    );
}

#[test]
fn sse_content_type_expected_values_and_body_boundary() {
    for (content_type, expected) in [
        (None, false),
        (Some(""), false),
        (Some("application/json"), false),
        (Some(" text/event-stream; charset=utf-8 "), true),
        (Some("TEXT/EVENT-STREAM"), true),
        (Some("prefix text/event-stream suffix"), true),
        (Some("世界 TEXT/EVENT-STREAM"), true),
        (Some("text/event-streaming"), true),
    ] {
        assert_eq!(
            runtime_response_content_type_is_sse(content_type),
            expected,
            "content type {content_type:?}"
        );
        assert_eq!(
            runtime_response_forwarding_body_kind(content_type),
            if expected {
                RuntimeResponseForwardingBodyKind::Sse
            } else {
                RuntimeResponseForwardingBodyKind::Unary
            }
        );
    }

    assert!(runtime_stream_response_should_flush_each_chunk([(
        "Content-Type",
        "TEXT/EVENT-STREAM; charset=utf-8"
    )]));
    assert!(!runtime_stream_response_should_flush_each_chunk([(
        "Content-Type",
        "application/json"
    )]));
}

#[test]
fn token_usage_loggability_keeps_absent_exact_and_suffix_cases() {
    for (event_type, expected) in [
        (None, true),
        (Some("response.completed"), true),
        (Some("response.failed"), true),
        (Some("tool.completed"), true),
        (Some("é.completed"), true),
        (Some(".completed"), true),
        (Some("completed"), false),
        (Some("response.delta"), false),
        (Some("response.completed.extra"), false),
        (Some("RESPONSE.COMPLETED"), false),
        (Some(""), false),
    ] {
        assert_eq!(
            runtime_token_usage_event_is_loggable(event_type),
            expected,
            "event loggability {event_type:?}"
        );
    }
}

#[test]
fn generation_start_expected_values_drive_live_usage_boundary() {
    for event_type in [
        "response.output_text.delta",
        "response.refusal.delta",
        "response.reasoning_summary_text.delta",
        "response.reasoning_text.delta",
        "response.function_call_arguments.delta",
        "response.mcp_call_arguments.delta",
        "response.custom_tool_call_input.delta",
    ] {
        assert!(
            runtime_response_event_is_generation_start(Some(event_type)),
            "generation start {event_type:?}"
        );
        assert!(runtime_token_usage_event_is_live(
            Some(event_type),
            Some(RuntimeTokenUsage {
                output_tokens: 1,
                ..RuntimeTokenUsage::default()
            })
        ));
    }

    for event_type in [
        "response.output_item.added",
        "response.output_text.delta.extra",
        "prefix.response.output_text.delta",
        "response.created",
        "",
    ] {
        assert!(
            !runtime_response_event_is_generation_start(Some(event_type)),
            "not a generation start: {event_type:?}"
        );
    }
    assert!(!runtime_response_event_is_generation_start(None));
    assert!(runtime_websocket_terminal_should_reset(
        Some("response.failed"),
        false
    ));
    assert!(!runtime_websocket_terminal_should_reset(
        Some("response.failed"),
        true
    ));
    assert!(runtime_response_ids_should_record(false));
    assert!(!runtime_response_ids_should_record(true));
    assert!(runtime_committed_previous_response_not_found(true, true));
    assert!(!runtime_committed_previous_response_not_found(false, true));
    assert!(runtime_response_generation_should_start(
        Some("response.output_text.delta"),
        false
    ));
    assert!(!runtime_response_generation_should_start(
        Some("response.output_text.delta"),
        true
    ));
    assert!(!runtime_token_usage_event_is_live(
        Some("response.output_text.delta"),
        Some(RuntimeTokenUsage::default())
    ));
}

#[test]
fn tap_plan_is_single_mojo_decision_for_stream_boundaries() {
    // Bits: generation-start, terminal completion, live progress, loggable usage.
    let cases = [
        (None, 3, 0b1000),
        (Some("response.completed"), 3, 0b1010),
        (Some("response.completed"), 0, 0b1010),
        (Some("response.failed"), 0, 0b1000),
        (Some("tool.completed"), 1, 0b1000),
        (Some("é.completed"), 1, 0b1000),
        (Some("response.output_text.delta"), 3, 0b0101),
        (Some("response.output_text.delta"), 0, 0b0001),
        (Some("response.reasoning_summary_text.delta"), 1, 0b0101),
        (Some("response.function_call_arguments.delta"), 1, 0b0101),
        (Some("response.output_text.delta.extra"), 3, 0),
        (Some("response.created"), 2, 0),
        (Some("response.completed.extra"), 2, 0),
        (Some("RESPONSE.COMPLETED"), 2, 0),
        (Some(""), 0, 0),
    ];
    for (event_type, output_tokens, expected) in cases {
        let actual = runtime_sse_tap_plan(event_type, output_tokens);
        assert_eq!(
            actual, expected,
            "event={event_type:?}, tokens={output_tokens}"
        );
    }
    assert_eq!(
        runtime_sse_tap_plan(Some("response.output_text.delta"), u64::MAX),
        0b0101
    );
}

#[test]
fn responses_stream_inspection_does_not_depend_on_a_missing_mime_header() {
    for (mime, requested, expected) in [
        (None, true, true),
        (None, false, false),
        (Some(""), true, true),
        (Some("  "), true, true),
        (Some("text/event-stream"), false, true),
        (Some("TEXT/EVENT-STREAM; charset=utf-8"), true, true),
        (Some("application/json"), true, false),
        (Some("application/json"), false, false),
        (Some("text/plain"), true, false),
    ] {
        assert_eq!(
            runtime_responses_should_inspect_sse(mime, requested),
            expected,
            "MIME={mime:?}, requested={requested}"
        );
    }
    // The generic content-type helper must not silently change its contract.
    assert!(!runtime_response_content_type_is_sse(None));
    assert!(!runtime_response_content_type_is_sse(Some("")));
}

#[test]
fn compat_replay_codex_sse_fixture_matches_golden_surface_and_stream_parse() {
    let request =
        compat_replay_request_from_fixture(&compat_replay_fixture_json("codex_sse_request.json"));
    let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");
    compat_replay_assert_golden(
        compat_replay_surface_value(surface),
        "codex_sse_expected_surface.json",
    );

    let stream = compat_replay_fixture_text("codex_sse_stream.txt");
    compat_replay_assert_golden(
        compat_replay_sse_payload_values(stream),
        "codex_sse_expected_payloads.json",
    );
    let inspection = inspect_runtime_sse_buffer(stream.as_bytes());
    compat_replay_assert_golden(
        compat_replay_sse_inspection_value(inspection),
        "codex_sse_expected_inspection.json",
    );
}

#[test]
fn compat_replay_codex_websocket_fixture_matches_golden_surface_and_metadata() {
    let replay = compat_replay_fixture_json("codex_websocket_replay.json");
    let handshake = compat_replay_request_from_fixture(
        replay
            .get("handshake")
            .expect("websocket replay fixture should include handshake"),
    );
    let message = replay
        .get("message")
        .expect("websocket replay fixture should include message")
        .to_string();

    let surface =
        runtime_detect_websocket_message_compatibility_surface(&handshake, message.as_str());
    compat_replay_assert_golden(
        compat_replay_surface_value(surface),
        "codex_websocket_expected_surface.json",
    );

    let metadata = parse_runtime_websocket_request_metadata(message.as_str());
    compat_replay_assert_golden(
        serde_json::json!({
            "previous_response_id": metadata.previous_response_id,
            "session_id": metadata.session_id,
            "requires_previous_response_affinity": metadata.requires_previous_response_affinity,
            "fresh_fallback_shape": runtime_previous_response_fresh_fallback_shape_label(
                metadata.previous_response_fresh_fallback_shape,
            ),
        }),
        "codex_websocket_expected_metadata.json",
    );
}

#[test]
fn compat_replay_claude_mcp_fixture_matches_golden_surface_and_translation() {
    let request =
        compat_replay_request_from_fixture(&compat_replay_fixture_json("claude_mcp_request.json"));
    let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");
    compat_replay_assert_golden(
        compat_replay_surface_value(surface),
        "claude_mcp_expected_surface.json",
    );

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("fixture should translate");
    assert!(translated.stream);
    assert!(translated.server_tools.mcp);
    compat_replay_assert_golden(
        compat_replay_translated_request_value(&translated.translated_request),
        "claude_mcp_expected_translation.json",
    );
}

#[test]
fn compat_replay_capture_sample_fixture_is_scrubbed() {
    let text = compat_replay_fixture_text("sample_capture_replay.json");
    serde_json::from_str::<Value>(text).expect("sample capture replay fixture should be JSON");

    let mut needles = compat_replay_sample_fake_secrets();
    needles.extend(
        [
            "resp_parent_live",
            "sess_body_live",
            "turn_live_20260428",
            "call_ws_live",
            "2026-04-28T01:02:03Z",
        ]
        .map(str::to_string),
    );
    for needle in needles {
        assert!(
            !text.contains(&needle),
            "sample replay fixture should not contain raw capture value {needle}"
        );
    }
    for placeholder in [
        "<redacted>",
        "<session_id>",
        "<previous_response_id>",
        "<response_id>",
        "<turn_state>",
    ] {
        assert!(
            text.contains(placeholder),
            "sample replay fixture should contain placeholder {placeholder}"
        );
    }
}


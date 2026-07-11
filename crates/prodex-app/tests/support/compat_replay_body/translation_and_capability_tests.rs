#[test]
fn compat_replay_anthropic_translation_is_stable_across_header_noise() {
    let variants = compat_replay_anthropic_contract_variants();
    let expected = compat_replay_translate_anthropic_request(
        variants.first().expect("at least one replay variant"),
    );

    for request in variants {
        let snapshot = compat_replay_translate_anthropic_request(&request);
        assert_eq!(
            snapshot, expected,
            "Anthropic replay snapshot drifted under header noise"
        );
    }
}

#[test]
fn compat_replay_anthropic_translation_survives_chaos_replay_loops() {
    let variants = compat_replay_anthropic_contract_variants();
    let mut previous_snapshot = None;

    for iteration in 0..48 {
        let request = &variants[iteration % variants.len()];
        let snapshot = compat_replay_translate_anthropic_request(request);
        if let Some(previous_snapshot) = previous_snapshot.as_ref() {
            assert_eq!(
                snapshot, *previous_snapshot,
                "replay snapshot changed during a soak-style replay loop"
            );
        }
        previous_snapshot = Some(snapshot);
    }
}

#[test]
fn compat_replay_claude_session_id_header_is_case_insensitive() {
    let request = compat_replay_anthropic_request(
        vec![
            ("X-Claude-Code-Session-Id", "claude-session-42"),
            ("Content-Type", "application/json"),
        ],
        compat_replay_anthropic_contract_body(),
    );

    assert_eq!(
        runtime_proxy_claude_session_id(&request),
        Some("claude-session-42".to_string())
    );
}

#[test]
fn compat_replay_request_capabilities_detect_claude_code_mcp_stream() {
    let request = compat_replay_anthropic_request(
        vec![
            ("User-Agent", "claude-code/1.2.3"),
            ("x-claude-code-session-id", "claude-session-42"),
            ("Content-Type", "application/json"),
            ("anthropic-version", "2023-06-01"),
        ],
        compat_replay_anthropic_contract_body(),
    );

    let surface = runtime_detect_request_compatibility_surface(&request, "request", "http");

    assert_eq!(surface.family, "claude_code");
    assert_eq!(surface.client, "claude_code");
    assert_eq!(surface.route, "anthropic_messages");
    assert_eq!(surface.transport, "http");
    assert_eq!(surface.stream, "streaming");
    assert_eq!(surface.tool_surface, "mcp");
    assert_eq!(surface.continuation, "none");
    assert!(!surface.approval);
    assert!(surface.warnings.is_empty());
}

#[test]
fn compat_replay_websocket_capabilities_warn_without_turn_state() {
    let handshake_request =
        compat_replay_websocket_handshake_request(vec![("User-Agent", "codex-cli/test-fixture")]);
    let surface = runtime_detect_websocket_message_compatibility_surface(
        &handshake_request,
        r#"{"previous_response_id":"resp_123","input":[]}"#,
    );

    assert_eq!(surface.family, "codex");
    assert_eq!(surface.client, "codex_cli");
    assert_eq!(surface.route, "responses");
    assert_eq!(surface.transport, "websocket");
    assert_eq!(surface.stream, "streaming");
    assert_eq!(surface.continuation, "previous_response");
    assert!(
        surface
            .warnings
            .contains(&"websocket_previous_response_without_turn_state")
    );
}

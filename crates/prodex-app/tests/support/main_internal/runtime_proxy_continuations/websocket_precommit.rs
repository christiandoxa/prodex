use super::*;

#[test]
fn runtime_proxy_realtime_forwards_multiple_client_frames_before_upstream_output() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_realtime_sideband(),
        "main",
        &["main"],
        &[],
        Vec::new(),
    );
    let mut socket = fixture.connect_realtime_websocket("backend-api/prodex/live");

    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({"type": "session.update"}),
    );
    let (_, session) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"session.updated\"")
    });
    assert!(session.contains("sess-realtime"), "{session}");

    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({"type": "input_audio.append", "audio": "chunk-one"}),
    );
    socket
        .send(WsMessage::Ping("realtime-ping".into()))
        .expect("realtime ping should send while upstream is silent");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({"type": "input_audio.append", "audio": "chunk-two"}),
    );

    let mut saw_pong = false;
    let output = loop {
        match socket.read().expect("realtime frame should arrive") {
            WsMessage::Pong(payload) => saw_pong |= payload.as_ref() == b"realtime-ping",
            WsMessage::Text(text) if text.contains("\"type\":\"output_audio.delta\"") => {
                break text.to_string();
            }
            _ => {}
        }
    };
    let _ = socket.close(None);

    assert!(saw_pong, "local ping must not wait for upstream output");
    assert!(output.contains("fixture-audio"), "{output}");
    let requests = fixture.backend.websocket_requests();
    assert_eq!(requests.len(), 3, "{requests:?}");
    assert!(requests[1].contains("chunk-one"), "{requests:?}");
    assert!(requests[2].contains("chunk-two"), "{requests:?}");
    let log = fixture.wait_for_log(|log| {
        log.contains("upstream_connect_start") && log.contains("/backend-api/codex/live")
    });
    assert!(log.contains("/backend-api/codex/live"), "{log}");
}

#[test]
fn runtime_proxy_websocket_fresh_request_rotates_past_usage_limit_account() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket(),
        "main",
        &["main", "second"],
        &[],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [{
                "type": "message",
                "role": "user",
                "content": "continue on the next healthy account"
            }],
        }),
    );

    let (frames, completed_message) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
    });
    let _ = socket.close(None);

    assert!(
        completed_message.contains("\"response\":{\"id\":\"resp-second\"}"),
        "fresh websocket request should complete on the later healthy profile: {completed_message}"
    );
    assert!(
        frames.iter().all(|frame| !frame.contains("usage limit")),
        "retryable usage-limit websocket failures must not leak after later-profile success: {frames:?}"
    );
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()],
        "websocket fresh request should try the active profile first, then rotate to the ready profile"
    );

    let log = fixture.wait_for_log(|log| {
        log.contains("request=")
            && log.contains("quota_blocked profile=main")
            && log.contains("transport=websocket committed profile=second")
    });
    assert!(
        log.contains("quota_blocked profile=main")
            && log.contains("transport=websocket committed profile=second"),
        "websocket quota-blocked owner should rotate and commit the healthy profile: {log}"
    );
}

#[test]
fn runtime_proxy_websocket_keepalive_before_content_does_not_commit_or_block_forwarding() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_keepalive_before_content(),
        "second",
        &["second"],
        &[],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");

    socket
        .send(WsMessage::Ping("local-ping-before-content".into()))
        .expect("local websocket ping should send");
    match socket.read().expect("local websocket pong should read") {
        WsMessage::Pong(payload) => assert_eq!(payload.as_ref(), b"local-ping-before-content"),
        other => panic!("expected local pong before request content, got {other:?}"),
    }

    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [{
                "type": "message",
                "role": "user",
                "content": "hello"
            }],
        }),
    );

    let (frames, completed_message) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
    });
    let _ = socket.close(None);

    assert!(
        frames
            .iter()
            .any(|frame| frame.contains("\"type\":\"response.created\"")),
        "normal response.created should be forwarded after upstream keepalive: {frames:?}"
    );
    assert!(
        completed_message.contains("\"response\":{\"id\":\"resp-second\"}"),
        "normal response.completed should be forwarded after upstream keepalive: {completed_message}"
    );
    assert_eq!(
        fixture.backend.websocket_requests().len(),
        1,
        "upstream keepalive should not force retry or replay"
    );

    let log = fixture.wait_for_log(|log| {
        log.contains("request=") && log.contains("transport=websocket committed profile=second")
    });
    assert_eq!(
        log.matches("transport=websocket committed profile=second")
            .count(),
        1,
        "keepalive frames should not be logged as model-output commits: {log}"
    );
    assert!(
        !log.contains("websocket_upstream_read_error"),
        "keepalive frames should not block normal websocket forwarding: {log}"
    );
}

#[test]
fn runtime_proxy_websocket_preserves_rich_turn_metadata_handshake_header() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let turn_metadata = codex_0135_compaction_turn_metadata();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket(),
        "second",
        &["second"],
        &[],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket_with_headers(
        "backend-api/prodex/responses",
        &[
            runtime_continuation_header("session_id", "sess-rich-metadata"),
            runtime_continuation_header("x-codex-turn-state", "turn-rich-metadata"),
            runtime_continuation_header("x-codex-turn-metadata", turn_metadata.clone()),
        ],
    );
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [{
                "type": "message",
                "role": "user",
                "content": "hello"
            }],
        }),
    );

    let (_frames, completed_message) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
    });
    let _ = socket.close(None);

    assert!(
        completed_message.contains("\"response\":{\"id\":\"resp-second\"}"),
        "normal websocket response should complete: {completed_message}"
    );
    let responses_headers = fixture.backend.responses_headers();
    assert_eq!(
        responses_headers.len(),
        1,
        "backend should observe one websocket handshake: {responses_headers:?}"
    );
    assert_eq!(
        responses_headers[0]
            .get("x-codex-turn-metadata")
            .map(String::as_str),
        Some(turn_metadata.as_str()),
        "websocket handshake should preserve Codex 0.135 turn metadata unchanged"
    );
}

#[test]
fn runtime_proxy_websocket_preserves_codex_0142_backend_handshake_headers() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let turn_metadata = serde_json::json!({
        "session_id": "sess-ws-0142",
        "thread_id": "thread-ws-0142",
        "turn_id": "turn-ws-0142",
        "window_id": "thread-ws-0142:1",
    })
    .to_string();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket(),
        "second",
        &["second"],
        &[],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket_with_headers(
        "backend-api/prodex/responses",
        &[
            runtime_continuation_header("authorization", "Bearer caller-token"),
            runtime_continuation_header("chatgpt-account-id", "caller-account"),
            runtime_continuation_header("session-id", "sess-ws-0142"),
            runtime_continuation_header("thread-id", "thread-ws-0142"),
            runtime_continuation_header("x-client-request-id", "thread-ws-0142"),
            runtime_continuation_header("x-codex-turn-metadata", turn_metadata.clone()),
            runtime_continuation_header("x-codex-turn-state", "turn-state-ws-0142"),
            runtime_continuation_header("x-codex-beta-features", "goals,plugins"),
            runtime_continuation_header("x-oai-attestation", "attestation-ws-0142"),
            runtime_continuation_header("openai-beta", "responses_websockets=2026-02-06"),
            runtime_continuation_header("x-responsesapi-include-timing-metrics", "true"),
            runtime_continuation_header("x-openai-internal-codex-responses-lite", "true"),
            runtime_continuation_header("user-agent", "codex-cli/0.142.0"),
        ],
    );
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [{
                "type": "message",
                "role": "user",
                "content": "hello"
            }],
            "client_metadata": {
                "session_id": "sess-ws-0142",
                "thread_id": "thread-ws-0142",
                "x-codex-turn-metadata": turn_metadata,
                "ws_request_header_x_openai_internal_codex_responses_lite": "true"
            }
        }),
    );

    let (_frames, completed_message) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
    });
    let _ = socket.close(None);

    assert!(
        completed_message.contains("\"response\":{\"id\":\"resp-second\"}"),
        "normal websocket response should complete: {completed_message}"
    );
    let responses_headers = fixture.backend.responses_headers();
    assert_eq!(
        responses_headers.len(),
        1,
        "backend should observe one websocket handshake: {responses_headers:?}"
    );
    let headers = &responses_headers[0];
    for (name, expected) in [
        ("session-id", "sess-ws-0142"),
        ("thread-id", "thread-ws-0142"),
        ("x-client-request-id", "thread-ws-0142"),
        ("x-codex-turn-metadata", turn_metadata.as_str()),
        ("x-codex-turn-state", "turn-state-ws-0142"),
        ("x-codex-beta-features", "goals,plugins"),
        ("x-oai-attestation", "attestation-ws-0142"),
        ("openai-beta", "responses_websockets=2026-02-06"),
        ("x-responsesapi-include-timing-metrics", "true"),
        ("x-openai-internal-codex-responses-lite", "true"),
        ("user-agent", "codex-cli/0.142.0"),
    ] {
        assert_eq!(
            headers.get(name).map(String::as_str),
            Some(expected),
            "backend websocket handshake header {name} should be forwarded"
        );
    }
    assert_eq!(
        headers.get("authorization").map(String::as_str),
        Some("Bearer test-token"),
        "proxy must replace caller websocket Authorization with selected profile auth"
    );
    assert_eq!(
        headers.get("chatgpt-account-id").map(String::as_str),
        Some("second-account"),
        "proxy must replace caller websocket ChatGPT account with selected profile account"
    );

    let websocket_requests = fixture.backend.websocket_requests();
    assert_eq!(
        websocket_requests.len(),
        1,
        "backend should receive exactly one websocket request"
    );
    let request = serde_json::from_str::<serde_json::Value>(&websocket_requests[0])
        .expect("websocket request should be JSON");
    assert_eq!(request["client_metadata"]["session_id"], "sess-ws-0142");
    assert_eq!(
        request["client_metadata"]["ws_request_header_x_openai_internal_codex_responses_lite"],
        "true"
    );
}

#[test]
fn runtime_proxy_websocket_does_not_synthesize_user_agent() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket(),
        "second",
        &["second"],
        &[],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [{
                "type": "message",
                "role": "user",
                "content": "hello"
            }],
        }),
    );

    let (_frames, completed_message) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
    });
    let _ = socket.close(None);

    assert!(
        completed_message.contains("\"response\":{\"id\":\"resp-second\"}"),
        "normal websocket response should complete: {completed_message}"
    );
    let responses_headers = fixture.backend.responses_headers();
    assert_eq!(
        responses_headers.len(),
        1,
        "backend should observe one websocket handshake: {responses_headers:?}"
    );
    assert!(
        !responses_headers[0].contains_key("user-agent"),
        "proxy should not synthesize websocket User-Agent; upstream Codex owns client/version headers"
    );
}

#[test]
fn runtime_proxy_websocket_empty_session_previous_response_does_not_fresh_fallback() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_previous_response_missing_without_turn_state(),
        "second",
        &["second"],
        &[("resp-second", "second")],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/codex/realtime?call_id=call-123");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "session_id": "sess-replayable",
            "input": [],
        }),
    );

    let response_message = read_runtime_websocket_text(&mut socket);
    let _ = socket.close(None);

    assert!(
        response_message.contains("\"code\":\"stale_continuation\""),
        "empty session-scoped previous_response continuation should fail stale instead of replaying fresh: {response_message}"
    );

    let websocket_requests = fixture.backend.websocket_requests();
    assert_eq!(
        websocket_requests.len(),
        1,
        "backend should observe only the original continuation"
    );

    let first_request = &websocket_requests[0];
    for (field, value) in [
        ("previous_response_id", "resp-second"),
        ("session_id", "sess-replayable"),
    ] {
        assert_request_json_field(
            first_request,
            field,
            value,
            "empty session-scoped continuation should preserve original context",
        );
    }

    let log = fixture.wait_for_log(|log| {
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity")
    });
    assert!(
        !log.contains("previous_response_fresh_fallback reason="),
        "empty session-scoped continuations must not drop previous_response_id: {log}"
    );
    assert!(
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity"),
        "runtime log should show guarded stale-continuation behavior: {log}"
    );
}

#[test]
fn runtime_proxy_websocket_tool_output_with_session_does_not_fresh_fallback() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_previous_response_missing_without_turn_state(),
        "second",
        &["second"],
        &[("resp-second", "second")],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/codex/realtime?call_id=call-123");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "session_id": "sess-replayable",
            "input": [{
                "type": "function_call_output",
                "call_id": "call_h7GvfUPAvb95drykPBrTw65i",
                "output": "ok"
            }],
        }),
    );

    let response_message = read_runtime_websocket_text(&mut socket);
    let _ = socket.close(None);

    assert!(
        response_message.contains("\"code\":\"stale_continuation\""),
        "tool-output continuation should fail as stale instead of replaying fresh: {response_message}"
    );
    assert!(
        !response_message.contains("No tool call found"),
        "proxy should not surface the fresh tool-output context error: {response_message}"
    );

    let websocket_requests = fixture.backend.websocket_requests();
    assert!(
        !websocket_requests.is_empty(),
        "backend should observe at least the original continuation"
    );
    for (field, value) in [
        ("previous_response_id", "resp-second"),
        ("session_id", "sess-replayable"),
        ("call_id", "call_h7GvfUPAvb95drykPBrTw65i"),
    ] {
        assert_all_requests_json_field(
            &websocket_requests,
            field,
            value,
            "backend should observe at least the original continuation",
            "websocket retries must preserve original continuation context",
        );
    }

    let log = fixture.wait_for_log(|log| {
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity")
    });
    assert!(
        log.contains("previous_response_not_found"),
        "runtime log should classify the broken continuation before surfacing stale: {log}"
    );
    assert!(
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity"),
        "tool outputs must stay chained instead of becoming fresh requests: {log}"
    );
    assert!(
        !log.contains(
            "previous_response_fresh_fallback reason=websocket_missing_turn_state_tool_result"
        ),
        "tool-output-only requests must not use proactive fresh replay: {log}"
    );
}

#[test]
fn runtime_proxy_websocket_tool_output_with_session_blocks_proactive_session_replay() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_owned_tool_output_needs_session_replay(),
        "second",
        &["second"],
        &[("resp-second", "second")],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "session_id": "sess-replayable",
            "input": [{
                "type": "function_call_output",
                "call_id": "call_J7U3Kdc539EyfWU4nZj9LCWQZ",
                "output": "ok"
            }],
        }),
    );

    let response_message = read_runtime_websocket_text(&mut socket);
    let _ = socket.close(None);

    assert!(
        response_message.contains("\"code\":\"stale_continuation\""),
        "tool-context failures should surface as stale continuation, not fresh replay: {response_message}"
    );
    assert!(
        !response_message.contains("No tool call found"),
        "proxy should translate upstream tool-context loss before it reaches Codex: {response_message}"
    );

    let websocket_requests = fixture.backend.websocket_requests();
    assert!(
        !websocket_requests.is_empty(),
        "backend should observe the guarded continuation"
    );
    for (field, value) in [
        ("previous_response_id", "resp-second"),
        ("session_id", "sess-replayable"),
    ] {
        assert_all_requests_json_field(
            &websocket_requests,
            field,
            value,
            "backend should observe the guarded continuation",
            "guarded attempts must preserve original continuation context",
        );
    }

    let log = fixture.wait_for_log(|log| {
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity")
    });
    assert!(
        !log.contains(
            "previous_response_fresh_fallback reason=websocket_missing_turn_state_tool_result"
        ),
        "runtime log must not show proactive fresh replay for tool outputs: {log}"
    );
    assert!(
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity"),
        "runtime log should show the guarded stale-continuation path: {log}"
    );
}

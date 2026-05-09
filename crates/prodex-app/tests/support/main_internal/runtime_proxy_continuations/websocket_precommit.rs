use super::*;

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

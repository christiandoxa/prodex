use super::*;

#[test]
fn runtime_proxy_websocket_invalid_previous_response_triggers_codex_full_context_replay() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_invalid_previous_response_id(),
        "second",
        &["second", "main"],
        &[],
        Vec::new(),
    );
    let headers = [
        runtime_continuation_header("user-agent", "codex-tui/0.148.0 (Linux; x86_64)"),
        runtime_continuation_header("session_id", "session-chain-owner"),
    ];
    let mut socket =
        fixture.connect_websocket_with_headers("backend-api/prodex/responses", &headers);

    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({"input": [{"type": "message", "role": "user", "content": "turn one"}]}),
    );
    let (_, first) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("response.completed")
    });
    assert!(first.contains("resp-second"), "{first}");

    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "input": [{"type": "message", "role": "user", "content": "turn two"}],
        }),
    );
    let (_, invalid) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("Invalid `previous_response_id`")
    });
    assert!(invalid.contains("invalid_request_error"), "{invalid}");
    assert!(
        invalid.contains("previous_response_not_found"),
        "Codex 0.148 needs the retryable code to replay full context: {invalid}"
    );
    let _ = socket.close(None);

    let mut socket =
        fixture.connect_websocket_with_headers("backend-api/prodex/responses", &headers);
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [
                {"type": "message", "role": "user", "content": "turn one"},
                {"type": "message", "role": "assistant", "content": "turn one result"},
                {"type": "message", "role": "user", "content": "turn two"}
            ],
        }),
    );
    let (_, recovered) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("response.completed")
    });
    let _ = socket.close(None);

    assert!(recovered.contains("resp-second"), "{recovered}");
    let requests = fixture.backend.websocket_requests();
    assert_eq!(
        requests.len(),
        3,
        "one stale snapshot must be followed by one full replay: {requests:?}"
    );
    assert!(requests[1].contains("previous_response_id"), "{requests:?}");
    assert!(!requests[2].contains("previous_response_id"), "{requests:?}");
    assert!(requests[2].contains("turn one result"));
    assert_eq!(requests[2].matches("turn two").count(), 1);
    let log = fixture.wait_for_log(|log| log.contains("codex_full_context_retry_signal"));
    assert!(log.contains("compatibility_gate=true"), "{log}");
    assert!(
        log.contains("transport_generation=2"),
        "the stale request must be observed after Prodex reconnects upstream: {log}"
    );
}

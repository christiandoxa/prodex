use super::*;

#[test]
fn runtime_proxy_websocket_owner_retry_survives_expired_budget_after_reuse_watchdog() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let _connect_timeout_guard = ci_runtime_proxy_timeout_guard(
        "PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS",
        250,
        1_000,
    );
    let _progress_timeout_guard = ci_runtime_proxy_timeout_guard(
        "PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS",
        12_100,
        15_000,
    );
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_reuse_previous_response_needs_turn_state(),
        "second",
        &["second"],
        &[],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
    set_test_websocket_io_timeout(&mut socket, ci_timing_upper_bound_ms(15_000, 25_000));

    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [{
                "type": "message",
                "role": "user",
                "content": "establish a synthetic continuation"
            }],
        }),
    );
    let (_, initial_completed) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
    });
    assert!(
        initial_completed.contains("\"response\":{\"id\":\"resp-second\"}"),
        "initial websocket response should establish owner affinity: {initial_completed}"
    );

    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "session_id": "sess-owner-retry",
            "input": [{
                "type": "function_call_output",
                "call_id": "call-owner-retry",
                "output": "synthetic tool result"
            }],
        }),
    );
    let (frames, recovered) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
            || text.contains("service_unavailable")
            || text.contains("stale_continuation")
    });
    let _ = socket.close(None);

    assert!(
        recovered.contains("\"response\":{\"id\":\"resp-second-next\"}"),
        "owner retry should complete after watchdog and turn-state recovery: {frames:?}"
    );
    assert!(
        frames.iter().all(|frame| {
            !frame.contains("service_unavailable") && !frame.contains("stale_continuation")
        }),
        "bounded owner recovery must not leak a local terminal failure: {frames:?}"
    );
    assert_eq!(
        fixture.backend.websocket_requests().len(),
        4,
        "recovery should use the reused attempt, one fresh turn-state discovery, and one owner retry"
    );

    let log = fixture.wait_for_log(|log| {
        log.contains("websocket_reuse_watchdog_timeout")
            && log.contains("previous_response_not_found")
            && log.contains("chain_retried_owner")
            && log.contains("transport=websocket committed profile=second")
    });
    assert!(
        !log.contains("precommit_budget_exhausted"),
        "a scheduled owner retry must run once after the elapsed watchdog budget: {log}"
    );
}

#[test]
fn runtime_proxy_response_owner_survives_http_websocket_transitions() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket(),
        "second",
        &["second", "main"],
        &[],
        Vec::new(),
    );

    let first = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "http one"}],
            "client_metadata": {"session_id": "cross-transport"}
        }),
    );
    assert!(first.text().expect("HTTP body").contains("resp-second"));

    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "session_id": "cross-transport",
            "input": [{"type": "message", "role": "user", "content": "websocket two"}]
        }),
    );
    let (_, websocket_response) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("response.completed")
    });
    let _ = socket.close(None);
    assert!(websocket_response.contains("resp-second-next"));

    let third = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "previous_response_id": "resp-second-next",
            "input": [{"type": "message", "role": "user", "content": "http three"}],
            "client_metadata": {"session_id": "cross-transport"}
        }),
    );
    assert!(
        third
            .text()
            .expect("HTTP continuation body")
            .contains("resp-second-next-next")
    );
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec![
            "second-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string()
        ]
    );
}

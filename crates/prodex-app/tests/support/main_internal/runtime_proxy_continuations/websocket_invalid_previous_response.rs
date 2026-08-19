use super::*;

#[test]
fn runtime_proxy_websocket_invalid_previous_response_is_forwarded_once_and_allows_fresh_reconnect() {
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
    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");

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
    let _ = socket.close(None);

    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "reconnect without stale id"}],
        }),
    );
    let (_, recovered) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("response.completed")
    });
    let _ = socket.close(None);

    assert!(recovered.contains("resp-second"), "{recovered}");
    let requests = fixture.backend.websocket_requests();
    assert_eq!(requests.len(), 3, "the reconnect must send one fresh full request");
    assert!(requests[1].contains("previous_response_id"));
    assert!(!requests[2].contains("previous_response_id"));
    let log = fixture.wait_for_log(|log| {
        log.contains("transport=websocket") && log.contains("upstream_rejected")
    });
    assert!(log.contains("upstream_rejected"), "{log}");
}

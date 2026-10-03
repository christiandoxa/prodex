use super::*;

#[test]
fn runtime_proxy_websocket_fresh_overload_rotates_without_leaking_retryable_error() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_overloaded(),
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
                "content": "continue through temporary provider overload"
            }],
        }),
    );

    let (frames, completed) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("\"type\":\"response.completed\"")
    });
    let _ = socket.close(None);

    assert!(
        completed.contains("\"response\":{\"id\":\"resp-second\"}"),
        "fresh websocket overload should rotate to the healthy profile: {completed}"
    );
    assert!(
        frames
            .iter()
            .all(|frame| !frame.contains("server_is_overloaded")),
        "retryable websocket overload must remain internal after successful rotation: {frames:?}"
    );
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()],
        "websocket should rotate from the temporarily overloaded profile"
    );
}

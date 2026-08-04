use super::*;

#[test]
fn websocket_precommit_turn_state_is_attempt_local_until_commit() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("upstream websocket listener should bind");
    let upstream_addr = listener
        .local_addr()
        .expect("upstream websocket listener should expose address");
    let upstream = thread::spawn(move || {
        let (stream, _) = listener
            .accept()
            .expect("upstream websocket should accept connection");
        let mut socket = tungstenite::accept(stream).expect("upstream websocket handshake");
        let _request = socket
            .read()
            .expect("upstream websocket should receive request");
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.failed","response":{"id":"resp-missing","error":{"code":"previous_response_not_found","message":"missing"}},"turn_state":"turn-attempt"}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send previous_response_not_found");
    });

    let shared = websocket_test_shared_with_main_profile("turn-state-attempt-local", upstream_addr);
    let (mut local_socket, _client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 42,
        local_socket: &mut local_socket,
        handshake_request: &handshake_request,
        request_text: r#"{"type":"response.create"}"#,
        request_previous_response_id: None,
        request_prompt_cache_key: None,
        request_session_id: None,
        request_turn_state: None,
        shared: &shared,
        websocket_session: &mut websocket_session,
        profile_name: "main",
        turn_state_override: None,
        promote_committed_profile: true,
    })
    .expect("previous_response_not_found should remain retryable before commit");

    assert!(matches!(
        attempt,
        RuntimeWebsocketAttempt::PreviousResponseNotFound {
            turn_state: Some(turn_state),
            payload: RuntimeWebsocketErrorPayload::Text(payload),
            ..
        } if turn_state == "turn-attempt" && payload.contains("previous_response_not_found")
    ));
    assert!(
        shared
            .runtime
            .lock()
            .expect("runtime state lock should be available")
            .turn_state_bindings
            .is_empty(),
        "precommit turn state must not be persisted"
    );
    upstream
        .join()
        .expect("upstream websocket thread should finish");
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_hard_affinity_precommit_hold_limit_fails_closed_without_forwarding() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("upstream websocket listener should bind");
    let upstream_addr = listener
        .local_addr()
        .expect("upstream websocket listener should expose address");
    let upstream = thread::spawn(move || {
        let (stream, _) = listener
            .accept()
            .expect("upstream websocket should accept connection");
        let mut socket = tungstenite::accept(stream).expect("upstream websocket handshake");
        let _request = socket
            .read()
            .expect("upstream websocket should receive request");
        let _ = socket.send(WsMessage::Text(
            serde_json::json!({
                "type": "response.in_progress",
                "padding": "x".repeat(
                    RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_HARD_AFFINITY_MAX_BYTES
                ),
            })
            .to_string()
            .into(),
        ));
    });

    let shared =
        websocket_test_shared_with_main_profile("precommit-hold-hard-limit", upstream_addr);
    let (mut local_socket, mut client_socket) = websocket_test_local_pair();
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = client_socket.get_mut() {
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("client websocket should set a read timeout");
    }
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let error = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 43,
        local_socket: &mut local_socket,
        handshake_request: &handshake_request,
        request_text: r#"{"type":"response.create","previous_response_id":"resp-owner"}"#,
        request_previous_response_id: Some("resp-owner"),
        request_prompt_cache_key: None,
        request_session_id: None,
        request_turn_state: None,
        shared: &shared,
        websocket_session: &mut websocket_session,
        profile_name: "main",
        turn_state_override: None,
        promote_committed_profile: false,
    })
    .expect_err("ambiguous hard-affinity response should fail closed");
    assert!(error.to_string().contains("bounded hard-affinity limit"));
    assert!(
        client_socket.read().is_err(),
        "ambiguous frame must not be forwarded"
    );
    assert_eq!(
        shared
            .runtime
            .lock()
            .expect("runtime state lock should be available")
            .current_profile,
        "main"
    );
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(
        &shared.log_path,
        "websocket_precommit_hold_limit_exceeded",
    );
    assert!(log.contains("request=43") && log.contains("action=fail_closed"));
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_hard_affinity_accepts_large_rate_limit_metadata_before_completion() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("upstream websocket listener should bind");
    let upstream_addr = listener
        .local_addr()
        .expect("upstream websocket listener should expose address");
    let upstream = thread::spawn(move || {
        let (stream, _) = listener
            .accept()
            .expect("upstream websocket should accept connection");
        let mut socket = tungstenite::accept(stream).expect("upstream websocket handshake");
        let _request = socket
            .read()
            .expect("upstream websocket should receive request");
        socket
            .send(WsMessage::Text(
                serde_json::json!({
                    "type": "codex.rate_limits",
                    "padding": "x".repeat(32 * 1024),
                })
                .to_string()
                .into(),
            ))
            .expect("upstream should send rate-limit metadata");
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-next"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should complete response");
    });

    let shared =
        websocket_test_shared_with_main_profile("large-rate-limit-metadata", upstream_addr);
    let (mut local_socket, mut client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 44,
        local_socket: &mut local_socket,
        handshake_request: &handshake_request,
        request_text: r#"{"type":"response.create","previous_response_id":"resp-owner"}"#,
        request_previous_response_id: Some("resp-owner"),
        request_prompt_cache_key: None,
        request_session_id: None,
        request_turn_state: None,
        shared: &shared,
        websocket_session: &mut websocket_session,
        profile_name: "main",
        turn_state_override: None,
        promote_committed_profile: false,
    })
    .expect("large metadata should remain transparent");

    assert!(matches!(attempt, RuntimeWebsocketAttempt::Delivered));
    assert!(
        client_socket
            .read()
            .expect("client should receive metadata")
            .into_text()
            .expect("metadata should be text")
            .contains("codex.rate_limits")
    );
    assert!(
        client_socket
            .read()
            .expect("client should receive completion")
            .into_text()
            .expect("completion should be text")
            .contains("response.completed")
    );
    upstream
        .join()
        .expect("upstream websocket thread should finish");
    let log = read_websocket_test_log_after_marker(&shared.log_path, "precommit_hold");
    assert!(!log.contains("websocket_precommit_hold_limit_exceeded"));
    let _ = std::fs::remove_file(&shared.log_path);
}

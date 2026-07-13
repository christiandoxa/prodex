use super::*;

#[test]
fn websocket_precommit_hold_commits_when_lookahead_budget_is_exhausted() {
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
                    "type": "response.in_progress",
                    "padding": "x".repeat(RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES),
                })
                .to_string()
                .into(),
            ))
            .expect("upstream should send oversized precommit metadata");
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-budget"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send response.completed");
    });

    let shared = websocket_test_shared_with_main_profile("precommit-hold-budget", upstream_addr);
    let (mut local_socket, mut client_socket) = websocket_test_local_pair();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let attempt_shared = shared.clone();
    let attempt = thread::spawn(move || {
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
            request_id: 41,
            local_socket: &mut local_socket,
            handshake_request: &handshake_request,
            request_text: r#"{"type":"response.create"}"#,
            request_previous_response_id: None,
            request_prompt_cache_key: None,
            request_session_id: None,
            request_turn_state: None,
            shared: &attempt_shared,
            websocket_session: &mut websocket_session,
            profile_name: "main",
            turn_state_override: None,
            promote_committed_profile: true,
        })
    });

    let in_progress = client_socket
        .read()
        .expect("client should receive precommit metadata after the byte budget");
    let completed = client_socket
        .read()
        .expect("client should receive response.completed");
    assert!(in_progress.to_string().contains("response.in_progress"));
    assert!(completed.to_string().contains("response.completed"));
    assert!(matches!(
        attempt
            .join()
            .expect("websocket attempt thread should finish")
            .expect("websocket attempt should succeed"),
        RuntimeWebsocketAttempt::Delivered
    ));
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log =
        read_websocket_test_log_after_marker(&shared.log_path, "event=lookahead_budget_exhausted");
    assert!(
        log.contains("websocket_precommit_hold_promoted")
            && log.contains("event=lookahead_budget_exhausted")
            && log.contains("request=41 transport=websocket committed profile=main"),
        "the byte budget should commit and release buffered frames: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_precommit_hold_response_created_commits_at_terminal_event() {
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
                r#"{"type":"response.created","response":{"id":"resp-hold"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send response.created");
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-hold"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send response.completed");
    });

    let shared = websocket_test_shared_with_main_profile("precommit-hold-promoted", upstream_addr);

    let (mut local_socket, mut client_socket) = websocket_test_local_pair();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let attempt_shared = shared.clone();
    let attempt = thread::spawn(move || {
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
            request_id: 31,
            local_socket: &mut local_socket,
            handshake_request: &handshake_request,
            request_text: r#"{"type":"response.create"}"#,
            request_previous_response_id: None,
            request_prompt_cache_key: None,
            request_session_id: None,
            request_turn_state: None,
            shared: &attempt_shared,
            websocket_session: &mut websocket_session,
            profile_name: "main",
            turn_state_override: None,
            promote_committed_profile: true,
        })
    });

    let created = client_socket
        .read()
        .expect("client should receive promoted response.created before terminal event");
    let completed = client_socket
        .read()
        .expect("client should receive response.completed");
    let attempt = attempt
        .join()
        .expect("websocket attempt thread should finish")
        .expect("websocket attempt should not fail after response.created promotion");
    assert!(matches!(attempt, RuntimeWebsocketAttempt::Delivered));
    assert!(
        created.to_string().contains("response.created"),
        "first frame should be response.created: {created:?}"
    );
    assert!(
        completed.to_string().contains("response.completed"),
        "second frame should be response.completed: {completed:?}"
    );
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(
        &shared.log_path,
        "request=31 transport=websocket committed profile=main",
    );
    assert!(
        log.contains("request=31 transport=websocket committed profile=main")
            && !log.contains("websocket_precommit_hold_promoted")
            && !log.contains("websocket_precommit_hold_timeout"),
        "response.created should stay buffered until terminal commit: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_response_incomplete_is_terminal() {
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
                r#"{"type":"response.incomplete","response":{"id":"resp-incomplete","incomplete_details":{"reason":"max_output_tokens"}}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send response.incomplete");
    });

    let shared = websocket_test_shared_with_main_profile("response-incomplete", upstream_addr);
    let (mut local_socket, mut client_socket) = websocket_test_local_pair();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 38,
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
    .expect("response.incomplete should finish the websocket attempt");
    let incomplete = client_socket
        .read()
        .expect("client should receive response.incomplete");
    assert!(matches!(attempt, RuntimeWebsocketAttempt::Delivered));
    assert!(incomplete.to_string().contains("response.incomplete"));
    assert!(
        !websocket_session.can_reuse("main", None),
        "Responses error terminals should drop the upstream websocket like upstream Codex"
    );
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log =
        read_websocket_test_log_after_marker(&shared.log_path, "event_type=response.incomplete");
    assert!(
        log.contains("request=38 transport=websocket terminal_event profile=main event_type=response.incomplete"),
        "response.incomplete must release websocket attempt as terminal: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_connection_limit_is_precommit_reuse_retry() {
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
                r#"{"type":"error","error":{"code":"websocket_connection_limit_reached","message":"Responses websocket connection limit reached (60 minutes). Create a new websocket connection to continue."}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send connection-limit error");
    });

    let shared = websocket_test_shared_with_main_profile("connection-limit", upstream_addr);
    let (mut local_socket, _client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 39,
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
    .expect("connection-limit error should be handled before commit");

    assert!(matches!(
        attempt,
        RuntimeWebsocketAttempt::ReuseWatchdogTripped {
            profile_name,
            event: "connection_limit_reached",
        } if profile_name == "main"
    ));
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(&shared.log_path, "connection_limit_reached");
    assert!(
        log.contains("request=39") && !log.contains("transport=websocket committed profile=main"),
        "connection limit must not commit raw error to Codex: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_wrapped_status_error_is_forwarded_as_terminal_without_reuse() {
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
                r#"{"type":"error","status_code":400,"error":{"type":"invalid_request_error","message":"Model does not support image inputs"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send wrapped status error");
    });

    let shared = websocket_test_shared_with_main_profile("wrapped-status-error", upstream_addr);
    let (mut local_socket, mut client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 40,
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
    .expect("wrapped status error should finish after forwarding");

    let error = client_socket
        .read()
        .expect("client should receive wrapped status error");
    assert!(matches!(attempt, RuntimeWebsocketAttempt::Delivered));
    assert!(error.to_string().contains("invalid_request_error"));
    assert!(
        !websocket_session.can_reuse("main", None),
        "wrapped status errors should drop the upstream websocket like upstream Codex"
    );
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(&shared.log_path, "event_type=error");
    assert!(
        log.contains("request=40 transport=websocket terminal_event profile=main event_type=error"),
        "wrapped status error should be terminal after forwarding: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_response_failed_error_is_forwarded_as_terminal_without_reuse() {
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
                r#"{"type":"response.failed","response":{"id":"resp-failed","error":{"code":"invalid_prompt","message":"Invalid request."}}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send response.failed");
    });

    let shared = websocket_test_shared_with_main_profile("response-failed-terminal", upstream_addr);
    let (mut local_socket, mut client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 41,
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
    .expect("response.failed should finish after forwarding");

    let failed = client_socket
        .read()
        .expect("client should receive response.failed");
    assert!(matches!(attempt, RuntimeWebsocketAttempt::Delivered));
    assert!(failed.to_string().contains("response.failed"));
    assert!(
        !websocket_session.can_reuse("main", None),
        "response.failed should drop the upstream websocket like upstream Codex"
    );
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(&shared.log_path, "event_type=response.failed");
    assert!(
        log.contains(
            "request=41 transport=websocket terminal_event profile=main event_type=response.failed"
        ),
        "response.failed should be terminal after forwarding: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_rate_limits_before_quota_stays_precommit_retryable() {
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
                r#"{"type":"codex.rate_limits","primary":{"used_percent":99}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send rate limits");
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.failed","status":429,"response":{"error":{"code":"insufficient_quota","message":"quota exhausted"}}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send quota failure");
    });

    let shared = websocket_test_shared_with_main_profile("rate-limits-before-quota", upstream_addr);
    let (mut local_socket, _client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 40,
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
    .expect("quota after rate-limit metadata should be retryable before commit");

    assert!(matches!(
        attempt,
        RuntimeWebsocketAttempt::QuotaBlocked {
            profile_name,
            ..
        } if profile_name == "main"
    ));
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(&shared.log_path, "precommit_hold");
    assert!(
        log.contains("request=40")
            && log.contains("event_type=codex.rate_limits")
            && !log.contains("request=40 transport=websocket committed profile=main"),
        "rate-limit metadata must not commit before retryable quota: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_quota_after_response_created_stays_precommit_retryable() {
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
                    "type": "response.created",
                    "response": {"id": "resp-quota-hold"},
                    "padding": "x".repeat(RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES),
                })
                .to_string()
                .into(),
            ))
            .expect("upstream should send response.created");
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.failed","status":429,"error":{"type":"usage_limit_reached","message":"You've hit your usage limit. Upgrade to Pro or try again later."}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send quota failure");
    });

    let shared = websocket_test_shared_with_main_profile("precommit-hold-quota", upstream_addr);
    let (mut local_socket, _client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 37,
        local_socket: &mut local_socket,
        handshake_request: &handshake_request,
        request_text: r#"{"type":"response.create"}"#,
        request_previous_response_id: None,
        request_prompt_cache_key: None,
        request_session_id: Some("session-quota-hold"),
        request_turn_state: None,
        shared: &shared,
        websocket_session: &mut websocket_session,
        profile_name: "main",
        turn_state_override: None,
        promote_committed_profile: true,
    })
    .expect("quota after response.created should be retryable before commit");

    assert!(matches!(
        attempt,
        RuntimeWebsocketAttempt::QuotaBlocked {
            profile_name,
            ..
        } if profile_name == "main"
    ));
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(&shared.log_path, "precommit_hold");
    assert!(
        log.contains("request=37")
            && log.contains("precommit_hold")
            && !log.contains("request=37 transport=websocket committed profile=main")
            && !log.contains("websocket_precommit_hold_promoted"),
        "quota after response.created must stay precommit-retryable: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_precommit_hold_timeout_does_not_promote_without_response_id_signal() {
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
                r#"{"type":"response.in_progress"}"#.to_string().into(),
            ))
            .expect("upstream should send response.in_progress");
        thread::sleep(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms() + 40,
        ));
    });

    let shared = websocket_test_shared_with_main_profile("precommit-hold-no-id", upstream_addr);
    let (mut local_socket, _client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 35,
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
    .expect("hold timeout without response id signal should be retryable before commit");

    assert!(matches!(
        attempt,
        RuntimeWebsocketAttempt::TransportFailed {
            profile_name,
            stage: "read_error",
        } if profile_name == "main"
    ));
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(&shared.log_path, "upstream_read_error");
    assert!(
        log.contains("upstream_read_error")
            && log.contains("request=35")
            && !log.contains("websocket_precommit_hold_promoted")
            && !log.contains("transport=websocket committed profile=main"),
        "hold timeout without response id signal must not commit/promote: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_precommit_hold_timeout_does_not_promote_created_frame_without_usable_response_id() {
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
                r#"{"type":"response.created","response":{}}"#.to_string().into(),
            ))
            .expect("upstream should send response.created without id");
        thread::sleep(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms() + 40,
        ));
    });

    let shared =
        websocket_test_shared_with_main_profile("precommit-hold-created-without-id", upstream_addr);
    let (mut local_socket, _client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 36,
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
    .expect("created hold timeout without usable response id should be retryable before commit");

    assert!(matches!(
        attempt,
        RuntimeWebsocketAttempt::TransportFailed {
            profile_name,
            stage: "read_error",
        } if profile_name == "main"
    ));
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(&shared.log_path, "upstream_read_error");
    assert!(
        log.contains("upstream_read_error")
            && log.contains("request=36")
            && !log.contains("websocket_precommit_hold_promoted")
            && !log.contains("transport=websocket committed profile=main"),
        "created hold timeout without usable response id must not commit/promote: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_precommit_hold_response_created_commits_previous_response_affinity() {
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
                r#"{"type":"response.created","response":{"id":"resp-affinity-hold"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send response.created");
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-affinity-hold"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send response.completed");
    });

    let shared = websocket_test_shared_with_main_profile("precommit-hold-affinity", upstream_addr);
    let (mut local_socket, mut client_socket) = websocket_test_local_pair();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let attempt_shared = shared.clone();
    let attempt = thread::spawn(move || {
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
            request_id: 32,
            local_socket: &mut local_socket,
            handshake_request: &handshake_request,
            request_text: r#"{"type":"response.create","previous_response_id":"resp-owner"}"#,
            request_previous_response_id: Some("resp-owner"),
            request_prompt_cache_key: None,
            request_session_id: None,
            request_turn_state: None,
            shared: &attempt_shared,
            websocket_session: &mut websocket_session,
            profile_name: "main",
            turn_state_override: None,
            promote_committed_profile: false,
        })
    });

    let created = client_socket
        .read()
        .expect("client should receive hard-affinity response.created");
    let completed = client_socket
        .read()
        .expect("client should receive hard-affinity response.completed");
    let attempt = attempt
        .join()
        .expect("websocket attempt thread should finish")
        .expect("hard-affinity response.created should commit without timeout");
    assert!(matches!(attempt, RuntimeWebsocketAttempt::Delivered));
    assert!(created.to_string().contains("response.created"));
    assert!(completed.to_string().contains("response.completed"));
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(
        &shared.log_path,
        "request=32 transport=websocket committed profile=main",
    );
    assert!(
        log.contains("request=32")
            && log.contains("transport=websocket committed profile=main")
            && !log.contains("websocket_precommit_hold_promoted")
            && !log.contains("websocket_precommit_hold_timeout"),
        "hard-affinity response.created should stay buffered until terminal commit: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_rate_limits_keeps_reused_session_alive_past_progress_timeout() {
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
        let _first_request = socket
            .read()
            .expect("upstream websocket should receive first request");
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.created","response":{"id":"resp-first"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send first response.created");
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-first"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send first response.completed");
        let _second_request = socket
            .read()
            .expect("upstream websocket should receive reused-session request");
        socket
            .send(WsMessage::Text(r#"{"type":"codex.rate_limits"}"#.into()))
            .expect("upstream should send reused codex.rate_limits");
        thread::sleep(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms() + 40,
        ));
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-reuse-hold"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send reused response.completed");
    });

    let shared = websocket_test_shared_with_main_profile("precommit-hold-reuse", upstream_addr);
    let (mut local_socket, mut client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let first_attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 33,
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
    .expect("first websocket attempt should complete");
    assert!(matches!(first_attempt, RuntimeWebsocketAttempt::Delivered));
    let _created = client_socket
        .read()
        .expect("client should receive first response.created");
    let _completed = client_socket
        .read()
        .expect("client should receive first response.completed");

    let reused_shared = shared.clone();
    let reused_attempt = thread::spawn(move || {
        attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
            request_id: 34,
            local_socket: &mut local_socket,
            handshake_request: &handshake_request,
            request_text: r#"{"type":"response.create"}"#,
            request_previous_response_id: None,
            request_prompt_cache_key: None,
            request_session_id: None,
            request_turn_state: None,
            shared: &reused_shared,
            websocket_session: &mut websocket_session,
            profile_name: "main",
            turn_state_override: None,
            promote_committed_profile: true,
        })
    });
    let reused_rate_limits = client_socket
        .read()
        .expect("client should receive reused codex.rate_limits");
    let reused_completed = client_socket
        .read()
        .expect("client should receive reused response.completed");
    let reused_attempt = reused_attempt
        .join()
        .expect("reused websocket attempt thread should finish")
        .expect("reused websocket rate limits should keep waiting for model output");
    assert!(matches!(reused_attempt, RuntimeWebsocketAttempt::Delivered));
    assert!(reused_rate_limits.to_string().contains("codex.rate_limits"));
    assert!(reused_completed.to_string().contains("response.completed"));
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(
        &shared.log_path,
        "request=34 transport=websocket committed profile=main",
    );
    assert!(
        log.contains("request=34")
            && log.contains("request=34 transport=websocket committed profile=main")
            && !log.contains("websocket_precommit_hold_promoted")
            && !log.contains("websocket_precommit_hold_timeout"),
        "reused-session rate limits should keep the upstream alive past the progress timeout: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

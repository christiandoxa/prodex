use super::*;

#[test]
fn websocket_read_poll_timeout_preserves_write_timeout() {
    let (_local_socket, mut upstream_socket) = websocket_test_local_pair();
    let write_timeout = Duration::from_secs(3);
    let read_timeout = Duration::from_millis(20);

    runtime_set_upstream_websocket_io_timeout(&mut upstream_socket, Some(write_timeout)).unwrap();
    runtime_set_upstream_websocket_read_timeout(&mut upstream_socket, Some(read_timeout)).unwrap();

    let MaybeTlsStream::Plain(stream) = upstream_socket.get_ref() else {
        panic!("test websocket should use a plain local TCP stream");
    };
    assert_eq!(stream.read_timeout().unwrap(), Some(read_timeout));
    assert_eq!(stream.write_timeout().unwrap(), Some(write_timeout));
}

#[test]
fn websocket_upstream_handshake_rejection_preserves_status() {
    let _guard = acquire_test_runtime_lock();
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("upstream listener should bind");
    let upstream_addr = listener
        .local_addr()
        .expect("upstream address should resolve");
    let upstream = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("upstream should accept connection");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 512];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).expect("handshake should read");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
        }
        let body = r#"{"error":{"code":"invalid_request_error","message":"unsupported beta"}}"#;
        write!(
            stream,
            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("rejection should write");
    });
    let shared = websocket_test_shared_with_main_profile("handshake-rejected", upstream_addr);
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let result =
        connect_runtime_proxy_upstream_websocket(36, &handshake_request, &shared, "main", None)
            .expect("upstream HTTP rejection should remain a proxy result");

    assert!(
        matches!(
            &result,
            RuntimeWebsocketConnectResult::Rejected(RuntimeWebsocketErrorPayload::Text(body))
                if body.contains("upstream_rejected") && body.contains("\"status\":400")
        ),
        "unexpected rejection: {result:?}"
    );
    upstream.join().expect("upstream thread should finish");
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_fresh_connect_failure_returns_retryable_transport_attempt() {
    let _guard = acquire_test_runtime_lock();
    let _env_lock = TestEnvVarGuard::lock();
    let _http_proxy = TestEnvVarGuard::unset("HTTP_PROXY");
    let _http_proxy_lower = TestEnvVarGuard::unset("http_proxy");
    let _https_proxy = TestEnvVarGuard::unset("HTTPS_PROXY");
    let _https_proxy_lower = TestEnvVarGuard::unset("https_proxy");
    let _all_proxy = TestEnvVarGuard::unset("ALL_PROXY");
    let _all_proxy_lower = TestEnvVarGuard::unset("all_proxy");
    let _proxy = TestEnvVarGuard::unset("PROXY");
    let _proxy_lower = TestEnvVarGuard::unset("proxy");
    let upstream_addr = websocket_unused_local_addr();
    let shared = websocket_test_shared_with_main_profile("fresh-connect-failed", upstream_addr);
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
        request_text: r#"{"type":"response.create","session_id":"sess-fresh"}"#,
        request_previous_response_id: None,
        request_prompt_cache_key: None,
        request_session_id: Some("sess-fresh"),
        request_turn_state: None,
        shared: &shared,
        websocket_session: &mut websocket_session,
        profile_name: "main",
        turn_state_override: None,
        promote_committed_profile: true,
    })
    .expect("fresh precommit connect failure should be retryable");

    assert!(matches!(
        attempt,
        RuntimeWebsocketAttempt::TransportFailed {
            profile_name,
            stage: "connect",
        } if profile_name == "main"
    ));
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_hard_affinity_connect_failure_does_not_return_retryable_transport_attempt() {
    let _guard = acquire_test_runtime_lock();
    let _env_lock = TestEnvVarGuard::lock();
    let _http_proxy = TestEnvVarGuard::unset("HTTP_PROXY");
    let _http_proxy_lower = TestEnvVarGuard::unset("http_proxy");
    let _https_proxy = TestEnvVarGuard::unset("HTTPS_PROXY");
    let _https_proxy_lower = TestEnvVarGuard::unset("https_proxy");
    let _all_proxy = TestEnvVarGuard::unset("ALL_PROXY");
    let _all_proxy_lower = TestEnvVarGuard::unset("all_proxy");
    let _proxy = TestEnvVarGuard::unset("PROXY");
    let _proxy_lower = TestEnvVarGuard::unset("proxy");
    let upstream_addr = websocket_unused_local_addr();
    let shared =
        websocket_test_shared_with_main_profile("hard-affinity-connect-failed", upstream_addr);
    let (mut local_socket, _client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let err = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 38,
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
    .expect_err("hard-affinity connect failure should not be retryable");

    assert!(
        err.to_string()
            .contains("failed to connect runtime websocket upstream"),
        "unexpected hard-affinity connect error: {err:?}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_timed_out_handshake_returns_transport_attempt_instead_of_panicking() {
    let _guard = acquire_test_runtime_lock();
    let _env_lock = TestEnvVarGuard::lock();
    let _http_proxy = TestEnvVarGuard::unset("HTTP_PROXY");
    let _http_proxy_lower = TestEnvVarGuard::unset("http_proxy");
    let _https_proxy = TestEnvVarGuard::unset("HTTPS_PROXY");
    let _https_proxy_lower = TestEnvVarGuard::unset("https_proxy");
    let _all_proxy = TestEnvVarGuard::unset("ALL_PROXY");
    let _all_proxy_lower = TestEnvVarGuard::unset("all_proxy");
    let _proxy = TestEnvVarGuard::unset("PROXY");
    let _proxy_lower = TestEnvVarGuard::unset("proxy");

    let upstream =
        std::net::TcpListener::bind("127.0.0.1:0").expect("stalled upstream listener should bind");
    let upstream_addr = upstream
        .local_addr()
        .expect("stalled upstream listener should expose addr");
    let upstream_thread = thread::spawn(move || {
        let (_stream, _) = upstream
            .accept()
            .expect("stalled upstream should accept websocket TCP connection");
        thread::sleep(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms() + 80,
        ));
    });

    let shared = websocket_test_shared_with_main_profile("interrupted-handshake", upstream_addr);
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
        request_text: r#"{"type":"response.create","session_id":"sess-stalled"}"#,
        request_previous_response_id: None,
        request_prompt_cache_key: None,
        request_session_id: Some("sess-stalled"),
        request_turn_state: None,
        shared: &shared,
        websocket_session: &mut websocket_session,
        profile_name: "main",
        turn_state_override: None,
        promote_committed_profile: true,
    })
    .expect("fresh interrupted handshake should be retryable");

    assert!(matches!(
        attempt,
        RuntimeWebsocketAttempt::TransportFailed {
            profile_name,
            stage: "connect",
        } if profile_name == "main"
    ));
    upstream_thread
        .join()
        .expect("stalled upstream thread should finish");
    let log = read_websocket_test_log_after_marker(&shared.log_path, "upstream_connect_failure");
    assert!(
        log.contains("upstream_connect_timeout")
            && log.contains("class=connect_timeout")
            && log.contains(
                "profile_transport_failure profile=main route=websocket class=connect_timeout"
            ),
        "timed-out handshake should be logged as connect timeout: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_upstream_connect_uses_http_connect_proxy_from_env() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("test proxy should bind");
    let proxy_addr = listener
        .local_addr()
        .expect("test proxy should expose local addr");
    let (request_tx, request_rx) = mpsc::channel::<String>();
    let proxy_thread = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("proxy should accept CONNECT");
        let mut request = Vec::new();
        let mut buffer = [0u8; 256];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).expect("proxy should read request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
        }
        request_tx
            .send(String::from_utf8_lossy(&request).into_owned())
            .expect("CONNECT request should be recorded");
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .expect("proxy should accept tunnel");
        thread::sleep(Duration::from_millis(50));
    });

    let _env_lock = TestEnvVarGuard::lock();
    let _http_proxy = TestEnvVarGuard::unset("HTTP_PROXY");
    let _http_proxy_lower = TestEnvVarGuard::unset("http_proxy");
    let _https_proxy = TestEnvVarGuard::unset("HTTPS_PROXY");
    let _all_proxy = TestEnvVarGuard::unset("ALL_PROXY");
    let _all_proxy_lower = TestEnvVarGuard::unset("all_proxy");
    let _proxy = TestEnvVarGuard::unset("PROXY");
    let _proxy_lower = TestEnvVarGuard::unset("proxy");
    let _no_proxy = TestEnvVarGuard::unset("NO_PROXY");
    let _no_proxy_lower = TestEnvVarGuard::unset("no_proxy");
    let proxy_url = format!("http://user:pass@{proxy_addr}");
    let _https_proxy_lower = TestEnvVarGuard::set("https_proxy", &proxy_url);

    let shared = websocket_test_shared("http-connect-proxy");
    let uri = "wss://chatgpt.com/backend-api/codex/responses"
        .parse()
        .expect("websocket URI should parse");
    let connected = connect_runtime_proxy_upstream_tcp_stream(90, &shared, &uri)
        .expect("websocket connect should establish HTTP CONNECT tunnel");
    drop(connected.stream);

    let connect_request = request_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("proxy should observe CONNECT request");
    proxy_thread
        .join()
        .expect("proxy thread should finish cleanly");
    assert!(
        connect_request.starts_with("CONNECT chatgpt.com:443 HTTP/1.1\r\n"),
        "CONNECT target should be the upstream websocket host: {connect_request:?}"
    );
    assert!(
        connect_request.contains("\r\nHost: chatgpt.com:443\r\n"),
        "CONNECT request should include target Host header: {connect_request:?}"
    );
    assert!(
        connect_request.contains("\r\nProxy-Authorization: Basic dXNlcjpwYXNz\r\n"),
        "CONNECT request should forward basic proxy auth without logging secrets: {connect_request:?}"
    );

    let log = read_websocket_test_log_after_marker(&shared.log_path, "websocket_proxy_tunnel_ok");
    assert!(
        log.contains("websocket_proxy_connect_start")
            && log.contains("websocket_proxy_tunnel_ok")
            && log.contains("target=chatgpt.com:443"),
        "proxy tunnel markers should be logged: {log}"
    );
    assert!(
        !log.contains("user:pass"),
        "proxy credentials must not be written to runtime logs: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_proxy_selection_honors_no_proxy_for_upstream_host() {
    let _env_lock = TestEnvVarGuard::lock();
    let _http_proxy = TestEnvVarGuard::unset("HTTP_PROXY");
    let _http_proxy_lower = TestEnvVarGuard::unset("http_proxy");
    let _https_proxy = TestEnvVarGuard::set("HTTPS_PROXY", "http://127.0.0.1:1086");
    let _https_proxy_lower = TestEnvVarGuard::unset("https_proxy");
    let _all_proxy = TestEnvVarGuard::unset("ALL_PROXY");
    let _all_proxy_lower = TestEnvVarGuard::unset("all_proxy");
    let _proxy = TestEnvVarGuard::unset("PROXY");
    let _proxy_lower = TestEnvVarGuard::unset("proxy");
    let _no_proxy = TestEnvVarGuard::set("NO_PROXY", ".chatgpt.com,example.test:443");
    let _no_proxy_lower = TestEnvVarGuard::unset("no_proxy");

    let shared = websocket_test_shared("no-proxy");
    let uri = "wss://chatgpt.com/backend-api/codex/responses"
        .parse()
        .expect("websocket URI should parse");
    let target = runtime_websocket_target_from_uri(&uri).expect("websocket target should parse");

    assert!(
        runtime_websocket_http_proxy_for_uri(&shared, &uri, &target)
            .expect("proxy selection should succeed")
            .is_none(),
        "NO_PROXY should bypass the configured proxy for chatgpt.com"
    );
}

#[test]
fn websocket_local_pressure_connect_error_does_not_mark_profile_transport_failure() {
    let shared = websocket_test_shared("local-pressure");
    let _ = std::fs::remove_file(&shared.log_path);
    let cases = [
        (
            RuntimeWebsocketLocalPressureKind::DnsResolveTimeout,
            "websocket DNS resolution timed out after 25ms for slow.example.test:443",
        ),
        (
            RuntimeWebsocketLocalPressureKind::DnsResolveExecutorOverflow,
            "websocket DNS resolution executor overflow for slow.example.test:443",
        ),
        (
            RuntimeWebsocketLocalPressureKind::TcpConnectExecutorOverflow,
            "websocket TCP connect executor overflow for 127.0.0.1:443",
        ),
    ];

    for (index, (kind, message)) in cases.into_iter().enumerate() {
        let err = WsError::Io(runtime_websocket_local_pressure_io_error(kind, message));
        let request_id = 81 + index as u64;
        let transport_error =
            runtime_websocket_connect_transport_error(&shared, request_id, "main", &err);

        assert!(
            transport_error
                .to_string()
                .contains("failed to connect runtime websocket upstream"),
            "connect helper should preserve websocket failure for caller"
        );
    }
    {
        let runtime = shared
            .runtime
            .lock()
            .expect("runtime state should not be poisoned");
        assert!(
            runtime.profile_transport_backoff_until.is_empty(),
            "local websocket pressure should not mark profile transport backoff"
        );
        assert!(
            runtime.profile_health.is_empty(),
            "local websocket pressure should not penalize profile health"
        );
    }

    runtime_proxy_flush_logs_for_path(&shared.log_path);
    let log = std::fs::read_to_string(&shared.log_path).expect("local-pressure log should exist");
    for (index, (kind, _message)) in cases.into_iter().enumerate() {
        let request_id = 81 + index as u64;
        let expected_class = format!("class={}", kind.as_str());
        let expected_request = format!("request={request_id}");
        assert!(
            log.lines().any(|line| {
                line.contains("websocket_connect_local_pressure")
                    && line.contains(&expected_request)
                    && line.contains("transport=websocket")
                    && line.contains("profile=main")
                    && line.contains(&expected_class)
            }),
            "local pressure log marker missing: {log}"
        );
    }
    assert!(
        !log.contains("profile_transport_failure") && !log.contains("profile_transport_backoff"),
        "local pressure should not emit profile transport health markers: {log}"
    );

    let _ = std::fs::remove_file(&shared.log_path);
}

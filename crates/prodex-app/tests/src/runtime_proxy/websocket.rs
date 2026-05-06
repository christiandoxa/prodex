use super::*;

fn websocket_test_log_path(name: &str) -> PathBuf {
    static NEXT_LOG_ID: AtomicU64 = AtomicU64::new(1);
    std::env::temp_dir().join(format!(
        "prodex-websocket-{name}-{}-{}.log",
        std::process::id(),
        NEXT_LOG_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn read_websocket_test_log_after_marker(log_path: &Path, marker: &str) -> String {
    let started_at = Instant::now();
    loop {
        runtime_proxy_flush_logs_for_path(log_path);
        let log = std::fs::read_to_string(log_path).unwrap_or_default();
        if log.contains(marker) || started_at.elapsed() >= Duration::from_secs(1) {
            return log;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn websocket_test_local_pair() -> (RuntimeLocalWebSocket, RuntimeUpstreamWebSocket) {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("test websocket listener should bind");
    let addr = listener
        .local_addr()
        .expect("test websocket listener should expose address");
    let client = thread::spawn(move || {
        let (socket, _response) =
            tungstenite::connect(format!("ws://{addr}")).expect("test websocket client connect");
        socket
    });
    let (server_stream, _addr) = listener
        .accept()
        .expect("test websocket server should accept connection");
    let local_socket =
        tungstenite::accept(Box::new(server_stream) as Box<dyn TinyReadWrite + Send>)
            .expect("test websocket server handshake");
    let client_socket = client
        .join()
        .expect("test websocket client thread should join");
    (local_socket, client_socket)
}

fn websocket_test_shared(name: &str) -> RuntimeRotationProxyShared {
    let root = std::env::temp_dir().join(format!("prodex-websocket-{name}-{}", std::process::id()));
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    };

    RuntimeRotationProxyShared {
        upstream_no_proxy: false,
        async_client: reqwest::Client::new(),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state: AppState::default(),
            upstream_base_url: "http://127.0.0.1".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
        log_path: websocket_test_log_path(name),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 8,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
            responses: 8,
            compact: 8,
            websocket: 8,
            standard: 8,
        }),
    }
}

fn websocket_test_shared_with_main_profile(
    name: &str,
    upstream_addr: std::net::SocketAddr,
) -> RuntimeRotationProxyShared {
    let shared = websocket_test_shared(name);
    let auth_location = {
        let mut runtime = shared
            .runtime
            .lock()
            .expect("runtime state should not be poisoned");
        runtime.upstream_base_url = format!("http://{upstream_addr}");
        let codex_home = runtime.paths.root.join("main-home");
        runtime.state.profiles.insert(
            "main".to_string(),
            ProfileEntry {
                codex_home: codex_home.clone(),
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        );
        let auth_location = secret_store::auth_json_location(&codex_home);
        runtime.profile_usage_auth.insert(
            "main".to_string(),
            RuntimeProfileUsageAuthCacheEntry {
                auth: UsageAuth {
                    access_token: "test-token".to_string(),
                    account_id: None,
                    refresh_token: None,
                    expires_at: None,
                    last_refresh: None,
                },
                location: auth_location.clone(),
                revision: None,
            },
        );
        let now = Local::now().timestamp();
        runtime.profile_usage_snapshots.insert(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 100,
                five_hour_reset_at: now + 18_000,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 100,
                weekly_reset_at: now + 604_800,
            },
        );
        auth_location
    };
    if let secret_store::SecretLocation::File(path) = &auth_location {
        let _ = std::fs::remove_file(path);
    }
    shared
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

#[test]
fn websocket_precommit_hold_response_created_commits_before_terminal_event() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("upstream websocket listener should bind");
    let upstream_addr = listener
        .local_addr()
        .expect("upstream websocket listener should expose address");
    let (allow_completed_tx, allow_completed_rx) = mpsc::channel::<()>();
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
        allow_completed_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("test should allow terminal event after response.created is forwarded");
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
    allow_completed_tx
        .send(())
        .expect("test should allow upstream terminal event");
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

    let log =
        read_websocket_test_log_after_marker(&shared.log_path, "websocket_precommit_hold_promoted");
    assert!(
        log.contains("websocket_precommit_hold_promoted")
            && log.contains("request=31")
            && log.contains("event=response_created")
            && !log.contains("websocket_precommit_hold_timeout"),
        "response.created should commit and forward without timeout promotion: {log}"
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

    let err = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
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
    .expect_err("hold timeout without response id signal should not promote buffered frames");

    assert!(
        err.to_string()
            .contains("runtime websocket upstream failed before response.completed"),
        "unexpected no-id timeout error: {err:?}"
    );
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

    let err = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
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
    .expect_err("created hold timeout without usable response id should not promote");

    assert!(
        err.to_string()
            .contains("runtime websocket upstream failed before response.completed"),
        "unexpected created-without-id timeout error: {err:?}"
    );
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
fn websocket_precommit_hold_timeout_does_not_promote_previous_response_affinity() {
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
        thread::sleep(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms() + 40,
        ));
    });

    let shared = websocket_test_shared_with_main_profile("precommit-hold-affinity", upstream_addr);
    let (mut local_socket, _client_socket) = websocket_test_local_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let err = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 32,
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
    .expect_err("hard-affinity hold timeout should not promote buffered frames");

    assert!(
        err.to_string()
            .contains("runtime websocket upstream failed before response.completed"),
        "unexpected hard-affinity timeout error: {err:?}"
    );
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(&shared.log_path, "upstream_read_error");
    assert!(
        log.contains("upstream_read_error")
            && log.contains("request=32")
            && !log.contains("websocket_precommit_hold_promoted")
            && !log.contains("transport=websocket committed profile=main"),
        "hard-affinity hold timeout must not commit/promote: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_precommit_hold_timeout_does_not_promote_reused_session() {
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
            .send(WsMessage::Text(
                r#"{"type":"response.created","response":{"id":"resp-reuse-hold"}}"#
                    .to_string()
                    .into(),
            ))
            .expect("upstream should send reused response.created");
        thread::sleep(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms() + 40,
        ));
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

    let reused_attempt = attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
        request_id: 34,
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
    .expect("reused websocket attempt should return watchdog outcome");

    assert!(matches!(
        reused_attempt,
        RuntimeWebsocketAttempt::ReuseWatchdogTripped {
            profile_name,
            event: "upstream_read_error",
        } if profile_name == "main"
    ));
    upstream
        .join()
        .expect("upstream websocket thread should finish");

    let log = read_websocket_test_log_after_marker(&shared.log_path, "upstream_read_error");
    assert!(
        log.contains("websocket_reuse_watchdog")
            && log.contains("upstream_read_error")
            && log.contains("request=34")
            && !log.contains("request=34 transport=websocket committed profile=main")
            && !log.contains("websocket_precommit_hold_promoted request=34"),
        "reused-session hold timeout must not commit/promote: {log}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

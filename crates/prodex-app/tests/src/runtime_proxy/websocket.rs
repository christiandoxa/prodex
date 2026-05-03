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

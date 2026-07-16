use super::*;

#[path = "websocket/connect.rs"]
mod connect;
#[path = "websocket/precommit_hold.rs"]
mod precommit_hold;

pub(super) fn websocket_test_log_path(name: &str) -> PathBuf {
    static NEXT_LOG_ID: AtomicU64 = AtomicU64::new(1);
    std::env::temp_dir().join(format!(
        "prodex-websocket-{name}-{}-{}.log",
        std::process::id(),
        NEXT_LOG_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(super) fn read_websocket_test_log_after_marker(log_path: &Path, marker: &str) -> String {
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

pub(super) fn websocket_test_local_pair() -> (RuntimeLocalWebSocket, RuntimeUpstreamWebSocket) {
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

pub(super) fn websocket_test_shared(name: &str) -> RuntimeRotationProxyShared {
    let root = std::env::temp_dir().join(format!("prodex-websocket-{name}-{}", std::process::id()));
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    };

    RuntimeRotationProxyShared {
        runtime_config: Arc::new(crate::RuntimeConfig::compatibility_current()),
        upstream_no_proxy: false,
        auto_redeem_enabled: false,
        compact_client: reqwest::Client::new(),
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

pub(super) fn websocket_test_shared_with_main_profile(
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

pub(super) fn websocket_unused_local_addr() -> std::net::SocketAddr {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("unused local listener should bind");
    listener
        .local_addr()
        .expect("unused local listener should expose address")
}

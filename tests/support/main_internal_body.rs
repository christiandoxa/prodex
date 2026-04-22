use super::*;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use crate::TestEnvVarGuard;

fn running_in_ci() -> bool {
    std::env::var_os("GITHUB_ACTIONS").is_some() || std::env::var_os("CI").is_some()
}

fn ci_timing_upper_bound_ms(local_ms: u64, ci_ms: u64) -> Duration {
    if running_in_ci() {
        Duration::from_millis(ci_ms.max(local_ms))
    } else {
        Duration::from_millis(local_ms)
    }
}

fn ci_timing_budget_ms(local_ms: u64, ci_ms: u64) -> String {
    if running_in_ci() {
        ci_ms.max(local_ms).to_string()
    } else {
        local_ms.to_string()
    }
}

fn ci_runtime_proxy_timeout_guard(
    env_key: &'static str,
    local_ms: u64,
    ci_ms: u64,
) -> TestEnvVarGuard {
    TestEnvVarGuard::set(env_key, &ci_timing_budget_ms(local_ms, ci_ms))
}

fn ci_runtime_proxy_admission_wait_budget_guard(local_ms: u64, ci_ms: u64) -> TestEnvVarGuard {
    ci_runtime_proxy_timeout_guard(
        "PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS",
        local_ms,
        ci_ms,
    )
}

fn ci_runtime_proxy_websocket_timeout_guards() -> (TestEnvVarGuard, TestEnvVarGuard) {
    (
        ci_runtime_proxy_timeout_guard(
            "PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS",
            250,
            1_000,
        ),
        ci_runtime_proxy_timeout_guard(
            "PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS",
            120,
            1_000,
        ),
    )
}

fn usage_with_main_windows(
    five_hour_remaining: i64,
    five_hour_reset_offset_seconds: i64,
    weekly_remaining: i64,
    weekly_reset_offset_seconds: i64,
) -> UsageResponse {
    let now = Local::now().timestamp();
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some((100 - five_hour_remaining).clamp(0, 100)),
                reset_at: Some(now + five_hour_reset_offset_seconds),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some((100 - weekly_remaining).clamp(0, 100)),
                reset_at: Some(now + weekly_reset_offset_seconds),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

fn runtime_proxy_backend_profile_name_for_account_id(account_id: &str) -> Option<&'static str> {
    match account_id {
        "main-account" => Some("main"),
        "second-account" => Some("second"),
        "third-account" => Some("third"),
        "fourth-account" => Some("fourth"),
        "fifth-account" => Some("fifth"),
        _ => None,
    }
}

fn runtime_proxy_backend_account_id_for_profile_name(profile_name: &str) -> Option<&'static str> {
    match profile_name {
        "main" => Some("main-account"),
        "second" => Some("second-account"),
        "third" => Some("third-account"),
        "fourth" => Some("fourth-account"),
        "fifth" => Some("fifth-account"),
        _ => None,
    }
}

fn runtime_proxy_backend_initial_response_id_for_account(account_id: &str) -> Option<&'static str> {
    match account_id {
        "second-account" => Some("resp-second"),
        "third-account" => Some("resp-third"),
        "fourth-account" => Some("resp-fourth"),
        "fifth-account" => Some("resp-fifth"),
        _ => None,
    }
}

fn runtime_proxy_backend_response_owner_account_id(response_id: &str) -> Option<&'static str> {
    if response_id == "resp-second" || response_id.starts_with("resp-second-next") {
        Some("second-account")
    } else if response_id == "resp-third" || response_id.starts_with("resp-third-next") {
        Some("third-account")
    } else if response_id == "resp-fourth" || response_id.starts_with("resp-fourth-next") {
        Some("fourth-account")
    } else if response_id == "resp-fifth" || response_id.starts_with("resp-fifth-next") {
        Some("fifth-account")
    } else {
        None
    }
}

fn runtime_proxy_backend_profile_name_for_response_id(response_id: &str) -> Option<&'static str> {
    runtime_proxy_backend_response_owner_account_id(response_id)
        .and_then(runtime_proxy_backend_profile_name_for_account_id)
}

fn runtime_proxy_backend_initial_response_id_for_profile_name(
    profile_name: &str,
) -> Option<&'static str> {
    runtime_proxy_backend_account_id_for_profile_name(profile_name)
        .and_then(runtime_proxy_backend_initial_response_id_for_account)
}

fn runtime_proxy_backend_next_response_id(previous_response_id: Option<&str>) -> Option<String> {
    match previous_response_id {
        Some("resp-second") | Some("resp-third") => {
            Some(format!("{}-next", previous_response_id.unwrap()))
        }
        Some(previous_response_id)
            if previous_response_id.starts_with("resp-second-next")
                || previous_response_id.starts_with("resp-third-next") =>
        {
            Some(format!("{previous_response_id}-next"))
        }
        _ => None,
    }
}

fn runtime_proxy_backend_is_owned_continuation(
    account_id: &str,
    previous_response_id: Option<&str>,
) -> bool {
    previous_response_id.is_some_and(|previous_response_id| {
        runtime_proxy_backend_response_owner_account_id(previous_response_id) == Some(account_id)
            && runtime_proxy_backend_next_response_id(Some(previous_response_id)).is_some()
    })
}

fn runtime_proxy_backend_profile_name_for_compact_turn_state(
    turn_state: &str,
) -> Option<&'static str> {
    match turn_state {
        "compact-turn-main" => Some("main"),
        "compact-turn-second" => Some("second"),
        "compact-turn-third" => Some("third"),
        _ => None,
    }
}

fn runtime_proxy_response_ids_from_http_body(body: &str) -> Vec<String> {
    let response_ids = extract_runtime_response_ids_from_body_bytes(body.as_bytes());
    if !response_ids.is_empty() {
        return response_ids;
    }

    body.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .flat_map(extract_runtime_response_ids_from_payload)
        .collect()
}

struct TestDir {
    path: PathBuf,
}

static TEST_DIR_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl TestDir {
    fn new() -> Self {
        wait_for_runtime_background_queues_idle();
        for _ in 0..32 {
            let unique = format!(
                "prodex-runtime-test-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock should be after unix epoch")
                    .as_nanos(),
                TEST_DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            );
            let path = std::env::temp_dir().join(unique);
            match fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!("failed to create test temp dir: {err}"),
            }
        }
        panic!("failed to allocate unique test temp dir after repeated collisions");
    }
}

fn wait_for_runtime_background_queues_idle() {
    let deadline = Instant::now() + ci_timing_upper_bound_ms(5_000, 10_000);
    let probe_refresh_grace = ci_timing_upper_bound_ms(250, 2_000);
    let mut lingering_probe_refresh_since = None;
    loop {
        let state_save_backlog = runtime_state_save_queue_backlog();
        let state_save_active = runtime_state_save_queue_active();
        let continuation_backlog = runtime_continuation_journal_queue_backlog();
        let continuation_active = runtime_continuation_journal_queue_active();
        let probe_refresh_backlog = runtime_probe_refresh_queue_backlog();
        let probe_refresh_active = runtime_probe_refresh_queue_active();
        let backlog = state_save_backlog + continuation_backlog + probe_refresh_backlog;
        let active = state_save_active + continuation_active + probe_refresh_active;
        if backlog == 0 && active == 0 {
            return;
        }
        let only_lingering_probe_refresh = state_save_backlog == 0
            && state_save_active == 0
            && continuation_backlog == 0
            && continuation_active == 0
            && (probe_refresh_backlog > 0 || probe_refresh_active > 0);
        if only_lingering_probe_refresh {
            let lingering_since = lingering_probe_refresh_since.get_or_insert_with(Instant::now);
            // Tests use isolated state roots, so once every other queue has drained we can
            // discard stale best-effort probe refresh backlog instead of timing out on work
            // that belongs to a previous test. Any workers already in flight can finish in the
            // background without touching the next test's state root.
            if lingering_since.elapsed() >= probe_refresh_grace {
                if probe_refresh_backlog > 0 {
                    let queue = runtime_probe_refresh_queue();
                    let mut pending = queue
                        .pending
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    pending.clear();
                }
                return;
            }
        } else {
            lingering_probe_refresh_since = None;
        }
        if Instant::now() >= deadline {
            panic!(
                "runtime background queues did not go idle before timeout: backlog={backlog} active={active} state_save_backlog={state_save_backlog} state_save_active={state_save_active} continuation_backlog={continuation_backlog} continuation_active={continuation_active} probe_refresh_backlog={probe_refresh_backlog} probe_refresh_active={probe_refresh_active}"
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_backend_usage_accounts(
    backend: &RuntimeProxyBackend,
    expected_accounts: &[&str],
) -> Vec<String> {
    let mut expected_accounts = expected_accounts
        .iter()
        .map(|account| (*account).to_string())
        .collect::<Vec<_>>();
    expected_accounts.sort();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut usage_accounts = backend.usage_accounts();
        usage_accounts.sort();
        if usage_accounts == expected_accounts || Instant::now() >= deadline {
            return usage_accounts;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn sorted_backend_usage_accounts(backend: &RuntimeProxyBackend) -> Vec<String> {
    let mut usage_accounts = backend.usage_accounts();
    usage_accounts.sort();
    usage_accounts
}

fn stale_critical_runtime_usage_snapshot(now: i64) -> RuntimeProfileUsageSnapshot {
    RuntimeProfileUsageSnapshot {
        checked_at: now - (RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS + 1),
        five_hour_status: RuntimeQuotaWindowStatus::Critical,
        five_hour_remaining_percent: 1,
        five_hour_reset_at: now + 300,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: 70,
        weekly_reset_at: now + 86_400,
    }
}

fn ready_runtime_usage_snapshot(now: i64, remaining_percent: i64) -> RuntimeProfileUsageSnapshot {
    RuntimeProfileUsageSnapshot {
        checked_at: now,
        five_hour_status: RuntimeQuotaWindowStatus::Ready,
        five_hour_remaining_percent: remaining_percent,
        five_hour_reset_at: now + 18_000,
        weekly_status: RuntimeQuotaWindowStatus::Ready,
        weekly_remaining_percent: remaining_percent,
        weekly_reset_at: now + 604_800,
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn runtime_rotation_proxy_shared(
    temp_dir: &TestDir,
    runtime: RuntimeRotationState,
    active_request_limit: usize,
) -> RuntimeRotationProxyShared {
    let active_request_limit = active_request_limit.max(1);
    RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(active_request_limit),
        runtime: Arc::new(Mutex::new(runtime)),
    }
}

fn runtime_shared_for_cold_start_probe_selection(
    temp_dir: &TestDir,
    upstream_base_url: String,
) -> RuntimeRotationProxyShared {
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    runtime_rotation_proxy_shared(
        temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([
                    (
                        "main".to_string(),
                        ProfileEntry {
                            codex_home: main_home,
                            managed: true,
                            email: Some("main@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                    (
                        "second".to_string(),
                        ProfileEntry {
                            codex_home: second_home,
                            managed: true,
                            email: Some("second@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                ]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url,
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(80, 300, 80, 86_400)),
                },
            )]),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::from([(
                runtime_profile_auth_failure_key("main"),
                RuntimeProfileHealth {
                    score: 1,
                    updated_at: now,
                },
            )]),
        },
        usize::MAX,
    )
}

struct TestProbeRefreshBacklogGuard {
    keys: Vec<(PathBuf, String)>,
}

impl Drop for TestProbeRefreshBacklogGuard {
    fn drop(&mut self) {
        let queue = runtime_probe_refresh_queue();
        let mut pending = queue
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for key in &self.keys {
            pending.remove(key);
        }
    }
}

fn force_runtime_probe_refresh_backlog(
    shared: &RuntimeRotationProxyShared,
    backlog: usize,
) -> TestProbeRefreshBacklogGuard {
    wait_for_runtime_background_queues_idle();
    let (state_file, upstream_base_url, root) = {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        (
            runtime.paths.state_file.clone(),
            runtime.upstream_base_url.clone(),
            runtime.paths.root.clone(),
        )
    };
    let queue = runtime_probe_refresh_queue();
    let mut pending = queue
        .pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(
        pending.is_empty(),
        "probe refresh backlog helper requires an idle queue"
    );
    let mut keys = Vec::with_capacity(backlog);
    for index in 0..backlog {
        let profile_name = format!("probe-pressure-{index}");
        let key = (state_file.clone(), profile_name.clone());
        pending.insert(
            key.clone(),
            RuntimeProbeRefreshJob {
                shared: shared.clone(),
                profile_name,
                codex_home: root.join(format!("probe-pressure-{index}")),
                upstream_base_url: upstream_base_url.clone(),
                queued_at: Instant::now(),
            },
        );
        keys.push(key);
    }
    TestProbeRefreshBacklogGuard { keys }
}

fn closed_loopback_backend_base_url() -> String {
    let listener =
        TcpListener::bind("127.0.0.1:0").expect("closed loopback helper should bind a port");
    let addr = listener
        .local_addr()
        .expect("closed loopback helper should read local address");
    drop(listener);
    format!("http://{addr}/backend-api")
}

fn unresponsive_loopback_backend_listener() -> (TcpListener, String) {
    let listener =
        TcpListener::bind("127.0.0.1:0").expect("unresponsive loopback helper should bind");
    let addr = listener
        .local_addr()
        .expect("unresponsive loopback helper should read local address");
    (listener, format!("http://{addr}/backend-api"))
}

struct TwoChunkReader {
    chunks: Vec<Vec<u8>>,
    index: usize,
    offset: usize,
}

impl TwoChunkReader {
    fn new(chunks: Vec<Vec<u8>>) -> Self {
        Self {
            chunks,
            index: 0,
            offset: 0,
        }
    }
}

impl Read for TwoChunkReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.index >= self.chunks.len() {
            return Ok(0);
        }
        let chunk = &self.chunks[self.index];
        let remaining = &chunk[self.offset..];
        let len = remaining.len().min(buf.len());
        buf[..len].copy_from_slice(&remaining[..len]);
        self.offset += len;
        if self.offset >= chunk.len() {
            self.index += 1;
            self.offset = 0;
        }
        Ok(len)
    }
}

struct FailAfterFirstChunkWriter {
    saw_first_chunk_body: bool,
}

impl FailAfterFirstChunkWriter {
    fn new() -> Self {
        Self {
            saw_first_chunk_body: false,
        }
    }
}

impl Write for FailAfterFirstChunkWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.saw_first_chunk_body {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "synthetic local writer disconnect",
            ));
        }
        if buf.starts_with(b"data:") {
            self.saw_first_chunk_body = true;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.saw_first_chunk_body {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "synthetic local writer disconnect",
            ));
        }
        Ok(())
    }
}

struct FailOnUnexpectedMidStreamFlushWriter {
    flush_count: usize,
    saw_trailer: bool,
}

impl FailOnUnexpectedMidStreamFlushWriter {
    fn new() -> Self {
        Self {
            flush_count: 0,
            saw_trailer: false,
        }
    }
}

impl Write for FailOnUnexpectedMidStreamFlushWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf == b"0\r\n\r\n" {
            self.saw_trailer = true;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.flush_count >= 2 && !self.saw_trailer {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "unexpected non-SSE chunk flush before trailer",
            ));
        }
        self.flush_count += 1;
        Ok(())
    }
}

fn set_test_websocket_io_timeout(
    socket: &mut WsSocket<MaybeTlsStream<TcpStream>>,
    timeout: Duration,
) {
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => {
            let _ = stream.set_read_timeout(Some(timeout));
            let _ = stream.set_write_timeout(Some(timeout));
        }
        MaybeTlsStream::Rustls(stream) => {
            let _ = stream.sock.set_read_timeout(Some(timeout));
            let _ = stream.sock.set_write_timeout(Some(timeout));
        }
        _ => {}
    }
}

fn runtime_test_local_websocket_pair() -> (RuntimeLocalWebSocket, RuntimeUpstreamWebSocket) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test websocket listener should bind");
    let addr = listener
        .local_addr()
        .expect("test websocket listener should expose an address");
    let client = thread::spawn(move || {
        let (socket, _response) =
            ws_connect(format!("ws://{addr}")).expect("test websocket client should connect");
        socket
    });
    let (server_stream, _addr) = listener
        .accept()
        .expect("test websocket server should accept a connection");
    let local_socket =
        tungstenite::accept(Box::new(server_stream) as Box<dyn TinyReadWrite + Send>)
            .expect("test websocket server handshake should succeed");
    let client_socket = client
        .join()
        .expect("test websocket client thread should join");
    (local_socket, client_socket)
}

fn wait_for_runtime_log_tail_until<F, G>(
    mut read_tail: G,
    predicate: F,
    local_ms: u64,
    ci_ms: u64,
    poll_ms: u64,
) -> Vec<u8>
where
    F: Fn(&str) -> bool,
    G: FnMut() -> Option<Vec<u8>>,
{
    let deadline = Instant::now() + ci_timing_upper_bound_ms(local_ms, ci_ms);
    let mut last_tail = Vec::new();

    loop {
        if let Some(tail) = read_tail() {
            last_tail = tail;
            let text = String::from_utf8_lossy(&last_tail);
            if predicate(&text) {
                return last_tail;
            }
        }

        if Instant::now() >= deadline {
            return last_tail;
        }

        thread::sleep(Duration::from_millis(poll_ms));
    }
}

fn wait_for_state<F>(paths: &AppPaths, predicate: F) -> AppState
where
    F: Fn(&AppState) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_state = None;
    loop {
        if let Ok(state) = AppState::load(paths) {
            if predicate(&state) {
                return state;
            }
            last_state = Some(state);
        }
        if Instant::now() >= deadline {
            let state = AppState::load(paths).expect("state should reload");
            panic!(
                "timed out waiting for app state predicate; last_state={:?} final_state={:?}",
                last_state, state
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

include!("main_internal/runtime_proxy_continuation_helpers.rs");

fn write_versioned_runtime_sidecar<T: Serialize>(
    path: &Path,
    backup_path: &Path,
    generation: u64,
    value: &T,
) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("versioned sidecar primary dir should exist");
    }
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).expect("versioned sidecar backup dir should exist");
    }
    let json = serde_json::to_string_pretty(&VersionedJson { generation, value })
        .expect("versioned sidecar should serialize");
    fs::write(path, &json).expect("versioned sidecar primary should write");
    fs::write(backup_path, &json).expect("versioned sidecar backup should write");
}

fn tiny_http_response_status_and_body(response: tiny_http::ResponseBox) -> (u16, String) {
    let status = response.status_code().0;
    let mut bytes = Vec::new();
    response
        .raw_print(&mut bytes, (1, 0).into(), &[], false, None)
        .expect("response should serialize");
    let text = String::from_utf8(bytes).expect("response bytes should be utf8");
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    (status, body)
}

fn tiny_http_response_status_content_type_and_body(
    response: tiny_http::ResponseBox,
) -> (u16, Option<String>, String) {
    let status = response.status_code().0;
    let mut bytes = Vec::new();
    response
        .raw_print(&mut bytes, (1, 0).into(), &[], false, None)
        .expect("response should serialize");
    let text = String::from_utf8(bytes).expect("response bytes should be utf8");
    let (headers, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_str(), ""));
    let content_type = headers.lines().find_map(|line| {
        line.strip_prefix("Content-Type: ")
            .map(|value| value.to_string())
    });
    (status, content_type, body.to_string())
}

struct RuntimeProxyBackend {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    responses_accounts: Arc<Mutex<Vec<String>>>,
    responses_headers: Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    responses_bodies: Arc<Mutex<Vec<String>>>,
    websocket_requests: Arc<Mutex<Vec<String>>>,
    usage_accounts: Arc<Mutex<Vec<String>>>,
    connection_threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Clone, Copy)]
enum RuntimeProxyBackendMode {
    HttpOnly,
    HttpOnlyUnauthorizedMain,
    HttpOnlyBufferedJson,
    HttpOnlyAnthropicWebSearchFollowup,
    HttpOnlyAnthropicMcpStream,
    HttpOnlyInitialBodyStall,
    HttpOnlySlowStream,
    HttpOnlyStallAfterSeveralChunks,
    HttpOnlyResetBeforeFirstByte,
    HttpOnlyResetAfterFirstChunk,
    HttpOnlyPreviousResponseNeedsTurnState,
    HttpOnlyPreviousResponseNotFoundAfterCommit,
    HttpOnlySseHeadersArrayTurnState,
    HttpOnlyCompactOverloaded,
    HttpOnlyCompactPreviousResponseNotFound,
    HttpOnlyLargeCompactResponse,
    HttpOnlyUsageLimitMessage,
    HttpOnlyUsageLimitMessageLateReadyFifth,
    HttpOnlyDelayedQuotaAfterOutputItemAdded,
    HttpOnlyQuotaThenToolOutputFreshFallbackError,
    HttpOnlyPreviousResponseToolContextMissing,
    HttpOnlyPlain429,
    Websocket,
    WebsocketOverloaded,
    WebsocketDelayedQuotaAfterPrelude,
    WebsocketDelayedOverloadAfterPrelude,
    WebsocketReuseSilentHang,
    WebsocketReusePrecommitHoldStall,
    WebsocketReuseOwnedPreviousResponseSilentHang,
    WebsocketReusePreviousResponseNeedsTurnState,
    WebsocketPreviousResponseNotFoundAfterCommit,
    WebsocketPreviousResponseMissingWithoutTurnState,
    WebsocketOwnedToolOutputNeedsSessionReplay,
    WebsocketCloseMidTurn,
    WebsocketPreviousResponseNeedsTurnState,
    WebsocketStaleReuseNeedsTurnState,
    WebsocketTopLevelResponseId,
    WebsocketRealtimeSideband,
}

impl RuntimeProxyBackend {
    fn start() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnly)
    }

    fn start_http_initial_body_stall() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
    }

    fn start_http_unauthorized_main() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyUnauthorizedMain)
    }

    fn start_http_buffered_json() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyBufferedJson)
    }

    fn start_http_anthropic_web_search_followup() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup)
    }

    fn start_http_anthropic_mcp_stream() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyAnthropicMcpStream)
    }

    fn start_http_slow_stream() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlySlowStream)
    }

    fn start_http_stall_after_several_chunks() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks)
    }

    fn start_http_reset_before_first_byte() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyResetBeforeFirstByte)
    }

    fn start_http_reset_after_first_chunk() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyResetAfterFirstChunk)
    }

    fn start_http_previous_response_needs_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyPreviousResponseNeedsTurnState)
    }

    fn start_http_previous_response_not_found_after_commit() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyPreviousResponseNotFoundAfterCommit)
    }

    fn start_http_sse_headers_array_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlySseHeadersArrayTurnState)
    }

    fn start_http_compact_overloaded() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyCompactOverloaded)
    }

    fn start_http_compact_previous_response_not_found() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyCompactPreviousResponseNotFound)
    }

    fn start_http_large_compact_response() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyLargeCompactResponse)
    }

    fn start_http_usage_limit_message() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage)
    }

    fn start_http_usage_limit_message_late_ready_fifth() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth)
    }

    fn start_http_delayed_quota_after_output_item_added() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyDelayedQuotaAfterOutputItemAdded)
    }

    fn start_http_quota_then_tool_output_fresh_fallback_error() -> Self {
        Self::start_with_mode(
            RuntimeProxyBackendMode::HttpOnlyQuotaThenToolOutputFreshFallbackError,
        )
    }

    fn start_http_previous_response_tool_context_missing() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyPreviousResponseToolContextMissing)
    }

    fn start_http_plain_429() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyPlain429)
    }

    fn start_websocket() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::Websocket)
    }

    fn start_websocket_overloaded() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketOverloaded)
    }

    fn start_websocket_delayed_quota_after_prelude() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude)
    }

    fn start_websocket_delayed_overload_after_prelude() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude)
    }

    fn start_websocket_reuse_silent_hang() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketReuseSilentHang)
    }

    fn start_websocket_reuse_precommit_hold_stall() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketReusePrecommitHoldStall)
    }

    fn start_websocket_reuse_owned_previous_response_silent_hang() -> Self {
        Self::start_with_mode(
            RuntimeProxyBackendMode::WebsocketReuseOwnedPreviousResponseSilentHang,
        )
    }

    fn start_websocket_reuse_previous_response_needs_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState)
    }

    fn start_websocket_previous_response_not_found_after_commit() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterCommit)
    }

    fn start_websocket_previous_response_missing_without_turn_state() -> Self {
        Self::start_with_mode(
            RuntimeProxyBackendMode::WebsocketPreviousResponseMissingWithoutTurnState,
        )
    }

    fn start_websocket_owned_tool_output_needs_session_replay() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketOwnedToolOutputNeedsSessionReplay)
    }

    fn start_websocket_close_mid_turn() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketCloseMidTurn)
    }

    fn start_websocket_previous_response_needs_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState)
    }

    fn start_websocket_stale_reuse_needs_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState)
    }

    fn start_websocket_top_level_response_id() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketTopLevelResponseId)
    }

    fn start_websocket_realtime_sideband() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketRealtimeSideband)
    }

    fn start_with_mode(mode: RuntimeProxyBackendMode) -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("failed to bind runtime proxy backend");
        let addr = listener
            .local_addr()
            .expect("failed to read runtime proxy backend address");
        listener
            .set_nonblocking(true)
            .expect("failed to set runtime proxy backend nonblocking");

        let shutdown = Arc::new(AtomicBool::new(false));
        let responses_accounts = Arc::new(Mutex::new(Vec::new()));
        let responses_headers = Arc::new(Mutex::new(Vec::new()));
        let responses_bodies = Arc::new(Mutex::new(Vec::new()));
        let websocket_requests = Arc::new(Mutex::new(Vec::new()));
        let usage_accounts = Arc::new(Mutex::new(Vec::new()));
        let connection_threads = Arc::new(Mutex::new(Vec::new()));
        let shutdown_flag = Arc::clone(&shutdown);
        let responses_accounts_flag = Arc::clone(&responses_accounts);
        let responses_headers_flag = Arc::clone(&responses_headers);
        let responses_bodies_flag = Arc::clone(&responses_bodies);
        let websocket_requests_flag = Arc::clone(&websocket_requests);
        let usage_accounts_flag = Arc::clone(&usage_accounts);
        let connection_threads_flag = Arc::clone(&connection_threads);
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let responses_accounts_flag = Arc::clone(&responses_accounts_flag);
                        let responses_headers_flag = Arc::clone(&responses_headers_flag);
                        let responses_bodies_flag = Arc::clone(&responses_bodies_flag);
                        let websocket_requests_flag = Arc::clone(&websocket_requests_flag);
                        let usage_accounts_flag = Arc::clone(&usage_accounts_flag);
                        let websocket_enabled = matches!(
                            mode,
                            RuntimeProxyBackendMode::Websocket
                                | RuntimeProxyBackendMode::WebsocketOverloaded
                                | RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude
                                | RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude
                                | RuntimeProxyBackendMode::WebsocketReuseSilentHang
                                | RuntimeProxyBackendMode::WebsocketReusePrecommitHoldStall
                                | RuntimeProxyBackendMode::WebsocketReuseOwnedPreviousResponseSilentHang
                                | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
                                | RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterCommit
                                | RuntimeProxyBackendMode::WebsocketPreviousResponseMissingWithoutTurnState
                                | RuntimeProxyBackendMode::WebsocketOwnedToolOutputNeedsSessionReplay
                                | RuntimeProxyBackendMode::WebsocketCloseMidTurn
                                | RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                                | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
                                | RuntimeProxyBackendMode::WebsocketTopLevelResponseId
                                | RuntimeProxyBackendMode::WebsocketRealtimeSideband
                        );
                        let handler = thread::spawn(move || {
                            if websocket_enabled
                                && runtime_proxy_backend_is_websocket_upgrade(&stream)
                            {
                                handle_runtime_proxy_backend_websocket(
                                    stream,
                                    &responses_accounts_flag,
                                    &responses_headers_flag,
                                    &websocket_requests_flag,
                                    mode,
                                );
                            } else {
                                handle_runtime_proxy_backend_request(
                                    stream,
                                    &responses_accounts_flag,
                                    &responses_headers_flag,
                                    &responses_bodies_flag,
                                    &usage_accounts_flag,
                                    mode,
                                );
                            }
                        });
                        connection_threads_flag
                            .lock()
                            .expect("connection_threads poisoned")
                            .push(handler);
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            addr,
            shutdown,
            responses_accounts,
            responses_headers,
            responses_bodies,
            websocket_requests,
            usage_accounts,
            connection_threads,
            thread: Some(thread),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}/backend-api", self.addr)
    }

    fn responses_accounts(&self) -> Vec<String> {
        self.responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .clone()
    }

    fn responses_headers(&self) -> Vec<BTreeMap<String, String>> {
        self.responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .clone()
    }

    fn responses_bodies(&self) -> Vec<String> {
        self.responses_bodies
            .lock()
            .expect("responses_bodies poisoned")
            .clone()
    }

    fn websocket_requests(&self) -> Vec<String> {
        self.websocket_requests
            .lock()
            .expect("websocket_requests poisoned")
            .clone()
    }

    fn usage_accounts(&self) -> Vec<String> {
        self.usage_accounts
            .lock()
            .expect("usage_accounts poisoned")
            .clone()
    }
}

impl Drop for RuntimeProxyBackend {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let mut handlers = self
            .connection_threads
            .lock()
            .expect("connection_threads poisoned");
        while let Some(thread) = handlers.pop() {
            let _ = thread.join();
        }
    }
}

fn handle_runtime_proxy_backend_request(
    mut stream: TcpStream,
    responses_accounts: &Arc<Mutex<Vec<String>>>,
    responses_headers: &Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    responses_bodies: &Arc<Mutex<Vec<String>>>,
    usage_accounts: &Arc<Mutex<Vec<String>>>,
    mode: RuntimeProxyBackendMode,
) {
    let request = match read_http_request(&mut stream) {
        Some(request) => request,
        None => return,
    };

    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let request_body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    let account_id = request_header(&request, "ChatGPT-Account-Id").unwrap_or_default();
    let turn_state = request_header(&request, "x-codex-turn-state");
    let captured_headers = request_headers_map(&request);

    if path.ends_with("/backend-api/codex/responses")
        && account_id == "main-account"
        && matches!(mode, RuntimeProxyBackendMode::HttpOnlyResetBeforeFirstByte)
    {
        responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .push(account_id.clone());
        responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .push(captured_headers);
        responses_bodies
            .lock()
            .expect("responses_bodies poisoned")
            .push(request_body);
        return;
    }

    let (status_line, content_type, body, response_turn_state, initial_body_stall, chunk_delay) =
        if path.ends_with("/backend-api/wham/usage") {
            usage_accounts
                .lock()
                .expect("usage_accounts poisoned")
                .push(account_id.clone());
            let body = match account_id.as_str() {
                "main-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
                    ) =>
                {
                    runtime_proxy_usage_body_with_remaining("main@example.com", 0, 0)
                }
                "main-account" => runtime_proxy_usage_body("main@example.com"),
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
                    ) =>
                {
                    runtime_proxy_usage_body_with_remaining("second@example.com", 0, 0)
                }
                "second-account" => runtime_proxy_usage_body("second@example.com"),
                "third-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
                    ) =>
                {
                    runtime_proxy_usage_body_with_remaining("third@example.com", 0, 0)
                }
                "third-account" => runtime_proxy_usage_body("third@example.com"),
                "fourth-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
                    ) =>
                {
                    runtime_proxy_usage_body_with_remaining("fourth@example.com", 0, 0)
                }
                "fourth-account" => runtime_proxy_usage_body("fourth@example.com"),
                "fifth-account" => runtime_proxy_usage_body("fifth@example.com"),
                _ => serde_json::json!({ "error": "unauthorized" }).to_string(),
            };
            let status = if matches!(
                account_id.as_str(),
                "main-account"
                    | "second-account"
                    | "third-account"
                    | "fourth-account"
                    | "fifth-account"
            ) {
                "HTTP/1.1 200 OK"
            } else {
                "HTTP/1.1 401 Unauthorized"
            };
            (status, "application/json", body, None, None, None)
        } else if path.ends_with("/backend-api/status") {
            responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .push(account_id.clone());
            responses_headers
                .lock()
                .expect("responses_headers poisoned")
                .push(captured_headers);
            match (account_id.as_str(), mode) {
                (
                    "main-account",
                    RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
                    | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth,
                ) => (
                    "HTTP/1.1 429 Too Many Requests",
                    "application/json",
                    serde_json::json!({
                        "error": {
                            "type": "usage_limit_reached",
                            "message": "The usage limit has been reached",
                        },
                        "status": 429
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("main-account", RuntimeProxyBackendMode::HttpOnlyPlain429) => (
                    "HTTP/1.1 429 Too Many Requests",
                    "text/plain",
                    "Too Many Requests".to_string(),
                    None,
                    None,
                    None,
                ),
                ("main-account", RuntimeProxyBackendMode::HttpOnlyUnauthorizedMain) => (
                    "HTTP/1.1 401 Unauthorized",
                    "application/json",
                    serde_json::json!({
                        "error": "unauthorized"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("main-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "main-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("second-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "second-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("third-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "third-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                _ => (
                    "HTTP/1.1 401 Unauthorized",
                    "application/json",
                    serde_json::json!({ "error": "unauthorized" }).to_string(),
                    None,
                    None,
                    None,
                ),
            }
        } else if path.ends_with("/backend-api/codex/realtime/calls") {
            responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .push(account_id.clone());
            responses_headers
                .lock()
                .expect("responses_headers poisoned")
                .push(captured_headers);
            responses_bodies
                .lock()
                .expect("responses_bodies poisoned")
                .push(request_body);
            match (account_id.as_str(), mode) {
                (
                    "main-account",
                    RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
                    | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth,
                ) => (
                    "HTTP/1.1 429 Too Many Requests",
                    "application/json",
                    serde_json::json!({
                        "error": {
                            "type": "usage_limit_reached",
                            "message": "The usage limit has been reached",
                        },
                        "status": 429
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("main-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "main-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("second-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "second-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("third-account", _) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "status": "ok",
                        "account_id": "third-account"
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                _ => (
                    "HTTP/1.1 401 Unauthorized",
                    "application/json",
                    serde_json::json!({ "error": "unauthorized" }).to_string(),
                    None,
                    None,
                    None,
                ),
            }
        } else if path.ends_with("/backend-api/codex/responses") {
            responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .push(account_id.clone());
            responses_headers
                .lock()
                .expect("responses_headers poisoned")
                .push(captured_headers);
            responses_bodies
                .lock()
                .expect("responses_bodies poisoned")
                .push(request_body.clone());
            let previous_response_id = request_previous_response_id(&request);
            let body_json = serde_json::from_str::<serde_json::Value>(&request_body)
                .unwrap_or(serde_json::Value::Null);
            match account_id.as_str() {
                "main-account" if matches!(mode, RuntimeProxyBackendMode::HttpOnlyPlain429) => (
                    "HTTP/1.1 429 Too Many Requests",
                    "text/plain",
                    "Too Many Requests".to_string(),
                    None,
                    None,
                    None,
                ),
                "main-account"
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyUnauthorizedMain) =>
                {
                    (
                        "HTTP/1.1 401 Unauthorized",
                        "application/json",
                        serde_json::json!({
                            "error": "unauthorized"
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "main-account" => {
                    let body = if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyDelayedQuotaAfterOutputItemAdded
                    ) {
                        concat!(
                            "event: response.created\r\n",
                            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-main\"}}\r\n",
                            "\r\n",
                            "event: response.in_progress\r\n",
                            "data: {\"type\":\"response.in_progress\",\"response\":{\"id\":\"resp-main\"}}\r\n",
                            "\r\n",
                            "event: response.output_item.added\r\n",
                            "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"msg-main\"}}\r\n",
                            "\r\n",
                            "event: response.failed\r\n",
                            "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"You've hit your usage limit. To get more access now, send a request to your admin or try again at Mar 24th, 2026 2:04 AM.\"}}}\r\n",
                            "\r\n"
                        )
                        .to_string()
                    } else if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
                            | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth
                    ) {
                        concat!(
                            "event: response.failed\r\n",
                            "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"You've hit your usage limit. To get more access now, send a request to your admin or try again at Mar 24th, 2026 2:04 AM.\"}}}\r\n",
                            "\r\n"
                        )
                        .to_string()
                    } else {
                        concat!(
                            "event: response.failed\r\n",
                            "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"insufficient_quota\",\"message\":\"main quota exhausted\"}}}\r\n",
                            "\r\n"
                        )
                        .to_string()
                    };
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        body,
                        None,
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(
                            mode,
                            RuntimeProxyBackendMode::HttpOnlySlowStream
                                | RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
                        )
                            .then_some(Duration::from_millis(100)),
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyPreviousResponseNotFoundAfterCommit
                    ) && runtime_proxy_backend_is_owned_continuation(
                        "second-account",
                        previous_response_id.as_deref(),
                    ) =>
                {
                    let next_response_id = runtime_proxy_backend_next_response_id(
                        previous_response_id.as_deref(),
                    )
                    .expect("next response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        format!(
                            concat!(
                                "event: response.created\r\n",
                                "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n",
                                "event: response.output_text.delta\r\n",
                                "data: {{\"type\":\"response.output_text.delta\",\"response\":{{\"id\":\"{}\"}},\"delta\":\"hello\"}}\r\n",
                                "\r\n",
                                "event: response.failed\r\n",
                                "data: {{\"type\":\"response.failed\",\"response\":{{\"error\":{{\"code\":\"previous_response_not_found\",\"message\":\"Previous response with id '{}' not found.\",\"param\":\"previous_response_id\"}}}}}}\r\n",
                                "\r\n"
                            ),
                            next_response_id,
                            next_response_id,
                            previous_response_id.as_deref().unwrap_or_default(),
                        ),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyPreviousResponseNeedsTurnState
                            | RuntimeProxyBackendMode::HttpOnlySseHeadersArrayTurnState
                    ) && runtime_proxy_backend_is_owned_continuation(
                        "second-account",
                        previous_response_id.as_deref(),
                    )
                        && turn_state.as_deref() != Some("turn-second") =>
                {
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string(),
                        Some("turn-second".to_string()),
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlySseHeadersArrayTurnState)
                        && previous_response_id.is_none() =>
                {
                    let response_id =
                        runtime_proxy_backend_initial_response_id_for_account("second-account")
                            .expect("second-account response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        format!(
                            concat!(
                                "event: response.created\r\n",
                                "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\",\"headers\":[[\"x-codex-turn-state\",\"turn-second\"]]}}}}\r\n",
                                "\r\n",
                                "event: response.completed\r\n",
                                "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n"
                            ),
                            response_id,
                            response_id
                        )
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup
                    ) && body_json
                        .get("stream")
                        .and_then(serde_json::Value::as_bool)
                        != Some(true) =>
                {
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
                        serde_json::json!({
                            "error": {
                                "message": "{\"detail\":\"Stream must be set to true\"}"
                            }
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup
                    ) && previous_response_id.as_deref() == Some("resp_ws_followup_1") =>
                {
                    (
                        "HTTP/1.1 200 OK",
                        "application/json",
                        serde_json::json!({
                            "id": "resp_ws_followup_2",
                            "object": "response",
                            "status": "completed",
                            "usage": {
                                "input_tokens": 18,
                                "output_tokens": 9
                            },
                            "tool_usage": {
                                "web_search": {
                                    "num_requests": 1
                                }
                            },
                            "output": [
                                {
                                    "type": "message",
                                    "content": [
                                        {
                                            "type": "output_text",
                                            "text": "Ringkasan terbaru reksadana Indonesia."
                                        }
                                    ]
                                }
                            ]
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup
                    ) =>
                {
                    (
                        "HTTP/1.1 200 OK",
                        "application/json",
                        serde_json::json!({
                            "id": "resp_ws_followup_1",
                            "object": "response",
                            "status": "completed",
                            "usage": {
                                "input_tokens": 12,
                                "output_tokens": 6
                            },
                            "tool_usage": {
                                "web_search": {
                                    "num_requests": 1
                                }
                            },
                            "output": [
                                {
                                    "type": "web_search_call",
                                    "id": "ws_1",
                                    "status": "completed",
                                    "action": {
                                        "type": "search",
                                        "queries": ["berita terbaru reksadana Indonesia"],
                                        "sources": [
                                            {
                                                "type": "url",
                                                "url": "https://example.com/news",
                                                "title": "Example News"
                                            }
                                        ]
                                    }
                                }
                            ]
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyAnthropicMcpStream) =>
                {
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        concat!(
                            "event: response.created\r\n",
                            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_mcp_1\"}}\r\n",
                            "\r\n",
                            "event: response.output_item.done\r\n",
                            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"mcp_call\",\"id\":\"mcp_1\",\"name\":\"list_files\",\"server_label\":\"local_fs\",\"arguments\":\"{\\\"path\\\":\\\"/workspace\\\"}\",\"output\":\"README.md\\nsrc/main.rs\"}}\r\n",
                            "\r\n",
                            "event: response.completed\r\n",
                            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_mcp_1\",\"usage\":{\"input_tokens\":14,\"output_tokens\":8},\"output\":[{\"type\":\"mcp_call\",\"id\":\"mcp_1\",\"name\":\"list_files\",\"server_label\":\"local_fs\",\"arguments\":\"{\\\"path\\\":\\\"/workspace\\\"}\",\"output\":\"README.md\\nsrc/main.rs\"}]}}\r\n",
                            "\r\n"
                        )
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyPreviousResponseToolContextMissing
                    )
                        && previous_response_id.is_some()
                        && request_body_contains_only_function_call_output(&request_body)
                        && request_body_contains_session_id(&request_body) =>
                {
                    let (call_id, item_label) = body_json
                        .get("input")
                        .and_then(serde_json::Value::as_array)
                        .and_then(|input| input.first())
                        .map(|item| {
                            let call_id = item
                                .get("call_id")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("call_missing");
                            let item_label = item
                                .get("type")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("tool_call_output")
                                .replace('_', " ");
                            (call_id.to_string(), item_label)
                        })
                        .unwrap_or_else(|| {
                            ("call_missing".to_string(), "tool call output".to_string())
                        });
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "type": "invalid_request_error",
                                "message": format!(
                                    "No tool call found for {item_label} with call_id {call_id}."
                                ),
                                "param": "input",
                            }
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson)
                        && runtime_proxy_backend_is_owned_continuation(
                            "second-account",
                            previous_response_id.as_deref(),
                        ) =>
                {
                    let next_response_id = runtime_proxy_backend_next_response_id(
                        previous_response_id.as_deref(),
                    )
                    .expect("next response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "application/json",
                        serde_json::json!({
                            "id": next_response_id,
                            "object": "response",
                            "status": "completed",
                            "output": []
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account"
                    if runtime_proxy_backend_is_owned_continuation(
                        "second-account",
                        previous_response_id.as_deref(),
                    ) =>
                {
                    let next_response_id = runtime_proxy_backend_next_response_id(
                        previous_response_id.as_deref(),
                    )
                    .expect("next response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        format!(
                            concat!(
                                "event: response.created\r\n",
                                "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n",
                                "event: response.completed\r\n",
                                "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n"
                            ),
                            next_response_id.clone(),
                            next_response_id
                        )
                        .to_string(),
                        None,
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(
                            mode,
                            RuntimeProxyBackendMode::HttpOnlySlowStream
                                | RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
                        )
                            .then_some(Duration::from_millis(100)),
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyQuotaThenToolOutputFreshFallbackError
                    )
                        && previous_response_id.is_none()
                        && request_body_contains_only_function_call_output(&request_body)
                        && !request_body_contains_session_id(&request_body) =>
                {
                    let (call_id, item_label) = body_json
                        .get("input")
                        .and_then(serde_json::Value::as_array)
                        .and_then(|input| input.first())
                        .map(|item| {
                            let call_id = item
                                .get("call_id")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("call_missing");
                            let item_label = item
                                .get("type")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("tool_call_output")
                                .replace('_', " ");
                            (call_id.to_string(), item_label)
                        })
                        .unwrap_or_else(|| {
                            ("call_missing".to_string(), "tool call output".to_string())
                        });
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "type": "invalid_request_error",
                                "message": format!(
                                    "No tool call found for {item_label} with call_id {call_id}."
                                ),
                                "param": "input",
                            }
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "second-account" if previous_response_id.is_some() => (
                    "HTTP/1.1 400 Bad Request",
                    "application/json",
                    serde_json::json!({
                        "type": "error",
                        "status": 400,
                        "error": {
                            "code": "previous_response_not_found",
                            "message": format!(
                                "Previous response with id '{}' not found.",
                                previous_response_id.as_deref().unwrap_or_default()
                            ),
                            "param": "previous_response_id",
                        }
                    })
                    .to_string(),
                    None,
                    matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                        .then_some(Duration::from_millis(750)),
                    None,
                ),
                "second-account" => {
                    let response_id =
                        runtime_proxy_backend_initial_response_id_for_account("second-account")
                            .expect("second-account response id should exist");
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson) {
                        (
                            "HTTP/1.1 200 OK",
                            "application/json",
                            serde_json::json!({
                                "id": response_id,
                                "object": "response",
                                "status": "completed",
                                "output": []
                            })
                            .to_string(),
                            None,
                            None,
                            None,
                        )
                    } else {
                        (
                            "HTTP/1.1 200 OK",
                            "text/event-stream",
                            format!(
                                concat!(
                                    "event: response.created\r\n",
                                    "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n",
                                    "event: response.completed\r\n",
                                    "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n"
                                ),
                                response_id,
                                response_id
                            )
                            .to_string(),
                            None,
                            matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                                .then_some(Duration::from_millis(750)),
                            matches!(
                                mode,
                                RuntimeProxyBackendMode::HttpOnlySlowStream
                                    | RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
                            )
                                .then_some(Duration::from_millis(100)),
                        )
                    }
                }
                "third-account"
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson)
                        && runtime_proxy_backend_is_owned_continuation(
                            "third-account",
                            previous_response_id.as_deref(),
                        ) =>
                {
                    let next_response_id = runtime_proxy_backend_next_response_id(
                        previous_response_id.as_deref(),
                    )
                    .expect("next response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "application/json",
                        serde_json::json!({
                            "id": next_response_id,
                            "object": "response",
                            "status": "completed",
                            "output": []
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                "third-account"
                    if runtime_proxy_backend_is_owned_continuation(
                        "third-account",
                        previous_response_id.as_deref(),
                    ) =>
                {
                    let next_response_id = runtime_proxy_backend_next_response_id(
                        previous_response_id.as_deref(),
                    )
                    .expect("next response id should exist");
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        format!(
                            concat!(
                                "event: response.created\r\n",
                                "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n",
                                "event: response.completed\r\n",
                                "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                "\r\n"
                            ),
                            next_response_id.clone(),
                            next_response_id
                        )
                        .to_string(),
                        None,
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                            .then_some(Duration::from_millis(100)),
                    )
                }
                "third-account" if previous_response_id.is_some() => (
                    "HTTP/1.1 400 Bad Request",
                    "application/json",
                    serde_json::json!({
                        "type": "error",
                        "status": 400,
                        "error": {
                            "code": "previous_response_not_found",
                            "message": format!(
                                "Previous response with id '{}' not found.",
                                previous_response_id.as_deref().unwrap_or_default()
                            ),
                            "param": "previous_response_id",
                        }
                    })
                    .to_string(),
                    None,
                    matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                        .then_some(Duration::from_millis(750)),
                    None,
                ),
                "third-account" => {
                    let response_id =
                        runtime_proxy_backend_initial_response_id_for_account("third-account")
                            .expect("third-account response id should exist");
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson) {
                        (
                            "HTTP/1.1 200 OK",
                            "application/json",
                            serde_json::json!({
                                "id": response_id,
                                "object": "response",
                                "status": "completed",
                                "output": []
                            })
                            .to_string(),
                            None,
                            None,
                            None,
                        )
                    } else {
                        (
                            "HTTP/1.1 200 OK",
                            "text/event-stream",
                            format!(
                                concat!(
                                    "event: response.created\r\n",
                                    "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n",
                                    "event: response.completed\r\n",
                                    "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n"
                                ),
                                response_id,
                                response_id
                            )
                            .to_string(),
                            None,
                            matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                                .then_some(Duration::from_millis(750)),
                            matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                            .then_some(Duration::from_millis(100)),
                        )
                    }
                }
                "fourth-account" | "fifth-account" => {
                    let response_id =
                        runtime_proxy_backend_initial_response_id_for_account(account_id.as_str())
                            .expect("late-pool account response id should exist");
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson) {
                        (
                            "HTTP/1.1 200 OK",
                            "application/json",
                            serde_json::json!({
                                "id": response_id,
                                "object": "response",
                                "status": "completed",
                                "output": []
                            })
                            .to_string(),
                            None,
                            None,
                            None,
                        )
                    } else {
                        (
                            "HTTP/1.1 200 OK",
                            "text/event-stream",
                            format!(
                                concat!(
                                    "event: response.created\r\n",
                                    "data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n",
                                    "event: response.completed\r\n",
                                    "data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"{}\"}}}}\r\n",
                                    "\r\n"
                                ),
                                response_id,
                                response_id
                            ),
                            None,
                            matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                                .then_some(Duration::from_millis(750)),
                            matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                                .then_some(Duration::from_millis(100)),
                        )
                    }
                }
                _ => (
                    "HTTP/1.1 200 OK",
                    "text/event-stream",
                    concat!(
                        "event: response.failed\r\n",
                        "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"rate_limit_exceeded\",\"message\":\"unexpected account\"}}}\r\n",
                        "\r\n"
                        )
                        .to_string(),
                    None,
                    matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                        .then_some(Duration::from_millis(750)),
                    matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlySlowStream
                            | RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
                    )
                        .then_some(Duration::from_millis(100)),
                ),
            }
        } else if path.ends_with("/backend-api/codex/responses/compact") {
            responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .push(account_id.clone());
            responses_headers
                .lock()
                .expect("responses_headers poisoned")
                .push(captured_headers);
            responses_bodies
                .lock()
                .expect("responses_bodies poisoned")
                .push(request_body);
            let previous_response_id = request_previous_response_id(&request);
            match (account_id.as_str(), mode) {
                (
                    "main-account",
                    RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage
                    | RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth,
                ) => (
                    "HTTP/1.1 429 Too Many Requests",
                    "application/json",
                    serde_json::json!({
                        "error": {
                            "type": "usage_limit_reached",
                            "message": "The usage limit has been reached",
                            "plan_type": "team",
                            "resets_at": 1775183113_i64,
                            "eligible_promo": serde_json::Value::Null,
                            "resets_in_seconds": 259149
                        }
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                ("second-account", RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "output": []
                    })
                    .to_string(),
                    Some("compact-turn-second".to_string()),
                    None,
                    None,
                ),
                ("main-account", RuntimeProxyBackendMode::HttpOnlyCompactOverloaded) => (
                    "HTTP/1.1 500 Internal Server Error",
                    "application/json",
                    serde_json::json!({
                        "error": {
                            "message": "backend under high demand"
                        },
                        "status": 500
                    })
                    .to_string(),
                    None,
                    None,
                    None,
                ),
                (_, RuntimeProxyBackendMode::HttpOnlyCompactPreviousResponseNotFound)
                    if previous_response_id.is_some() =>
                {
                    (
                        "HTTP/1.1 400 Bad Request",
                        "application/json",
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "type": "invalid_request_error",
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string(),
                        None,
                        None,
                        None,
                    )
                }
                ("second-account", RuntimeProxyBackendMode::HttpOnlyCompactOverloaded) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "output": []
                    })
                    .to_string(),
                    Some("compact-turn-second".to_string()),
                    None,
                    None,
                ),
                ("third-account", RuntimeProxyBackendMode::HttpOnlyCompactOverloaded) => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "output": []
                    })
                    .to_string(),
                    Some("compact-turn-third".to_string()),
                    None,
                    None,
                ),
                (_, RuntimeProxyBackendMode::HttpOnlyLargeCompactResponse) => {
                    let body = serde_json::json!({
                        "output": [
                            {
                                "type": "message",
                                "content": "x".repeat(RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES + 1024)
                            }
                        ]
                    })
                    .to_string();
                    (
                        "HTTP/1.1 200 OK",
                        "application/json",
                        body,
                        Some("compact-turn-main".to_string()),
                        None,
                        None,
                    )
                }
                _ => (
                    "HTTP/1.1 200 OK",
                    "application/json",
                    serde_json::json!({
                        "output": []
                    })
                    .to_string(),
                    Some(
                        match account_id.as_str() {
                            "main-account" => "compact-turn-main",
                            "second-account" => "compact-turn-second",
                            "third-account" => "compact-turn-third",
                            _ => "compact-turn-unknown",
                        }
                        .to_string(),
                    ),
                    None,
                    None,
                ),
            }
        } else {
            (
                "HTTP/1.1 404 Not Found",
                "application/json",
                serde_json::json!({ "error": "not_found" }).to_string(),
                None,
                None,
                None,
            )
        };

    let mut headers = format!(
        "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len(),
    );
    if let Some(turn_state) = response_turn_state.as_deref() {
        headers.push_str(&format!("x-codex-turn-state: {turn_state}\r\n"));
    }
    headers.push_str("\r\n");
    let _ = stream.write_all(headers.as_bytes());
    let _ = stream.flush();
    if let Some(delay) = initial_body_stall {
        thread::sleep(delay);
    }
    if content_type == "text/event-stream"
        && matches!(mode, RuntimeProxyBackendMode::HttpOnlyResetAfterFirstChunk)
        && account_id == "second-account"
    {
        let reset_chunk = body
            .split("\r\n\r\n")
            .next()
            .map(|chunk| format!("{chunk}\r\n\r\n"))
            .unwrap_or_else(|| body.clone());
        let _ = stream.write_all(reset_chunk.as_bytes());
        let _ = stream.flush();
        return;
    }
    if content_type == "text/event-stream"
        && matches!(
            mode,
            RuntimeProxyBackendMode::HttpOnlyPreviousResponseNotFoundAfterCommit
        )
        && account_id == "second-account"
    {
        for (index, event) in body.split("\r\n\r\n").filter(|event| !event.is_empty()).enumerate() {
            let _ = stream.write_all(format!("{event}\r\n\r\n").as_bytes());
            let _ = stream.flush();
            if index == 1 {
                thread::sleep(Duration::from_millis(50));
            }
        }
        return;
    }
    if content_type == "text/event-stream"
        && let Some(delay) = chunk_delay
    {
        let body_bytes = body.as_bytes();
        let chunk_size = body_bytes.len().max(1).div_ceil(4);
        for (index, chunk) in body_bytes.chunks(chunk_size).enumerate() {
            let _ = stream.write_all(chunk);
            let _ = stream.flush();
            if matches!(
                mode,
                RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
            ) && account_id == "second-account"
                && index >= 1
            {
                thread::sleep(Duration::from_millis(
                    runtime_proxy_stream_idle_timeout_ms() + 100,
                ));
                return;
            }
            if index + 1 < body_bytes.chunks(chunk_size).len() {
                thread::sleep(delay);
            }
        }
    } else {
        let _ = stream.write_all(body.as_bytes());
        let _ = stream.flush();
    }
}

fn runtime_proxy_backend_is_websocket_upgrade(stream: &TcpStream) -> bool {
    let mut buffer = [0_u8; 2048];
    let Ok(read) = stream.peek(&mut buffer) else {
        return false;
    };
    if read == 0 {
        return false;
    }
    let request = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
    request.contains("upgrade: websocket")
}

#[allow(clippy::result_large_err)]
fn handle_runtime_proxy_backend_websocket(
    stream: TcpStream,
    responses_accounts: &Arc<Mutex<Vec<String>>>,
    responses_headers: &Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    websocket_requests: &Arc<Mutex<Vec<String>>>,
    mode: RuntimeProxyBackendMode,
) {
    let account_id = Arc::new(Mutex::new(String::new()));
    let request_turn_state = Arc::new(Mutex::new(None::<String>));
    let captured_headers = Arc::new(Mutex::new(BTreeMap::new()));
    let captured_account_id = Arc::clone(&account_id);
    let captured_turn_state = Arc::clone(&request_turn_state);
    let captured_headers_for_callback = Arc::clone(&captured_headers);
    let callback = move |req: &tungstenite::handshake::server::Request,
                         response: tungstenite::handshake::server::Response| {
        if let Some(value) = req
            .headers()
            .get("ChatGPT-Account-Id")
            .and_then(|value| value.to_str().ok())
        {
            *captured_account_id
                .lock()
                .expect("captured_account_id poisoned") = value.to_string();
        }
        if let Some(value) = req
            .headers()
            .get("x-codex-turn-state")
            .and_then(|value| value.to_str().ok())
        {
            *captured_turn_state
                .lock()
                .expect("captured_turn_state poisoned") = Some(value.to_string());
        }
        let mut headers = captured_headers_for_callback
            .lock()
            .expect("captured_headers poisoned");
        headers.clear();
        for (name, value) in req.headers() {
            if let Ok(value) = value.to_str() {
                headers.insert(name.as_str().to_ascii_lowercase(), value.to_string());
            }
        }
        let mut response = response;
        if matches!(
            mode,
            RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
                | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
        ) && req
            .headers()
            .get("ChatGPT-Account-Id")
            .and_then(|value| value.to_str().ok())
            == Some("second-account")
        {
            response.headers_mut().insert(
                tungstenite::http::header::HeaderName::from_static("x-codex-turn-state"),
                tungstenite::http::HeaderValue::from_static("turn-second"),
            );
        }
        Ok(response)
    };
    let mut websocket = tungstenite::accept_hdr(stream, callback)
        .expect("backend websocket handshake should succeed");
    let account_id = account_id.lock().expect("account_id poisoned").clone();
    let mut effective_turn_state = request_turn_state
        .lock()
        .expect("request_turn_state poisoned")
        .clone();
    responses_accounts
        .lock()
        .expect("responses_accounts poisoned")
        .push(account_id.clone());
    responses_headers
        .lock()
        .expect("responses_headers poisoned")
        .push(
            captured_headers
                .lock()
                .expect("captured_headers poisoned")
                .clone(),
        );
    let response_turn_state = (matches!(
        mode,
        RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
            | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
            | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
    ) && account_id == "second-account")
        .then(|| "turn-second".to_string());
    let mut request_count = 0usize;

    loop {
        let request = match websocket.read() {
            Ok(WsMessage::Text(text)) => text.to_string(),
            Ok(WsMessage::Ping(payload)) => {
                websocket
                    .send(WsMessage::Pong(payload))
                    .expect("backend websocket pong should be sent");
                continue;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => continue,
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => break,
            Ok(other) => panic!("backend websocket expects text requests, got {other:?}"),
            Err(err) => panic!("backend websocket failed to read request: {err}"),
        };
        websocket_requests
            .lock()
            .expect("websocket_requests poisoned")
            .push(request.clone());
        request_count += 1;
        let previous_response_id = runtime_request_previous_response_id_from_text(&request);

        if matches!(mode, RuntimeProxyBackendMode::WebsocketRealtimeSideband) {
            let payload = match request_count {
                1 => serde_json::json!({
                    "type": "session.updated",
                    "session": {
                        "id": "sess-realtime",
                        "instructions": "backend prompt"
                    }
                }),
                2 => serde_json::json!({
                    "type": "conversation.item.added",
                    "item": {
                        "id": "item-realtime"
                    }
                }),
                3 => serde_json::json!({
                    "type": "response.done",
                    "response": {
                        "id": "resp-realtime"
                    }
                }),
                _ => serde_json::json!({
                    "type": "error",
                    "error": {
                        "message": "unexpected realtime sideband request"
                    }
                }),
            };
            websocket
                .send(WsMessage::Text(payload.to_string().into()))
                .expect("realtime sideband response should be sent");
            continue;
        }

        match account_id.as_str() {
            "main-account" => {
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude
                        | RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude
                ) && request_count == 1
                {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.created",
                                "response": {
                                    "id": "resp-main"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.created should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.completed",
                                "response": {
                                    "id": "resp-main"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.completed should be sent");
                    continue;
                }
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude
                        | RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude
                ) && request_count >= 2
                {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.in_progress"
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.in_progress should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.output_item.added",
                                "item": {
                                    "type": "message",
                                    "id": "msg-main"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.output_item.added should be sent");
                    let delayed_error = if matches!(
                        mode,
                        RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude
                    ) {
                        serde_json::json!({
                            "type": "response.failed",
                            "status": 429,
                            "error": {
                                "message": "Selected model is at capacity. Please try a different model."
                            }
                        })
                    } else {
                        serde_json::json!({
                            "type": "response.failed",
                            "status": 429,
                            "error": {
                                "type": "usage_limit_reached",
                                "message": "The usage limit has been reached"
                            }
                        })
                    };
                    websocket
                        .send(WsMessage::Text(delayed_error.to_string().into()))
                        .expect("delayed retryable error should be sent");
                    continue;
                }
                let immediate_error = if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketOverloaded
                ) {
                    serde_json::json!({
                        "type": "response.failed",
                        "status": 429,
                        "error": {
                            "message": "Selected model is at capacity. Please try a different model."
                        }
                    })
                } else {
                    serde_json::json!({
                        "type": "error",
                        "status": 429,
                        "error": {
                            "code": "insufficient_quota",
                            "message": "main quota exhausted",
                        }
                    })
                };
                websocket
                    .send(WsMessage::Text(immediate_error.to_string().into()))
                    .expect("retryable error should be sent");
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterCommit
                ) && runtime_proxy_backend_is_owned_continuation(
                    "second-account",
                    previous_response_id.as_deref(),
                ) =>
            {
                let next_response_id =
                    runtime_proxy_backend_next_response_id(previous_response_id.as_deref())
                        .expect("next response id should exist");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": next_response_id.clone()
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.created should be sent");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.output_text.delta",
                            "response": {
                                "id": next_response_id
                            },
                            "delta": "hello"
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.output_text.delta should be sent");
                thread::sleep(Duration::from_millis(50));
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.failed",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("post-commit previous_response_not_found should be sent");
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                        | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
                        | RuntimeProxyBackendMode::WebsocketPreviousResponseMissingWithoutTurnState
                        | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
                ) && runtime_proxy_backend_is_owned_continuation(
                    "second-account",
                    previous_response_id.as_deref(),
                ) && effective_turn_state.as_deref() != Some("turn-second") =>
            {
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("previous_response_not_found should be sent");
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketOwnedToolOutputNeedsSessionReplay
                ) && runtime_proxy_backend_is_owned_continuation(
                    "second-account",
                    previous_response_id.as_deref(),
                ) && request_body_contains_only_function_call_output(&request)
                    && request_body_contains_session_id(&request)
                    && effective_turn_state.is_none() =>
            {
                let (call_id, item_label) = serde_json::from_str::<serde_json::Value>(&request)
                    .ok()
                    .and_then(|body_json| {
                        body_json
                            .get("input")
                            .and_then(serde_json::Value::as_array)
                            .and_then(|input| input.first())
                            .map(|item| {
                                let call_id = item
                                    .get("call_id")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("call_missing");
                                let item_label = item
                                    .get("type")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("tool_call_output")
                                    .replace('_', " ");
                                (call_id.to_string(), item_label)
                            })
                    })
                    .unwrap_or_else(|| {
                        ("call_missing".to_string(), "tool call output".to_string())
                    });
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "type": "invalid_request_error",
                                "message": format!(
                                    "No tool call found for {item_label} with call_id {call_id}."
                                ),
                                "param": "input",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("tool context regression should be sent");
            }
            "second-account"
                if runtime_proxy_backend_is_owned_continuation(
                    "second-account",
                    previous_response_id.as_deref(),
                ) =>
            {
                let next_response_id =
                    runtime_proxy_backend_next_response_id(previous_response_id.as_deref())
                        .expect("next response id should exist");
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketReuseOwnedPreviousResponseSilentHang
                        | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
                ) && request_count == 2
                {
                    thread::sleep(Duration::from_millis(
                        runtime_proxy_websocket_precommit_progress_timeout_ms() + 100,
                    ));
                    break;
                }
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
                ) && request_count == 2
                {
                    thread::sleep(Duration::from_millis(
                        runtime_proxy_websocket_precommit_progress_timeout_ms() + 100,
                    ));
                    break;
                }
                if matches!(mode, RuntimeProxyBackendMode::WebsocketTopLevelResponseId) {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.output_item.done",
                                "response_id": next_response_id.clone(),
                                "item": {
                                    "id": "item-second"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.output_item.done should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.completed",
                                "response_id": next_response_id
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.completed should be sent");
                } else {
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.created",
                                "response": {
                                    "id": next_response_id.clone()
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.created should be sent");
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.completed",
                                "response": {
                                    "id": next_response_id
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.completed should be sent");
                }
            }
            "second-account" if previous_response_id.is_some() => {
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("previous_response_not_found should be sent");
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketPreviousResponseMissingWithoutTurnState
                ) && previous_response_id.is_none()
                    && request_body_contains_only_function_call_output(&request)
                    && !request_body_contains_session_id(&request) =>
            {
                let (call_id, item_label) = serde_json::from_str::<serde_json::Value>(&request)
                    .ok()
                    .and_then(|body_json| {
                        body_json
                            .get("input")
                            .and_then(serde_json::Value::as_array)
                            .and_then(|input| input.first())
                            .map(|item| {
                                let call_id = item
                                    .get("call_id")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("call_missing");
                                let item_label = item
                                    .get("type")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("tool_call_output")
                                    .replace('_', " ");
                                (call_id.to_string(), item_label)
                            })
                    })
                    .unwrap_or_else(|| {
                        ("call_missing".to_string(), "tool call output".to_string())
                    });
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "type": "invalid_request_error",
                                "message": format!(
                                    "No tool call found for {item_label} with call_id {call_id}."
                                ),
                                "param": "input",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("tool output mismatch should be sent");
            }
            "second-account" => {
                let response_id =
                    runtime_proxy_backend_initial_response_id_for_account("second-account")
                        .expect("second-account response id should exist");
                if matches!(mode, RuntimeProxyBackendMode::WebsocketReuseSilentHang)
                    && request_count == 2
                {
                    thread::sleep(Duration::from_millis(
                        runtime_proxy_stream_idle_timeout_ms() + 100,
                    ));
                    break;
                }
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketReusePrecommitHoldStall
                ) && request_count == 2
                {
                    let hold_delay = Duration::from_millis(
                        runtime_proxy_websocket_precommit_progress_timeout_ms() / 2 + 10,
                    );
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.created",
                                "response": {
                                    "id": response_id
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.created should be sent");
                    thread::sleep(hold_delay);
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.in_progress",
                                "response": {
                                    "id": response_id
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.in_progress should be sent");
                    thread::sleep(hold_delay);
                    websocket
                        .send(WsMessage::Text(
                            serde_json::json!({
                                "type": "response.output_item.added",
                                "response": {
                                    "id": response_id
                                },
                                "item": {
                                    "type": "message",
                                    "id": "msg-second"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("response.output_item.added should be sent");
                    continue;
                }
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.created should be sent");
                if matches!(mode, RuntimeProxyBackendMode::WebsocketCloseMidTurn) {
                    let _ = websocket.close(None);
                    break;
                }
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.completed",
                            "response": {
                                "id": response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.completed should be sent");
            }
            "third-account"
                if runtime_proxy_backend_is_owned_continuation(
                    "third-account",
                    previous_response_id.as_deref(),
                ) =>
            {
                let next_response_id =
                    runtime_proxy_backend_next_response_id(previous_response_id.as_deref())
                        .expect("next response id should exist");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": next_response_id.clone()
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.created should be sent");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.completed",
                            "response": {
                                "id": next_response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.completed should be sent");
            }
            "third-account" if previous_response_id.is_some() => {
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 400,
                            "error": {
                                "code": "previous_response_not_found",
                                "message": format!(
                                    "Previous response with id '{}' not found.",
                                    previous_response_id.as_deref().unwrap_or_default()
                                ),
                                "param": "previous_response_id",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("previous_response_not_found should be sent");
            }
            "third-account" => {
                let response_id =
                    runtime_proxy_backend_initial_response_id_for_account("third-account")
                        .expect("third-account response id should exist");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.created should be sent");
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.completed",
                            "response": {
                                "id": response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.completed should be sent");
            }
            _ => {
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 429,
                            "error": {
                                "code": "rate_limit_exceeded",
                                "message": "unexpected account",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("unexpected account error should be sent");
            }
        }

        if response_turn_state.is_some() {
            effective_turn_state = response_turn_state.clone();
        }
    }
}

fn read_http_request(stream: &mut TcpStream) -> Option<String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let mut buffer = [0_u8; 1024];
    let mut request = Vec::new();

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                request.extend_from_slice(&buffer[..read]);
                if let Some(header_end) =
                    request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let header_len = header_end + 4;
                    let header_text = String::from_utf8_lossy(&request[..header_len]);
                    let content_length = request_header(&header_text, "Content-Length")
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(0);
                    while request.len() < header_len + content_length {
                        match stream.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(read) => request.extend_from_slice(&buffer[..read]),
                            Err(err)
                                if matches!(
                                    err.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                                ) =>
                            {
                                break;
                            }
                            Err(_) => return None,
                        }
                    }
                    break;
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(_) => return None,
        }
    }

    (!request.is_empty()).then(|| String::from_utf8_lossy(&request).into_owned())
}

fn request_header(request: &str, header_name: &str) -> Option<String> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case(header_name) {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

fn request_headers_map(request: &str) -> BTreeMap<String, String> {
    request
        .lines()
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect()
}

fn request_previous_response_id(request: &str) -> Option<String> {
    let body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default();
    runtime_request_previous_response_id_from_text(body)
}

fn request_body_contains_only_function_call_output(request_body: &str) -> bool {
    let Ok(body) = serde_json::from_str::<serde_json::Value>(request_body) else {
        return false;
    };
    let Some(input) = body.get("input").and_then(serde_json::Value::as_array) else {
        return false;
    };
    !input.is_empty()
        && input.iter().all(|item| {
            let Some(object) = item.as_object() else {
                return false;
            };
            let item_type = object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let call_id = object
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            call_id.is_some() && item_type.ends_with("_call_output")
        })
}

fn request_body_contains_session_id(request_body: &str) -> bool {
    let Ok(body) = serde_json::from_str::<serde_json::Value>(request_body) else {
        return false;
    };
    body.get("session_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            body.get("client_metadata")
                .and_then(|metadata| metadata.get("session_id"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn runtime_proxy_usage_body(email: &str) -> String {
    runtime_proxy_usage_body_with_remaining(email, 95, 95)
}

fn runtime_proxy_usage_body_with_remaining(
    email: &str,
    five_hour_remaining: i64,
    weekly_remaining: i64,
) -> String {
    serde_json::json!({
        "email": email,
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": (100 - five_hour_remaining).clamp(0, 100),
                "reset_at": future_epoch(18_000),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": (100 - weekly_remaining).clamp(0, 100),
                "reset_at": future_epoch(604_800),
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}

fn future_epoch(offset_seconds: i64) -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64
        + offset_seconds
}

fn write_auth_json(path: &Path, account_id: &str) {
    write_auth_json_with_tokens(path, "test-token", account_id, None, None);
}

fn write_auth_json_with_tokens(
    path: &Path,
    access_token: &str,
    account_id: &str,
    refresh_token: Option<&str>,
    last_refresh: Option<&str>,
) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create auth parent dir");
    }
    let mut auth_json = serde_json::json!({
        "tokens": {
            "access_token": access_token,
            "account_id": account_id,
        }
    });
    if let Some(refresh_token) = refresh_token {
        auth_json["tokens"]["refresh_token"] = serde_json::Value::String(refresh_token.to_string());
    }
    if let Some(last_refresh) = last_refresh {
        auth_json["last_refresh"] = serde_json::Value::String(last_refresh.to_string());
    }
    fs::write(path, auth_json.to_string()).expect("failed to write auth.json");
}

fn fake_jwt_with_exp(exp: i64) -> String {
    fake_jwt_with_exp_and_account_id(exp, "")
}

fn fake_jwt_with_exp_and_account_id(exp: i64, account_id: &str) -> String {
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "exp": exp,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account_id,
            },
        })
        .to_string(),
    );
    format!("{header}.{payload}.signature")
}

#[derive(Clone, Copy)]
enum TokenAwareServerMode {
    RuntimeStatus,
    Usage,
}

struct TokenAwareServer {
    listen_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    auth_headers: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl TokenAwareServer {
    fn start(
        mode: TokenAwareServerMode,
        expected_token: &str,
        expected_account_id: &str,
        success_body: String,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind token-aware server");
        let listen_addr = listener
            .local_addr()
            .expect("failed to read token-aware server address");
        listener
            .set_nonblocking(true)
            .expect("failed to set token-aware server nonblocking");

        let shutdown = Arc::new(AtomicBool::new(false));
        let auth_headers = Arc::new(Mutex::new(Vec::new()));
        let shutdown_flag = Arc::clone(&shutdown);
        let auth_headers_flag = Arc::clone(&auth_headers);
        let expected_token = expected_token.to_string();
        let expected_account_id = expected_account_id.to_string();
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let Some(request) = read_http_request(&mut stream) else {
                            continue;
                        };
                        let path = request
                            .lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .unwrap_or("/");
                        let authorization = request_header(&request, "Authorization")
                            .unwrap_or_else(|| "<missing>".to_string());
                        let account_id = request_header(&request, "ChatGPT-Account-Id")
                            .unwrap_or_else(|| "<missing>".to_string());
                        auth_headers_flag
                            .lock()
                            .expect("auth_headers poisoned")
                            .push(authorization.clone());

                        let path_matches = match mode {
                            TokenAwareServerMode::RuntimeStatus => {
                                path.ends_with("/backend-api/status")
                            }
                            TokenAwareServerMode::Usage => {
                                path.ends_with("/backend-api/wham/usage")
                                    || path.ends_with("/api/codex/usage")
                            }
                        };
                        let authorized = authorization == format!("Bearer {expected_token}")
                            && account_id == expected_account_id;
                        let (status_line, body) = if path_matches && authorized {
                            ("HTTP/1.1 200 OK", success_body.clone())
                        } else if path_matches {
                            (
                                "HTTP/1.1 401 Unauthorized",
                                serde_json::json!({ "error": "unauthorized" }).to_string(),
                            )
                        } else {
                            (
                                "HTTP/1.1 404 Not Found",
                                serde_json::json!({ "error": "not_found" }).to_string(),
                            )
                        };
                        let response = format!(
                            "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            listen_addr,
            shutdown,
            auth_headers,
            thread: Some(thread),
        }
    }

    fn start_runtime_status(expected_token: &str, expected_account_id: &str) -> Self {
        Self::start(
            TokenAwareServerMode::RuntimeStatus,
            expected_token,
            expected_account_id,
            serde_json::json!({
                "status": "ok",
                "account_id": expected_account_id,
            })
            .to_string(),
        )
    }

    fn start_usage(expected_token: &str, expected_account_id: &str, email: &str) -> Self {
        Self::start(
            TokenAwareServerMode::Usage,
            expected_token,
            expected_account_id,
            runtime_proxy_usage_body(email),
        )
    }

    fn base_url(&self) -> String {
        format!("http://{}/backend-api", self.listen_addr)
    }

    fn auth_headers(&self) -> Vec<String> {
        self.auth_headers
            .lock()
            .expect("auth_headers poisoned")
            .clone()
    }
}

impl Drop for TokenAwareServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.listen_addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct AuthRefreshServer {
    listen_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    request_bodies: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl AuthRefreshServer {
    fn start(access_token: &str, refresh_token: &str) -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("failed to bind auth refresh server");
        let listen_addr = listener
            .local_addr()
            .expect("failed to read auth refresh server address");
        listener
            .set_nonblocking(true)
            .expect("failed to set auth refresh server nonblocking");

        let shutdown = Arc::new(AtomicBool::new(false));
        let request_bodies = Arc::new(Mutex::new(Vec::new()));
        let shutdown_flag = Arc::clone(&shutdown);
        let request_bodies_flag = Arc::clone(&request_bodies);
        let access_token = access_token.to_string();
        let refresh_token = refresh_token.to_string();
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let Some(request) = read_http_request(&mut stream) else {
                            continue;
                        };
                        let body = request
                            .split_once("\r\n\r\n")
                            .map(|(_, body)| body.to_string())
                            .unwrap_or_default();
                        request_bodies_flag
                            .lock()
                            .expect("request_bodies poisoned")
                            .push(body);
                        let response_body = serde_json::json!({
                            "access_token": access_token,
                            "refresh_token": refresh_token,
                        })
                        .to_string();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            response_body.len(),
                            response_body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            listen_addr,
            shutdown,
            request_bodies,
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("http://{}/oauth/token", self.listen_addr)
    }

    fn request_bodies(&self) -> Vec<String> {
        self.request_bodies
            .lock()
            .expect("request_bodies poisoned")
            .clone()
    }
}

impl Drop for AuthRefreshServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.listen_addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn write_api_key_auth_json(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create auth parent dir");
    }
    fs::write(
        path,
        serde_json::json!({
            "auth_mode": "api_key",
            "OPENAI_API_KEY": "test-api-key",
        })
        .to_string(),
    )
    .expect("failed to write api-key auth.json");
}

#[test]
fn validates_profile_names() {
    assert!(validate_profile_name("alpha-1").is_ok());
    assert!(validate_profile_name("bad/name").is_err());
    assert!(validate_profile_name("bad space").is_err());
}

#[test]
fn recognizes_known_windows() {
    assert_eq!(window_label(Some(18_000)), "5h");
    assert_eq!(window_label(Some(604_800)), "weekly");
    assert_eq!(window_label(Some(2_592_000)), "monthly");
}

#[test]
fn blocks_when_main_window_is_exhausted() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(100),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    let blocked = collect_blocked_limits(&usage, false);
    assert_eq!(blocked.len(), 2);
    assert!(blocked[0].message.starts_with("5h exhausted until "));
    assert_eq!(blocked[1].message, "weekly quota unavailable");
}

#[test]
fn blocks_when_weekly_window_is_missing() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    let blocked = collect_blocked_limits(&usage, false);
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].message, "weekly quota unavailable");
}

#[test]
fn compact_window_format_uses_scale_of_100() {
    let window = UsageWindow {
        used_percent: Some(37),
        reset_at: None,
        limit_window_seconds: Some(18_000),
    };

    assert_eq!(format_window_status_compact(&window), "5h 63% left");
    assert!(format_window_status(&window).contains("63% left"));
    assert!(format_window_status(&window).contains("37% used"));
}

#[test]
fn main_reset_summary_lists_required_windows() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some(30),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    let summary = format_main_reset_summary(&usage);
    assert!(summary.starts_with("5h "));
    assert!(summary.contains(" | weekly "));
    assert!(summary.contains(&format_precise_reset_time(Some(1_700_000_000))));
}

#[test]
fn main_reset_summary_marks_missing_required_window() {
    let usage = UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some(20),
                reset_at: Some(1_700_000_000),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: None,
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    };

    assert_eq!(
        format_main_reset_summary(&usage),
        format!(
            "5h {} | weekly unavailable",
            format_precise_reset_time(Some(1_700_000_000))
        )
    );
}

#[test]
fn map_parallel_runs_jobs_concurrently_and_preserves_order() {
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(Mutex::new(0usize));
    let started = Instant::now();

    let output = map_parallel(vec![1, 2, 3, 4], {
        let active = Arc::clone(&active);
        let max_active = Arc::clone(&max_active);
        move |value| {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            {
                let mut seen_max = max_active.lock().expect("max_active poisoned");
                *seen_max = (*seen_max).max(current);
            }

            thread::sleep(Duration::from_millis(50));
            active.fetch_sub(1, Ordering::SeqCst);
            value * 10
        }
    });

    assert_eq!(output, vec![10, 20, 30, 40]);
    assert!(
        *max_active.lock().expect("max_active poisoned") >= 2,
        "parallel worker count never exceeded one"
    );
    assert!(
        started.elapsed() < Duration::from_millis(150),
        "parallel execution took too long: {:?}",
        started.elapsed()
    );
}

#[test]
fn ready_profile_ranking_prefers_soon_recovering_weekly_capacity() {
    let candidates = vec![
        ReadyProfileCandidate {
            name: "slow".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "fast".to_string(),
            usage: usage_with_main_windows(80, 18_000, 80, 86_400),
            order_index: 1,
            preferred: false,
            provider_priority: 0,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let mut ranked = candidates.clone();
    ranked.sort_by_key(ready_profile_sort_key);
    assert_eq!(ranked[0].name, "fast");
}

#[test]
fn runtime_probe_cache_freshness_distinguishes_fresh_stale_and_expired() {
    let now = Local::now().timestamp();
    let fresh = RuntimeProfileProbeCacheEntry {
        checked_at: now,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
    };
    let stale = RuntimeProfileProbeCacheEntry {
        checked_at: now - (RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS + 1),
        auth: fresh.auth.clone(),
        result: fresh.result.clone(),
    };
    let expired = RuntimeProfileProbeCacheEntry {
        checked_at: now - (RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS + 1),
        auth: fresh.auth.clone(),
        result: fresh.result.clone(),
    };

    assert_eq!(
        runtime_profile_probe_cache_freshness(&fresh, now),
        RuntimeProbeCacheFreshness::Fresh
    );
    assert_eq!(
        runtime_profile_probe_cache_freshness(&stale, now),
        RuntimeProbeCacheFreshness::StaleUsable
    );
    assert_eq!(
        runtime_profile_probe_cache_freshness(&expired, now),
        RuntimeProbeCacheFreshness::Expired
    );
}

#[test]
fn startup_probe_refresh_targets_current_then_stale_or_missing_profiles() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    let fourth_home = temp_dir.path.join("homes/fourth");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");
    write_auth_json(&fourth_home.join("auth.json"), "fourth-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "fourth".to_string(),
                ProfileEntry {
                    codex_home: fourth_home,
                    managed: true,
                    email: Some("fourth@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let probe_cache = BTreeMap::from([(
        "fourth".to_string(),
        RuntimeProfileProbeCacheEntry {
            checked_at: now,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 18_000, 90, 604_800)),
        },
    )]);
    let usage_snapshots = BTreeMap::from([(
        "second".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: now,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 85,
            five_hour_reset_at: now + 18_000,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 92,
            weekly_reset_at: now + 604_800,
        },
    )]);

    assert_eq!(
        runtime_profiles_needing_startup_probe_refresh(
            &state,
            "third",
            &probe_cache,
            &usage_snapshots,
            now,
        ),
        vec!["third".to_string(), "main".to_string()]
    );
}

#[test]
fn startup_probe_refresh_warms_current_profiles_when_snapshots_are_empty() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    assert_eq!(
        runtime_profiles_needing_startup_probe_refresh(
            &state,
            "second",
            &BTreeMap::new(),
            &BTreeMap::new(),
            Local::now().timestamp(),
        ),
        vec![
            "second".to_string(),
            "third".to_string(),
            "main".to_string()
        ]
    );
}


include!("main_internal/runtime_proxy_selection_and_pressure.rs");

#[test]
fn version_is_newer_compares_semver_like_versions() {
    assert!(version_is_newer("0.2.47", "0.2.46"));
    assert!(version_is_newer("1.0.0", "0.9.9"));
    assert!(!version_is_newer("0.2.46", "0.2.46"));
    assert!(!version_is_newer("0.2.45", "0.2.46"));
}

#[test]
fn prodex_update_command_prefers_cargo_for_native_installations() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    assert_eq!(
        prodex_update_command_for_version("0.2.99"),
        "cargo install prodex --force --version 0.2.99"
    );
}

#[test]
fn prodex_update_command_prefers_npm_for_npm_installations() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "@christiandoxa/prodex");
    assert_eq!(
        prodex_update_command_for_version("0.2.99"),
        "npm install -g @christiandoxa/prodex@0.2.99 or npm install -g @christiandoxa/prodex@latest"
    );
}

#[test]
fn current_prodex_release_source_is_npm_when_wrapped_by_npm() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "@christiandoxa/prodex");
    assert_eq!(current_prodex_release_source(), ProdexReleaseSource::Npm);
}

#[test]
fn current_prodex_release_source_defaults_to_crates_io() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    assert_eq!(
        current_prodex_release_source(),
        ProdexReleaseSource::CratesIo
    );
}

#[test]
fn cached_update_version_is_scoped_to_release_source() {
    let now = Local::now().timestamp();
    assert!(!should_use_cached_update_version(
        ProdexReleaseSource::CratesIo,
        "0.2.99",
        now,
        ProdexReleaseSource::Npm,
        current_prodex_version(),
        now
    ));
    assert!(should_use_cached_update_version(
        ProdexReleaseSource::Npm,
        "0.2.99",
        now,
        ProdexReleaseSource::Npm,
        current_prodex_version(),
        now
    ));
}

#[test]
fn update_notice_is_suppressed_for_machine_output_modes() {
    assert!(!should_emit_update_notice(&Commands::Info(InfoArgs {})));
    assert!(!should_emit_update_notice(&Commands::Doctor(DoctorArgs {
        quota: false,
        runtime: true,
        json: true,
    })));
    assert!(!should_emit_update_notice(&Commands::Audit(AuditArgs {
        tail: 20,
        json: true,
        component: None,
        action: None,
        outcome: None,
    })));
    assert!(!should_emit_update_notice(&Commands::Quota(QuotaArgs {
        profile: None,
        all: false,
        detail: false,
        raw: true,
        watch: false,
        once: false,
        base_url: None,
    })));
    assert!(should_emit_update_notice(&Commands::Current));
}

#[test]
fn update_check_cache_ttl_is_short_when_cached_version_matches_current() {
    assert_eq!(
        update_check_cache_ttl_seconds("0.2.47", "0.2.47"),
        UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS
    );
    assert_eq!(
        update_check_cache_ttl_seconds("0.2.46", "0.2.47"),
        UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS
    );
    assert_eq!(
        update_check_cache_ttl_seconds("0.2.48", "0.2.47"),
        UPDATE_CHECK_CACHE_TTL_SECONDS
    );
}

#[test]
fn format_info_prodex_version_reports_up_to_date_from_cache() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should be created");
    fs::write(
        update_check_cache_file_path(&paths),
        serde_json::to_string_pretty(&serde_json::json!({
            "source": "CratesIo",
            "latest_version": current_prodex_version(),
            "checked_at": Local::now().timestamp(),
        }))
        .expect("update cache json should serialize"),
    )
    .expect("update cache should save");

    assert_eq!(
        format_info_prodex_version(&paths).expect("version summary should render"),
        format!("{} (up to date)", current_prodex_version())
    );
}

#[test]
fn format_info_prodex_version_reports_available_update_from_cache() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should be created");
    fs::write(
        update_check_cache_file_path(&paths),
        serde_json::to_string_pretty(&serde_json::json!({
            "source": "CratesIo",
            "latest_version": "99.0.0",
            "checked_at": Local::now().timestamp(),
        }))
        .expect("update cache json should serialize"),
    )
    .expect("update cache should save");

    assert_eq!(
        format_info_prodex_version(&paths).expect("version summary should render"),
        format!("{} (update available: 99.0.0)", current_prodex_version())
    );
}

#[test]
fn normalize_run_codex_args_rewrites_session_id_to_resume() {
    let args = vec![
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("continue from here"),
    ];
    assert_eq!(
        normalize_run_codex_args(&args),
        vec![
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("continue from here"),
        ]
    );
}

#[test]
fn prepare_codex_launch_args_preserves_review_detection_after_normalization() {
    let (args, include_code_review) = prepare_codex_launch_args(&[
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("review"),
    ]);

    assert_eq!(
        args,
        vec![
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("review"),
        ]
    );
    assert!(include_code_review);
}

#[test]
fn build_info_quota_aggregate_uses_live_and_snapshot_data() {
    let now = Local::now().timestamp();
    let reports = vec![
        RunProfileProbeReport {
            name: "main".to_string(),
            order_index: 0,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(80, 3_600, 90, 86_400)),
        },
        RunProfileProbeReport {
            name: "second".to_string(),
            order_index: 1,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("timeout".to_string()),
        },
        RunProfileProbeReport {
            name: "api".to_string(),
            order_index: 2,
            auth: AuthSummary {
                label: "api-key".to_string(),
                quota_compatible: false,
            },
            result: Err("api-key auth".to_string()),
        },
    ];
    let snapshots = BTreeMap::from([(
        "second".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: now,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 40,
            five_hour_reset_at: now + 1_800,
            weekly_status: RuntimeQuotaWindowStatus::Thin,
            weekly_remaining_percent: 70,
            weekly_reset_at: now + 7_200,
        },
    )]);

    let aggregate = build_info_quota_aggregate(&reports, &snapshots, now);

    assert_eq!(aggregate.quota_compatible_profiles, 2);
    assert_eq!(aggregate.live_profiles, 1);
    assert_eq!(aggregate.snapshot_profiles, 1);
    assert_eq!(aggregate.unavailable_profiles, 0);
    assert_eq!(aggregate.five_hour_pool_remaining, 120);
    assert_eq!(aggregate.weekly_pool_remaining, 160);
    assert_eq!(aggregate.earliest_five_hour_reset_at, Some(now + 1_800));
    assert_eq!(aggregate.earliest_weekly_reset_at, Some(now + 7_200));
}

#[test]
fn parse_ps_process_rows_and_classify_runtime_prodex_process() {
    let rows = parse_ps_process_rows(
        "  111 prodex /usr/local/bin/prodex run --profile main\n  222 bash bash\n",
    );

    assert_eq!(rows.len(), 2);
    let process = classify_prodex_process_row(rows[0].clone(), 999, Some("prodex"))
        .expect("prodex row should be classified");

    assert_eq!(process.pid, 111);
    assert!(process.runtime);
    assert!(classify_prodex_process_row(rows[1].clone(), 999, Some("prodex")).is_none());
}

#[test]
fn collect_info_runtime_load_summary_from_text_parses_recent_activity() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-03-30T12:10:00+07:00")
        .expect("timestamp should parse")
        .timestamp();
    let text = r#"
[2026-03-30 12:00:00.000 +07:00] profile_inflight profile=main count=2 weight=2 context=responses_http event=acquire
[2026-03-30 12:01:00.000 +07:00] selection_keep_current route=responses profile=main inflight=2 health=0 quota_source=probe_cache quota_band=quota_healthy five_hour_status=ready five_hour_remaining=80 five_hour_reset_at=1760000000 weekly_status=ready weekly_remaining=90 weekly_reset_at=1760500000
[2026-03-30 12:05:00.000 +07:00] selection_pick route=responses profile=main mode=ready inflight=1 health=0 order=0 quota_source=probe_cache quota_band=quota_healthy five_hour_status=ready five_hour_remaining=70 five_hour_reset_at=1760000000 weekly_status=ready weekly_remaining=85 weekly_reset_at=1760500000
[2026-03-30 12:06:00.000 +07:00] profile_inflight profile=main count=1 weight=1 context=standard_http event=release
"#;

    let summary = collect_info_runtime_load_summary_from_text(text, now, 30 * 60, 3 * 60 * 60);

    assert_eq!(summary.recent_selection_events, 2);
    assert_eq!(summary.active_inflight_units, 1);
    assert_eq!(summary.observations.len(), 2);
    assert_eq!(
        summary.recent_first_timestamp,
        Some(
            chrono::DateTime::parse_from_rfc3339("2026-03-30T12:01:00+07:00")
                .expect("timestamp should parse")
                .timestamp()
        )
    );
    assert_eq!(
        summary.recent_last_timestamp,
        Some(
            chrono::DateTime::parse_from_rfc3339("2026-03-30T12:05:00+07:00")
                .expect("timestamp should parse")
                .timestamp()
        )
    );
}

#[test]
fn estimate_info_runway_uses_latest_monotonic_segment_after_reset() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-03-30T11:00:00+07:00")
        .expect("timestamp should parse")
        .timestamp();
    let observations = vec![
        InfoRuntimeQuotaObservation {
            timestamp: now - 3_600,
            profile: "main".to_string(),
            five_hour_remaining: 20,
            weekly_remaining: 40,
        },
        InfoRuntimeQuotaObservation {
            timestamp: now - 1_800,
            profile: "main".to_string(),
            five_hour_remaining: 100,
            weekly_remaining: 80,
        },
        InfoRuntimeQuotaObservation {
            timestamp: now,
            profile: "main".to_string(),
            five_hour_remaining: 80,
            weekly_remaining: 70,
        },
        InfoRuntimeQuotaObservation {
            timestamp: now - 1_800,
            profile: "second".to_string(),
            five_hour_remaining: 90,
            weekly_remaining: 95,
        },
        InfoRuntimeQuotaObservation {
            timestamp: now,
            profile: "second".to_string(),
            five_hour_remaining: 60,
            weekly_remaining: 90,
        },
    ];

    let estimate = estimate_info_runway(&observations, InfoQuotaWindow::FiveHour, 140, now)
        .expect("five-hour runway should be estimated");

    assert_eq!(estimate.observed_profiles, 2);
    assert_eq!(estimate.observed_span_seconds, 1_800);
    assert!((estimate.burn_per_hour - 100.0).abs() < 0.001);
    assert_eq!(estimate.exhaust_at, now + 5_040);
}

#[test]
fn normalize_run_codex_args_keeps_regular_prompt_intact() {
    let args = vec![OsString::from("fix this bug")];
    assert_eq!(normalize_run_codex_args(&args), args);
}

#[test]
fn runtime_proxy_broker_health_endpoint_reports_registered_metadata() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let current_identity = runtime_current_prodex_binary_identity();
    register_runtime_broker_metadata(
        &proxy.log_path,
        RuntimeBrokerMetadata {
            broker_key: runtime_broker_key(&backend.base_url(), false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: current_identity.prodex_version.clone(),
            executable_path: current_identity
                .executable_path
                .as_ref()
                .map(|path| path.display().to_string()),
            executable_sha256: current_identity.executable_sha256.clone(),
        },
    );

    let response = Client::builder()
        .build()
        .expect("client")
        .get(format!(
            "http://{}/__prodex/runtime/health",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker health request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let health = response
        .json::<RuntimeBrokerHealth>()
        .expect("runtime broker health should decode");
    assert_eq!(health.current_profile, "main");
    assert_eq!(health.instance_token, "instance");
    assert_eq!(health.persistence_role, "owner");
    assert_eq!(
        health.prodex_version.as_deref(),
        Some(runtime_current_prodex_version())
    );
    assert!(health.executable_path.is_some());
    assert!(
        health
            .executable_sha256
            .as_deref()
            .is_some_and(|hash| hash.len() == 64)
    );
}

#[test]
fn runtime_proxy_broker_metrics_endpoint_reports_live_runtime_snapshot() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    register_runtime_broker_metadata(
        &proxy.log_path,
        RuntimeBrokerMetadata {
            broker_key: runtime_broker_key(&backend.base_url(), false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: None,
            executable_sha256: None,
        },
    );

    let response = Client::builder()
        .build()
        .expect("client")
        .get(format!(
            "http://{}/__prodex/runtime/metrics",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker metrics request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let metrics = response
        .json::<RuntimeBrokerMetrics>()
        .expect("runtime broker metrics should decode");
    assert_eq!(metrics.health.current_profile, "main");
    assert_eq!(metrics.health.instance_token, "instance");
    assert_eq!(metrics.health.persistence_role, "owner");
    assert_eq!(
        metrics.health.prodex_version.as_deref(),
        Some(runtime_current_prodex_version())
    );
    assert!(metrics.active_request_limit > 0);
    assert!(metrics.traffic.responses.limit > 0);
    assert_eq!(metrics.traffic.responses.admissions_total, 0);
    assert_eq!(metrics.traffic.responses.global_limit_rejections_total, 0);
    assert_eq!(metrics.traffic.responses.lane_limit_rejections_total, 0);
    assert_eq!(metrics.local_overload_backoff_remaining_seconds, 0);
    assert_eq!(metrics.continuations.response_bindings, 0);
}

#[test]
fn runtime_proxy_broker_prometheus_metrics_endpoint_reports_text_snapshot() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    register_runtime_broker_metadata(
        &proxy.log_path,
        RuntimeBrokerMetadata {
            broker_key: runtime_broker_key(&backend.base_url(), false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: None,
            executable_sha256: None,
        },
    );

    let response = Client::builder()
        .build()
        .expect("client")
        .get(format!(
            "http://{}/__prodex/runtime/metrics/prometheus",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker prometheus request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/plain; version=0.0.4; charset=utf-8")
    );
    let body = response.text().expect("prometheus body should decode");
    assert!(body.contains("prodex_runtime_broker_info"));
    assert!(body.contains("broker_key=\""));
    assert!(body.contains("current_profile=\"main\""));
    assert!(body.contains("prodex_version=\""));
    assert!(body.contains("executable_sha256=\""));
    assert!(body.contains("prodex_runtime_broker_lane_admissions_total"));
    assert!(body.contains("prodex_runtime_broker_lane_global_limit_rejections_total"));
    assert!(body.contains("prodex_runtime_broker_lane_lane_limit_rejections_total"));
    assert!(body.contains("prodex_runtime_broker_continuation_binding_counts"));
    assert!(body.contains("binding_kind=\"response\""));
    assert!(body.contains("lifecycle=\"warm\""));
}

#[test]
fn runtime_broker_metrics_snapshot_tracks_lane_admissions_and_rejections() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
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
        },
        2,
    );

    let guard = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("responses admission should succeed");
    drop(guard);

    shared
        .active_request_count
        .store(shared.active_request_limit, Ordering::SeqCst);
    assert!(matches!(
        try_acquire_runtime_proxy_active_request_slot(
            &shared,
            "http",
            "/backend-api/codex/responses/compact",
        ),
        Err(RuntimeProxyAdmissionRejection::GlobalLimit)
    ));
    shared.active_request_count.store(0, Ordering::SeqCst);

    shared
        .lane_admission
        .responses_active
        .store(shared.lane_admission.limits.responses, Ordering::SeqCst);
    assert!(matches!(
        try_acquire_runtime_proxy_active_request_slot(
            &shared,
            "http",
            "/backend-api/codex/responses",
        ),
        Err(RuntimeProxyAdmissionRejection::LaneLimit(
            RuntimeRouteKind::Responses
        ))
    ));
    shared
        .lane_admission
        .responses_active
        .store(0, Ordering::SeqCst);

    let metrics = runtime_broker_metrics_snapshot(
        &shared,
        &RuntimeBrokerMetadata {
            broker_key: "broker".to_string(),
            listen_addr: "127.0.0.1:12345".to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
        },
    )
    .expect("broker metrics snapshot should succeed");

    assert_eq!(metrics.traffic.responses.admissions_total, 1);
    assert_eq!(metrics.traffic.responses.global_limit_rejections_total, 0);
    assert_eq!(metrics.traffic.responses.lane_limit_rejections_total, 1);
    assert_eq!(metrics.traffic.compact.admissions_total, 0);
    assert_eq!(metrics.traffic.compact.global_limit_rejections_total, 1);
    assert_eq!(metrics.traffic.compact.lane_limit_rejections_total, 0);
}

#[test]
fn runtime_proxy_log_paths_remain_unique_under_parallel_generation() {
    let worker_count = 32;
    let paths_per_worker = 8;
    let barrier = Arc::new(std::sync::Barrier::new(worker_count + 1));
    let (sender, receiver) = mpsc::channel();
    let mut workers = Vec::new();

    for _ in 0..worker_count {
        let barrier = Arc::clone(&barrier);
        let sender = sender.clone();
        workers.push(thread::spawn(move || {
            barrier.wait();
            let paths = (0..paths_per_worker)
                .map(|_| create_runtime_proxy_log_path())
                .collect::<Vec<_>>();
            sender
                .send(paths)
                .expect("parallel log path batch should send");
        }));
    }

    barrier.wait();
    drop(sender);

    let mut all_paths = Vec::new();
    for paths in receiver {
        all_paths.extend(paths);
    }

    for worker in workers {
        worker.join().expect("parallel log path worker should join");
    }

    let unique_paths = all_paths.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        unique_paths.len(),
        all_paths.len(),
        "runtime proxy log paths should stay unique even under parallel creation"
    );
}

#[test]
fn runtime_proxy_broker_activate_endpoint_updates_current_profile() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    register_runtime_broker_metadata(
        &proxy.log_path,
        RuntimeBrokerMetadata {
            broker_key: runtime_broker_key(&backend.base_url(), false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            instance_token: "instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: None,
            executable_sha256: None,
        },
    );

    let client = Client::builder().build().expect("client");
    let activate = client
        .post(format!(
            "http://{}/__prodex/runtime/activate",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .json(&serde_json::json!({
            "current_profile": "second",
        }))
        .send()
        .expect("runtime broker activate request should succeed");
    assert_eq!(activate.status().as_u16(), 200);

    let health = client
        .get(format!(
            "http://{}/__prodex/runtime/health",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker health request should succeed")
        .json::<RuntimeBrokerHealth>()
        .expect("runtime broker health should decode");
    assert_eq!(health.current_profile, "second");
}

include!("main_internal/runtime_proxy_continuations.rs");

#[test]
fn runtime_proxy_worker_count_env_override_beats_policy_file() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    fs::create_dir_all(&prodex_home).expect("prodex home should exist");
    fs::write(
        prodex_home.join("policy.toml"),
        r#"
version = 1

[runtime_proxy]
worker_count = 11
"#,
    )
    .expect("policy file should write");
    let _prodex_guard = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home.display().to_string());
    let _worker_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_WORKER_COUNT", "13");

    clear_runtime_policy_cache();
    assert_eq!(runtime_proxy_worker_count(), 13);
    clear_runtime_policy_cache();
}

#[test]
fn cleanup_runtime_broker_stale_leases_removes_dead_pid_files() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "lease-test";
    let lease_dir = runtime_broker_lease_dir(&paths, broker_key);
    fs::create_dir_all(&lease_dir).expect("lease dir should exist");
    let stale_path = lease_dir.join("999999999-stale.lease");
    let live_path = lease_dir.join(format!("{}-live.lease", std::process::id()));
    fs::write(&stale_path, "stale").expect("stale lease should write");
    fs::write(&live_path, "live").expect("live lease should write");

    let live_count = cleanup_runtime_broker_stale_leases(&paths, broker_key);

    assert_eq!(live_count, 1);
    assert!(!stale_path.exists(), "dead-pid lease should be removed");
    assert!(live_path.exists(), "current-pid lease should remain");
}

#[test]
fn runtime_proxy_endpoint_child_lease_uses_requested_pid_and_cleans_up() {
    let temp_dir = TestDir::new();
    let lease_dir = temp_dir.path.join("leases");
    let endpoint = RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:33475".parse().expect("listen addr should parse"),
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        lease_dir: lease_dir.clone(),
        _lease: None,
    };

    let child_pid = 424242u32;
    let lease = endpoint
        .create_child_lease(child_pid)
        .expect("child lease should be created");
    let mut entries = fs::read_dir(&lease_dir)
        .expect("lease dir should exist")
        .collect::<Result<Vec<_>, _>>()
        .expect("lease dir should be readable");
    assert_eq!(entries.len(), 1, "expected exactly one child lease file");
    let lease_path = entries
        .pop()
        .expect("child lease entry should exist")
        .path();
    let file_name = lease_path
        .file_name()
        .and_then(|value| value.to_str())
        .expect("lease file name should be utf-8")
        .to_string();
    assert!(
        file_name.starts_with("424242-"),
        "child lease should use the child pid in its filename: {file_name}"
    );
    assert_eq!(
        fs::read_to_string(&lease_path).expect("lease file should be readable"),
        "pid=424242\n"
    );

    drop(lease);
    let remaining = fs::read_dir(&lease_dir)
        .expect("lease dir should still exist")
        .count();
    assert_eq!(remaining, 0, "dropping the lease should remove the file");
}

#[test]
fn runtime_broker_process_args_only_include_review_flag_when_enabled() {
    let without_review = runtime_broker_process_args(
        "main",
        "https://chatgpt.com/backend-api",
        false,
        "broker-key",
        "instance",
        "admin",
        None,
    );
    let without_review: Vec<String> = without_review
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    assert!(
        !without_review
            .iter()
            .any(|value| value == "--include-code-review"),
        "false should not emit a stray boolean value for the review flag"
    );

    let with_review = runtime_broker_process_args(
        "main",
        "https://chatgpt.com/backend-api",
        true,
        "broker-key",
        "instance",
        "admin",
        Some("127.0.0.1:33475"),
    );
    let with_review: Vec<String> = with_review
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    assert!(
        with_review
            .iter()
            .any(|value| value == "--include-code-review"),
        "true should emit the review flag"
    );
    assert!(
        !with_review
            .iter()
            .any(|value| value == "true" || value == "false"),
        "review flag must be encoded as a clap boolean switch"
    );
    assert!(
        with_review
            .windows(2)
            .any(|pair| pair == ["--listen-addr", "127.0.0.1:33475"]),
        "listen addr should be forwarded when requested"
    );
}

#[test]
fn runtime_broker_key_is_scoped_to_prodex_version() {
    let first = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        "version=0.39.0",
    );
    let second = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        "version=0.40.0",
    );
    let review = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        true,
        "version=0.39.0",
    );

    assert_ne!(
        first, second,
        "runtime broker keys must not reuse brokers from a different prodex version"
    );
    assert_ne!(
        first, review,
        "review and non-review broker routes should stay isolated"
    );
}

#[test]
fn preferred_runtime_broker_listen_addr_only_reuses_dead_registry_ports() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "reuse-port-test";
    save_runtime_broker_registry(
        &paths,
        broker_key,
        &RuntimeBrokerRegistry {
            pid: 999_999_999,
            listen_addr: "127.0.0.1:33475".to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            instance_token: "dead-instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        },
    )
    .expect("dead broker registry should save");

    assert_eq!(
        preferred_runtime_broker_listen_addr(&paths, broker_key)
            .expect("dead broker port lookup should succeed"),
        Some("127.0.0.1:33475".to_string())
    );

    save_runtime_broker_registry(
        &paths,
        broker_key,
        &RuntimeBrokerRegistry {
            pid: std::process::id(),
            listen_addr: "127.0.0.1:33475".to_string(),
            started_at: Local::now().timestamp(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            instance_token: "live-instance".to_string(),
            admin_token: "secret".to_string(),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
        },
    )
    .expect("live broker registry should save");

    assert_eq!(
        preferred_runtime_broker_listen_addr(&paths, broker_key)
            .expect("live broker port lookup should succeed"),
        None
    );
}

#[test]
fn runtime_rotation_proxy_can_bind_a_requested_listen_addr() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let probe = TcpListener::bind("127.0.0.1:0").expect("probe socket should bind");
    let requested_addr = probe
        .local_addr()
        .expect("probe socket should expose requested addr");
    drop(probe);

    let proxy = start_runtime_rotation_proxy_with_listen_addr(
        &paths,
        &state,
        "main",
        backend.base_url(),
        false,
        Some(&requested_addr.to_string()),
    )
    .expect("runtime proxy should bind requested listen addr");

    assert_eq!(proxy.listen_addr, requested_addr);
}

#[test]
fn runtime_broker_lease_drop_removes_file() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let lease = create_runtime_broker_lease(&paths, "drop-test")
        .expect("lease should be created for drop test");
    let lease_path = lease.path.clone();
    assert!(lease_path.exists(), "lease file should exist before drop");

    drop(lease);

    assert!(
        !lease_path.exists(),
        "lease file should be removed when the endpoint drops it"
    );
}

include!("main_internal/runtime_broker_registry.rs");

#[test]
fn runtime_broker_startup_grace_covers_ready_timeout() {
    let _timeout_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "15000");
    assert!(runtime_broker_startup_grace_seconds() >= 16);
}

#[test]
fn runtime_broker_command_is_the_only_command_without_update_notice() {
    let runtime_broker = Commands::RuntimeBroker(RuntimeBrokerArgs {
        current_profile: "main".to_string(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        broker_key: "broker".to_string(),
        instance_token: "instance".to_string(),
        admin_token: "admin".to_string(),
        listen_addr: None,
    });
    let run = Commands::Run(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        skip_quota_check: false,
        base_url: None,
        codex_args: vec![OsString::from("hello")],
    });

    assert!(!runtime_broker.should_show_update_notice());
    assert!(run.should_show_update_notice());
}

#[test]
fn claude_command_accepts_passthrough_args() {
    let command = parse_cli_command_from([
        "prodex",
        "claude",
        "--profile",
        "main",
        "--",
        "-p",
        "--output-format",
        "json",
        "hello",
    ])
    .expect("claude command should parse");
    let Commands::Claude(args) = command else {
        panic!("expected claude command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.claude_args,
        vec![
            OsString::from("-p"),
            OsString::from("--output-format"),
            OsString::from("json"),
            OsString::from("hello"),
        ]
    );
}

#[test]
fn claude_caveman_mode_extracts_prefix_and_preserves_passthrough_args() {
    let (launch_modes, claude_args) = runtime_proxy_claude_extract_launch_modes(&[
        OsString::from("caveman"),
        OsString::from("-p"),
        OsString::from("hello"),
    ]);
    assert!(launch_modes.caveman_mode);
    assert!(!launch_modes.mem_mode);
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );

    let (launch_modes, claude_args) =
        runtime_proxy_claude_extract_launch_modes(&[OsString::from("-p"), OsString::from("hi")]);
    assert!(!launch_modes.caveman_mode);
    assert!(!launch_modes.mem_mode);
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hi")]
    );
}

#[test]
fn runtime_proxy_claude_launch_modes_extract_mem_and_caveman_prefixes() {
    let (launch_modes, claude_args) = runtime_proxy_claude_extract_launch_modes(&[
        OsString::from("caveman"),
        OsString::from("mem"),
        OsString::from("-p"),
        OsString::from("hello"),
    ]);
    assert_eq!(
        launch_modes,
        RuntimeProxyClaudeLaunchModes {
            caveman_mode: true,
            mem_mode: true,
        }
    );
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );

    let (launch_modes, claude_args) = runtime_proxy_claude_extract_launch_modes(&[
        OsString::from("mem"),
        OsString::from("caveman"),
        OsString::from("--print"),
    ]);
    assert_eq!(
        launch_modes,
        RuntimeProxyClaudeLaunchModes {
            caveman_mode: true,
            mem_mode: true,
        }
    );
    assert_eq!(claude_args, vec![OsString::from("--print")]);
}

#[test]
fn runtime_mem_extract_mode_strips_only_leading_mem_prefix() {
    let (mem_mode, codex_args) =
        runtime_mem_extract_mode(&[OsString::from("mem"), OsString::from("exec")]);
    assert!(mem_mode);
    assert_eq!(codex_args, vec![OsString::from("exec")]);

    let (mem_mode, codex_args) =
        runtime_mem_extract_mode(&[OsString::from("exec"), OsString::from("mem")]);
    assert!(!mem_mode);
    assert_eq!(
        codex_args,
        vec![OsString::from("exec"), OsString::from("mem")]
    );
}

#[test]
fn runtime_proxy_claude_launch_args_prepend_plugin_dirs_when_present() {
    let launch_args = runtime_proxy_claude_launch_args(
        &[OsString::from("-p"), OsString::from("hello")],
        &[
            PathBuf::from("/tmp/claude-mem-plugin"),
            PathBuf::from("/tmp/prodex-caveman-plugin"),
        ],
    );
    assert_eq!(
        launch_args,
        vec![
            OsString::from("--plugin-dir"),
            OsString::from("/tmp/claude-mem-plugin"),
            OsString::from("--plugin-dir"),
            OsString::from("/tmp/prodex-caveman-plugin"),
            OsString::from("-p"),
            OsString::from("hello"),
        ]
    );

    let launch_args =
        runtime_proxy_claude_launch_args(&[OsString::from("-p"), OsString::from("hello")], &[]);
    assert_eq!(
        launch_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );
}

#[test]
fn prepare_runtime_proxy_claude_caveman_plugin_dir_installs_local_plugin_bundle() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };

    let plugin_dir = prepare_runtime_proxy_claude_caveman_plugin_dir(&paths)
        .expect("Claude Caveman plugin dir should prepare");
    assert!(
        plugin_dir.join(".claude-plugin/plugin.json").is_file(),
        "plugin manifest should exist"
    );
    assert!(
        plugin_dir.join("commands/caveman.toml").is_file(),
        "caveman command should exist"
    );
    assert!(
        plugin_dir.join("skills/caveman/SKILL.md").is_file(),
        "caveman skill should exist"
    );

    let activate_hook = fs::read_to_string(plugin_dir.join("hooks/caveman-activate.js"))
        .expect("activation hook should read");
    assert!(activate_hook.contains("CLAUDE_CONFIG_DIR"));
    let tracker_hook = fs::read_to_string(plugin_dir.join("hooks/caveman-mode-tracker.js"))
        .expect("tracker hook should read");
    assert!(tracker_hook.contains("getClaudeConfigDir"));
    let statusline = fs::read_to_string(plugin_dir.join("hooks/caveman-statusline.sh"))
        .expect("statusline script should read");
    assert!(statusline.contains("CLAUDE_CONFIG_DIR"));
}

#[test]
fn runtime_mem_claude_plugin_dir_from_home_uses_marketplace_install_path() {
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("home");
    assert_eq!(
        runtime_mem_claude_plugin_dir_from_home(&home),
        home.join(".claude")
            .join("plugins")
            .join("marketplaces")
            .join("thedotmack")
            .join("plugin")
    );
}

#[test]
fn runtime_mem_transcript_watch_config_path_from_home_prefers_settings_override() {
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("home");
    let data_dir = runtime_mem_data_dir_from_home(&home);
    fs::create_dir_all(&data_dir).expect("claude-mem data dir should exist");
    fs::write(
        data_dir.join("settings.json"),
        serde_json::json!({
            "CLAUDE_MEM_TRANSCRIPTS_CONFIG_PATH": data_dir.join("custom-watch.json").display().to_string()
        })
        .to_string(),
    )
    .expect("settings should write");

    assert_eq!(
        runtime_mem_transcript_watch_config_path_from_home(&home),
        data_dir.join("custom-watch.json")
    );
}

#[test]
fn ensure_runtime_mem_prodex_observer_writes_wrapper_and_settings() {
    let temp_dir = TestDir::new();
    let home = temp_dir.path.join("home");
    let settings_path = runtime_mem_settings_path_from_home(&home);
    fs::create_dir_all(settings_path.parent().expect("settings parent"))
        .expect("settings parent should exist");
    fs::write(
        &settings_path,
        serde_json::json!({
            "CLAUDE_MEM_PROVIDER": "claude",
            "CLAUDE_CODE_PATH": "/usr/bin/claude"
        })
        .to_string(),
    )
    .expect("settings should write");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex-home"),
        state_file: temp_dir.path.join("prodex-home/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex-home/profiles"),
        shared_codex_root: temp_dir.path.join("prodex-home/.codex"),
        legacy_shared_codex_root: temp_dir.path.join("prodex-home/shared"),
    };
    let prodex_exe = temp_dir.path.join("bin/prodex");
    fs::create_dir_all(prodex_exe.parent().expect("prodex bin parent"))
        .expect("prodex bin parent should exist");
    fs::write(&prodex_exe, "").expect("prodex exe should write");

    let wrapper_path = ensure_runtime_mem_prodex_observer_for_home(&home, &paths, &prodex_exe)
        .expect("prodex observer should configure");

    assert_eq!(wrapper_path, runtime_mem_prodex_claude_wrapper_path(&paths));
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings_path).expect("settings should read"))
            .expect("settings should parse");
    assert_eq!(settings["CLAUDE_MEM_PROVIDER"], serde_json::json!("claude"));
    assert_eq!(
        settings["CLAUDE_CODE_PATH"],
        serde_json::json!(wrapper_path.display().to_string())
    );

    let wrapper = fs::read_to_string(&wrapper_path).expect("wrapper should read");
    assert!(wrapper.contains(" claude --skip-quota-check -- "));
    assert!(wrapper.contains(&prodex_exe.display().to_string()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&wrapper_path)
                .expect("wrapper metadata should read")
                .permissions()
                .mode()
                & 0o111,
            0o111,
            "wrapper should be executable"
        );
    }
}

#[test]
fn ensure_runtime_mem_codex_watch_for_home_adds_prodex_watch_without_clobbering_default_watch() {
    let temp_dir = TestDir::new();
    let config_path = temp_dir.path.join("claude-mem/transcript-watch.json");
    fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("config parent should exist");
    fs::write(
        &config_path,
        serde_json::json!({
            "version": 1,
            "schemas": {
                "codex": runtime_mem_default_codex_schema(),
            },
            "watches": [{
                "name": "codex",
                "path": "~/.codex/sessions/**/*.jsonl",
                "schema": "codex",
                "startAtEnd": true,
                "context": {
                    "mode": "agents",
                    "updateOn": ["session_start", "session_end"],
                }
            }]
        })
        .to_string(),
    )
    .expect("transcript watch config should write");

    let sessions_root = temp_dir.path.join("prodex-shared/sessions");
    fs::create_dir_all(&sessions_root).expect("sessions root should exist");
    let codex_home = temp_dir.path.join("codex-home");
    fs::create_dir_all(&codex_home).expect("codex home should exist");
    runtime_proxy_create_symlink(&sessions_root, &codex_home.join("sessions"), true)
        .expect("sessions symlink should create");

    ensure_runtime_mem_codex_watch_for_home_at_path(&config_path, &codex_home)
        .expect("prodex codex watch should be added");
    ensure_runtime_mem_codex_watch_for_home_at_path(&config_path, &codex_home)
        .expect("prodex codex watch should stay deduplicated");

    let rendered: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("transcript watch config should read"),
    )
    .expect("transcript watch config should parse");
    let watches = rendered["watches"]
        .as_array()
        .expect("watches should be an array");
    assert_eq!(watches.len(), 2);
    assert_eq!(watches[0]["name"], serde_json::json!("codex"));
    let prodex_watch = watches
        .iter()
        .find(|watch| {
            watch["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("prodex-codex-"))
        })
        .expect("prodex watch should exist");
    assert_eq!(
        prodex_watch["path"],
        serde_json::json!(format!(
            "{}{}**{}*.jsonl",
            sessions_root.display(),
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        ))
    );
}

#[test]
fn prepare_caveman_launch_home_localizes_config_and_installs_plugin() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };
    create_codex_home_if_missing(&paths.shared_codex_root).expect("shared codex root");
    create_codex_home_if_missing(&paths.managed_profiles_root).expect("managed root");
    let shared_config = paths.shared_codex_root.join("config.toml");
    fs::write(
        &shared_config,
        "model = \"gpt-5\"\n[features]\nsearch_tool = true\n",
    )
    .expect("shared config should write");

    let base_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&base_home).expect("base home");
    runtime_proxy_create_symlink(&shared_config, &base_home.join("config.toml"), false)
        .expect("config symlink should create");
    fs::write(base_home.join("auth.json"), "{}").expect("auth file should write");

    let caveman_home =
        prepare_caveman_launch_home(&paths, &base_home).expect("caveman home should prepare");
    let temp_config = caveman_home.join("config.toml");
    let metadata = fs::symlink_metadata(&temp_config).expect("temp config metadata");
    assert!(
        !metadata.file_type().is_symlink(),
        "temporary Caveman config should be detached from the shared config symlink"
    );

    let rendered_config = fs::read_to_string(&temp_config).expect("temp config should read");
    assert!(rendered_config.contains("plugins = true"));
    assert!(rendered_config.contains("codex_hooks = true"));
    assert!(rendered_config.contains("suppress_unstable_features_warning = true"));
    assert!(rendered_config.contains("[marketplaces.prodex-caveman]"));
    assert!(rendered_config.contains("[plugins.\"caveman@prodex-caveman\"]"));
    assert!(rendered_config.contains("enabled = true"));

    let shared_rendered = fs::read_to_string(&shared_config).expect("shared config should read");
    assert!(
        !shared_rendered.contains("prodex-caveman"),
        "base shared config must stay unchanged"
    );
    assert!(
        !base_home.join("hooks.json").exists(),
        "base home should not gain a persistent hooks.json file"
    );

    let hooks: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(caveman_home.join("hooks.json")).expect("hooks should read"),
    )
    .expect("hooks should parse");
    assert_eq!(
        hooks["hooks"]["SessionStart"][0]["hooks"][0]["type"],
        serde_json::Value::String("command".to_string())
    );

    let marketplace_path =
        caveman_home.join(".tmp/marketplaces/prodex-caveman/.agents/plugins/marketplace.json");
    let marketplace_text =
        fs::read_to_string(&marketplace_path).expect("marketplace manifest should read");
    assert!(marketplace_text.contains("\"name\": \"prodex-caveman\""));
    assert!(
        caveman_home
            .join(".tmp/marketplaces/prodex-caveman/plugins/caveman/.codex-plugin/plugin.json")
            .is_file()
    );
    assert!(
        caveman_home
            .join("plugins/cache/prodex-caveman/caveman/0.1.0/.codex-plugin/plugin.json")
            .is_file()
    );
}


include!("main_internal/runtime_proxy_claude_and_anthropic.rs");

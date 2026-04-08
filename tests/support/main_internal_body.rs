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
        _ => None,
    }
}

fn runtime_proxy_backend_account_id_for_profile_name(profile_name: &str) -> Option<&'static str> {
    match profile_name {
        "main" => Some("main-account"),
        "second" => Some("second-account"),
        "third" => Some("third-account"),
        _ => None,
    }
}

fn runtime_proxy_backend_initial_response_id_for_account(account_id: &str) -> Option<&'static str> {
    match account_id {
        "second-account" => Some("resp-second"),
        "third-account" => Some("resp-third"),
        _ => None,
    }
}

fn runtime_proxy_backend_response_owner_account_id(response_id: &str) -> Option<&'static str> {
    if response_id == "resp-second" || response_id.starts_with("resp-second-next") {
        Some("second-account")
    } else if response_id == "resp-third" || response_id.starts_with("resp-third-next") {
        Some("third-account")
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
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let backlog = runtime_state_save_queue_backlog()
            + runtime_continuation_journal_queue_backlog()
            + runtime_probe_refresh_queue_backlog();
        let active = runtime_state_save_queue_active()
            + runtime_continuation_journal_queue_active()
            + runtime_probe_refresh_queue_active();
        if backlog == 0 && active == 0 {
            return;
        }
        if Instant::now() >= deadline {
            panic!(
                "runtime background queues did not go idle before timeout: backlog={backlog} active={active}"
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

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct TestEnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl TestEnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = env::var_os(key);
        // Tests run with --test-threads=1 in this repo, so mutating process env here is serialized.
        unsafe { env::set_var(key, value) };
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = env::var_os(key);
        // Tests run with --test-threads=1 in this repo, so mutating process env here is serialized.
        unsafe { env::remove_var(key) };
        Self { key, previous }
    }
}

impl Drop for TestEnvVarGuard {
    fn drop(&mut self) {
        if let Some(value) = self.previous.as_ref() {
            // Tests run with --test-threads=1 in this repo, so mutating process env here is serialized.
            unsafe { env::set_var(self.key, value) };
        } else {
            // Tests run with --test-threads=1 in this repo, so mutating process env here is serialized.
            unsafe { env::remove_var(self.key) };
        }
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

fn wait_for_state<F>(paths: &AppPaths, predicate: F) -> AppState
where
    F: Fn(&AppState) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_state = None;
    loop {
        if let Ok(state) = AppState::load(paths)
        {
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

fn wait_for_runtime_continuations<F>(paths: &AppPaths, predicate: F) -> RuntimeContinuationStore
where
    F: Fn(&RuntimeContinuationStore) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_continuations = None;
    loop {
        if let Ok(continuations) = load_runtime_continuations_with_recovery(
            paths,
            &AppState::load(paths).unwrap_or_default().profiles,
        )
        .map(|loaded| loaded.value)
        {
            if predicate(&continuations) {
                return continuations;
            }
            last_continuations = Some(continuations);
        }
        if Instant::now() >= deadline {
            let continuations = load_runtime_continuations_with_recovery(
                paths,
                &AppState::load(paths).unwrap_or_default().profiles,
            )
            .map(|loaded| loaded.value)
            .expect("runtime continuations should reload");
            panic!(
                "timed out waiting for runtime continuations predicate; last_continuations={:?} final_continuations={:?}",
                last_continuations, continuations
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

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

fn dead_continuation_status(now: i64) -> RuntimeContinuationBindingStatus {
    RuntimeContinuationBindingStatus {
        state: RuntimeContinuationBindingLifecycle::Dead,
        confidence: 0,
        last_touched_at: Some(now),
        last_verified_at: Some(now.saturating_sub(5)),
        last_verified_route: Some("responses".to_string()),
        last_not_found_at: Some(now),
        not_found_streak: RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT,
        success_count: 1,
        failure_count: 1,
    }
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

struct RuntimeProxyBackend {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    responses_accounts: Arc<Mutex<Vec<String>>>,
    responses_headers: Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    responses_bodies: Arc<Mutex<Vec<String>>>,
    websocket_requests: Arc<Mutex<Vec<String>>>,
    usage_accounts: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Clone, Copy)]
enum RuntimeProxyBackendMode {
    HttpOnly,
    HttpOnlyUnauthorizedMain,
    HttpOnlyBufferedJson,
    HttpOnlyInitialBodyStall,
    HttpOnlySlowStream,
    HttpOnlyStallAfterSeveralChunks,
    HttpOnlyResetBeforeFirstByte,
    HttpOnlyResetAfterFirstChunk,
    HttpOnlyPreviousResponseNeedsTurnState,
    HttpOnlyCompactOverloaded,
    HttpOnlyUsageLimitMessage,
    HttpOnlyDelayedQuotaAfterOutputItemAdded,
    HttpOnlyPlain429,
    Websocket,
    WebsocketOverloaded,
    WebsocketDelayedQuotaAfterPrelude,
    WebsocketDelayedOverloadAfterPrelude,
    WebsocketReuseSilentHang,
    WebsocketReusePreviousResponseNeedsTurnState,
    WebsocketCloseMidTurn,
    WebsocketPreviousResponseNeedsTurnState,
    WebsocketStaleReuseNeedsTurnState,
    WebsocketTopLevelResponseId,
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

    fn start_http_compact_overloaded() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyCompactOverloaded)
    }

    fn start_http_usage_limit_message() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage)
    }

    fn start_http_delayed_quota_after_output_item_added() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyDelayedQuotaAfterOutputItemAdded)
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

    fn start_websocket_reuse_previous_response_needs_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState)
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
        let shutdown_flag = Arc::clone(&shutdown);
        let responses_accounts_flag = Arc::clone(&responses_accounts);
        let responses_headers_flag = Arc::clone(&responses_headers);
        let responses_bodies_flag = Arc::clone(&responses_bodies);
        let websocket_requests_flag = Arc::clone(&websocket_requests);
        let usage_accounts_flag = Arc::clone(&usage_accounts);
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
                                | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
                                | RuntimeProxyBackendMode::WebsocketCloseMidTurn
                                | RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                                | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
                                | RuntimeProxyBackendMode::WebsocketTopLevelResponseId
                        );
                        thread::spawn(move || {
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
                "main-account" => runtime_proxy_usage_body("main@example.com"),
                "second-account" => runtime_proxy_usage_body("second@example.com"),
                "third-account" => runtime_proxy_usage_body("third@example.com"),
                _ => serde_json::json!({ "error": "unauthorized" }).to_string(),
            };
            let status = if matches!(
                account_id.as_str(),
                "main-account" | "second-account" | "third-account"
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
                ("main-account", RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage) => (
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
                .push(request_body);
            let previous_response_id = request_previous_response_id(&request);
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
                    } else if matches!(mode, RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage) {
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
                        RuntimeProxyBackendMode::HttpOnlyPreviousResponseNeedsTurnState
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
            match (account_id.as_str(), mode) {
                ("main-account", RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage) => (
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
                    RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                        | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
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
                    RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
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

fn runtime_proxy_usage_body(email: &str) -> String {
    serde_json::json!({
        "email": email,
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": 5,
                "reset_at": future_epoch(18_000),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 5,
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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create auth parent dir");
    }
    fs::write(
        path,
        serde_json::json!({
            "tokens": {
                "access_token": "test-token",
                "account_id": account_id,
            }
        })
        .to_string(),
    )
    .expect("failed to write auth.json");
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
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "fast".to_string(),
            usage: usage_with_main_windows(80, 18_000, 80, 86_400),
            order_index: 1,
            preferred: false,
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
                },
            ),
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
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
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
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

#[test]
fn runtime_proxy_default_long_lived_capacity_scales_for_terminal_fanout() {
    assert_eq!(runtime_proxy_long_lived_worker_count_default(1), 32);
    assert_eq!(runtime_proxy_long_lived_worker_count_default(4), 32);
    assert_eq!(runtime_proxy_long_lived_worker_count_default(8), 64);
    assert_eq!(runtime_proxy_long_lived_worker_count_default(32), 128);

    assert_eq!(runtime_proxy_long_lived_queue_capacity(1), 128);
    assert_eq!(runtime_proxy_long_lived_queue_capacity(32), 256);
    assert_eq!(runtime_proxy_long_lived_queue_capacity(64), 512);
    assert_eq!(runtime_proxy_long_lived_queue_capacity(128), 1024);
}

#[test]
fn runtime_proxy_async_worker_count_is_bounded() {
    let _low_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT", "1");
    let low = runtime_proxy_async_worker_count();
    assert!(
        (2..=8).contains(&low),
        "low async worker override should clamp into 2..=8, got {low}"
    );

    let _high_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT", "999");
    let high = runtime_proxy_async_worker_count();
    assert!(
        (2..=8).contains(&high),
        "high async worker override should clamp into 2..=8, got {high}"
    );
}

#[test]
fn runtime_proxy_async_worker_count_default_scales_with_parallelism() {
    assert_eq!(runtime_proxy_async_worker_count_default(1), 2);
    assert_eq!(runtime_proxy_async_worker_count_default(2), 4);
    assert_eq!(runtime_proxy_async_worker_count_default(4), 8);
    assert_eq!(runtime_proxy_async_worker_count_default(64), 8);
}

#[test]
fn runtime_proxy_default_active_limit_scales_with_long_lived_streams() {
    assert_eq!(runtime_proxy_active_request_limit_default(8, 32), 104);
    assert_eq!(runtime_proxy_active_request_limit_default(32, 64), 224);
    assert_eq!(runtime_proxy_active_request_limit_default(32, 128), 416);

    let lane_limits =
        runtime_proxy_lane_limits(runtime_proxy_active_request_limit_default(32, 64), 32, 64);
    assert!(lane_limits.responses > lane_limits.compact);
    assert!(lane_limits.responses > lane_limits.standard);
    assert!(lane_limits.responses >= lane_limits.websocket);
}

#[test]
fn runtime_profile_inflight_soft_limit_tightens_under_pressure() {
    let steady_responses =
        runtime_profile_inflight_soft_limit(RuntimeRouteKind::Responses, false);
    let pressured_responses =
        runtime_profile_inflight_soft_limit(RuntimeRouteKind::Responses, true);
    let pressured_compact = runtime_profile_inflight_soft_limit(RuntimeRouteKind::Compact, true);

    assert_eq!(steady_responses, RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT.max(1));
    assert!(pressured_responses >= 1);
    assert!(pressured_responses <= steady_responses);
    assert!(pressured_compact >= 1);
    assert!(pressured_compact <= pressured_responses);
}

#[test]
fn runtime_state_save_debounce_applies_to_hot_continuation_updates() {
    assert_eq!(
        runtime_state_save_debounce("profile_commit:main"),
        Duration::ZERO
    );
    assert!(
        runtime_state_save_debounce("session_id:main") > Duration::ZERO,
        "session id saves should be debounced"
    );
    assert!(
        runtime_state_save_debounce("response_ids:main") > Duration::ZERO,
        "response id saves should be debounced"
    );
    assert!(
        runtime_state_save_debounce("compact_session_touch:session-main") > Duration::ZERO,
        "compact lineage touches should be debounced"
    );
}

#[test]
fn runtime_state_save_reason_only_journals_owner_changes() {
    assert!(runtime_state_save_reason_requires_continuation_journal(
        "response_ids:main"
    ));
    assert!(runtime_state_save_reason_requires_continuation_journal(
        "turn_state:turn-main"
    ));
    assert!(runtime_state_save_reason_requires_continuation_journal(
        "session_id:session-main"
    ));
    assert!(runtime_state_save_reason_requires_continuation_journal(
        "compact_lineage:main"
    ));
    assert!(!runtime_state_save_reason_requires_continuation_journal(
        "response_touch:main"
    ));
    assert!(!runtime_state_save_reason_requires_continuation_journal(
        "turn_state_touch:main"
    ));
    assert!(!runtime_state_save_reason_requires_continuation_journal(
        "session_touch:main"
    ));
    assert!(!runtime_state_save_reason_requires_continuation_journal(
        "compact_session_touch:main"
    ));
}

#[derive(Debug)]
struct TestScheduledJob {
    ready_at: Instant,
}

impl RuntimeScheduledSaveJob for TestScheduledJob {
    fn ready_at(&self) -> Instant {
        self.ready_at
    }
}

#[test]
fn runtime_take_due_scheduled_jobs_waits_for_future_entries() {
    let now = Instant::now();
    let mut pending = BTreeMap::from([(
        "later".to_string(),
        TestScheduledJob {
            ready_at: now + Duration::from_millis(25),
        },
    )]);

    match runtime_take_due_scheduled_jobs(&mut pending, now) {
        RuntimeDueJobs::Wait(wait_for) => {
            assert!(wait_for >= Duration::from_millis(20));
            assert_eq!(pending.len(), 1);
        }
        RuntimeDueJobs::Due(_) => panic!("future work should not be returned early"),
    }
}

#[test]
fn runtime_take_due_scheduled_jobs_returns_only_ready_entries() {
    let now = Instant::now();
    let mut pending = BTreeMap::from([
        ("due".to_string(), TestScheduledJob { ready_at: now }),
        (
            "later".to_string(),
            TestScheduledJob {
                ready_at: now + Duration::from_millis(50),
            },
        ),
    ]);

    match runtime_take_due_scheduled_jobs(&mut pending, now) {
        RuntimeDueJobs::Due(due) => {
            assert_eq!(due.len(), 1);
            assert!(due.contains_key("due"));
            assert_eq!(pending.len(), 1);
            assert!(pending.contains_key("later"));
        }
        RuntimeDueJobs::Wait(_) => panic!("ready work should be returned immediately"),
    }
}

#[test]
fn runtime_soften_persisted_backoffs_for_startup_clamps_short_lived_penalties() {
    let now = Local::now().timestamp();
    let circuit_key = runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses);
    let mut backoffs = RuntimeProfileBackoffs {
        retry_backoff_until: BTreeMap::from([("retry".to_string(), now + 60)]),
        transport_backoff_until: BTreeMap::from([
            ("expired".to_string(), now - 1),
            (
                runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Responses),
                now + 60,
            ),
        ]),
        route_circuit_open_until: BTreeMap::from([
            ("expired".to_string(), now - 1),
            (circuit_key.clone(), now + 60),
        ]),
    };
    let profile_scores = BTreeMap::from([(
        runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
        RuntimeProfileHealth {
            score: RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2,
            updated_at: now,
        },
    )]);

    runtime_soften_persisted_backoffs_for_startup(&mut backoffs, &profile_scores, now);

    assert_eq!(backoffs.retry_backoff_until.get("retry"), Some(&(now + 60)));
    assert!(!backoffs.transport_backoff_until.contains_key("expired"));
    assert_eq!(
        backoffs
            .transport_backoff_until
            .get(&runtime_profile_transport_backoff_key(
                "main",
                RuntimeRouteKind::Responses,
            )),
        Some(&now.saturating_add(RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS))
    );
    assert!(!backoffs.route_circuit_open_until.contains_key("expired"));
    assert_eq!(
        backoffs.route_circuit_open_until.get(&circuit_key),
        Some(
            &now.saturating_add(runtime_profile_circuit_half_open_probe_seconds(
                RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2
            ))
        )
    );
}

#[test]
fn runtime_softened_backoffs_persist_after_proxy_startup() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let saved_now = Local::now().timestamp();
    save_runtime_profile_backoffs(
        &paths,
        &RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([("main".to_string(), saved_now + 600)]),
            transport_backoff_until: BTreeMap::from([(
                runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Responses),
                saved_now + 600,
            )]),
            route_circuit_open_until: BTreeMap::from([(
                runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
                saved_now + 600,
            )]),
        },
    )
    .expect("backoffs should save");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    wait_for_runtime_background_queues_idle();

    let loaded =
        load_runtime_profile_backoffs(&paths, &state.profiles).expect("backoffs should reload");
    let now = Local::now().timestamp();
    assert_eq!(
        loaded.retry_backoff_until.get("main"),
        Some(&(saved_now + 600))
    );
    assert!(
        loaded
            .transport_backoff_until
            .get(&runtime_profile_transport_backoff_key(
                "main",
                RuntimeRouteKind::Responses,
            ))
            .is_none_or(|until| *until <= now + RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS)
    );
    assert!(
        loaded
            .route_circuit_open_until
            .get(&runtime_profile_route_circuit_key(
                "main",
                RuntimeRouteKind::Responses
            ))
            .is_none_or(|until| *until <= now + RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS)
    );

    drop(proxy);
}

#[test]
fn runtime_state_save_accepts_legacy_backoffs_without_last_good_backup() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    create_codex_home_if_missing(&profile_home).expect("profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let now = Local::now().timestamp();
    let transport_key = runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Responses);
    fs::write(
        runtime_backoffs_file_path(&paths),
        format!(
            concat!(
                "{{\n",
                "  \"retry_backoff_until\": {{\"main\": {}}},\n",
                "  \"transport_backoff_until\": {{\"{}\": {}}}\n",
                "}}\n"
            ),
            now + 600,
            transport_key,
            now + 300,
        ),
    )
    .expect("legacy raw runtime backoffs should be written");

    let runtime = RuntimeRotationState {
        paths: paths.clone(),
        state: state.clone(),
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
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    schedule_runtime_state_save(
        &shared,
        state.clone(),
        runtime_continuation_store_from_app_state(&state),
        BTreeMap::new(),
        BTreeMap::new(),
        RuntimeProfileBackoffs::default(),
        paths.clone(),
        "legacy_backoffs",
    );
    wait_for_runtime_background_queues_idle();

    let log =
        fs::read_to_string(&shared.log_path).expect("runtime state save log should be readable");
    assert!(
        log.contains("state_save_ok"),
        "legacy backoffs should not break runtime saves: {log}"
    );
    assert!(
        !log.contains("state_save_error"),
        "legacy backoffs should not emit state save errors: {log}"
    );

    let loaded =
        load_runtime_profile_backoffs(&paths, &state.profiles).expect("backoffs should reload");
    assert_eq!(loaded.retry_backoff_until.get("main"), Some(&(now + 600)));
    assert_eq!(
        loaded.transport_backoff_until.get(&transport_key),
        Some(&(now + 300))
    );
    assert!(
        loaded.route_circuit_open_until.is_empty(),
        "missing legacy route circuit data should default to empty"
    );
}

#[test]
fn runtime_fault_injection_consumes_budget() {
    let _guard = TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE", "2");

    assert!(runtime_take_fault_injection(
        "PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE"
    ));
    assert!(runtime_take_fault_injection(
        "PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE"
    ));
    assert!(!runtime_take_fault_injection(
        "PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE"
    ));
}

#[test]
fn runtime_request_strips_previous_response_id_from_function_call_output_payloads() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"previous_response_id":"resp_123","input":[{"type":"function_call_output","call_id":"call_123","output":"ok"}]}"#.to_vec(),
    };

    assert!(
        runtime_request_without_previous_response_id(&request).is_some(),
        "helper should still be able to strip previous_response_id when explicitly asked"
    );
    assert!(
        runtime_request_requires_previous_response_affinity(&request),
        "function call outputs should keep previous_response affinity during normal proxying"
    );
}

#[test]
fn parse_runtime_websocket_request_metadata_extracts_affinity_fields() {
    let metadata = parse_runtime_websocket_request_metadata(
        r#"{"previous_response_id":"resp_123","client_metadata":{"session_id":"sess_123"},"input":[{"type":"function_call_output","call_id":"call_123","output":"ok"}]}"#,
    );

    assert_eq!(metadata.previous_response_id.as_deref(), Some("resp_123"));
    assert_eq!(metadata.session_id.as_deref(), Some("sess_123"));
    assert!(metadata.requires_previous_response_affinity);
}

#[test]
fn runtime_quota_summary_distinguishes_window_health() {
    let summary = runtime_quota_summary_for_route(
        &usage_with_main_windows(4, 18_000, 12, 604_800),
        RuntimeRouteKind::Responses,
    );

    assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Critical);
    assert_eq!(summary.five_hour.status, RuntimeQuotaWindowStatus::Critical);
    assert_eq!(summary.weekly.status, RuntimeQuotaWindowStatus::Thin);
    assert_eq!(summary.five_hour.remaining_percent, 4);
    assert_eq!(summary.weekly.remaining_percent, 12);
}

#[test]
fn ready_profile_ranking_prefers_larger_reserve_when_resets_match() {
    let candidates = vec![
        ReadyProfileCandidate {
            name: "thin".to_string(),
            usage: usage_with_main_windows(65, 18_000, 70, 604_800),
            order_index: 0,
            preferred: false,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "deep".to_string(),
            usage: usage_with_main_windows(95, 18_000, 98, 604_800),
            order_index: 1,
            preferred: false,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let mut ranked = candidates.clone();
    ranked.sort_by_key(ready_profile_sort_key);
    assert_eq!(ranked[0].name, "deep");
}

#[test]
fn scheduler_prefers_rested_profile_within_near_optimal_band() {
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: None,
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::from([
            ("fresh".to_string(), now),
            ("rested".to_string(), now - 3_600),
        ]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let candidates = vec![
        ReadyProfileCandidate {
            name: "fresh".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "rested".to_string(),
            usage: usage_with_main_windows(96, 18_000, 96, 604_800),
            order_index: 1,
            preferred: false,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let ranked = schedule_ready_profile_candidates(candidates, &state, None);
    assert_eq!(ranked[0].name, "rested");
}

#[test]
fn scheduler_keeps_preferred_profile_when_gain_is_small() {
    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let candidates = vec![
        ReadyProfileCandidate {
            name: "better".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "active".to_string(),
            usage: usage_with_main_windows(96, 18_000, 96, 604_800),
            order_index: 1,
            preferred: true,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let ranked = schedule_ready_profile_candidates(candidates, &state, Some("active"));
    assert_eq!(ranked[0].name, "active");
}

#[test]
fn scheduler_allows_switch_when_preferred_profile_is_in_cooldown() {
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::new(),
        last_run_selected_at: BTreeMap::from([("active".to_string(), now)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let candidates = vec![
        ReadyProfileCandidate {
            name: "better".to_string(),
            usage: usage_with_main_windows(100, 18_000, 100, 604_800),
            order_index: 0,
            preferred: false,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
        ReadyProfileCandidate {
            name: "active".to_string(),
            usage: usage_with_main_windows(96, 18_000, 96, 604_800),
            order_index: 1,
            preferred: true,
            quota_source: RuntimeQuotaSource::LiveProbe,
        },
    ];

    let ranked = schedule_ready_profile_candidates(candidates, &state, Some("active"));
    assert_eq!(ranked[0].name, "better");
}

#[test]
fn ready_profile_candidates_use_persisted_snapshot_when_probe_is_unavailable() {
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/main"),
                    managed: true,
                    email: None,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/second"),
                    managed: true,
                    email: None,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    };
    let reports = vec![
        RunProfileProbeReport {
            name: "main".to_string(),
            order_index: 0,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("runtime quota snapshot unavailable".to_string()),
        },
        RunProfileProbeReport {
            name: "second".to_string(),
            order_index: 1,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("runtime quota snapshot unavailable".to_string()),
        },
    ];
    let now = Local::now().timestamp();
    let persisted = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 70,
                weekly_reset_at: now + 86_400,
            },
        ),
        (
            "second".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 90,
                five_hour_reset_at: now + 3_600,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 95,
                weekly_reset_at: now + 604_800,
            },
        ),
    ]);

    let ranked = ready_profile_candidates(&reports, false, Some("main"), &state, Some(&persisted));
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].name, "second");
    assert_eq!(
        ranked[0].quota_source,
        RuntimeQuotaSource::PersistedSnapshot
    );
}

#[test]
fn run_profile_probe_is_ready_only_when_live_quota_is_clear() {
    let ready = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(90, 3_600, 95, 86_400)),
    };
    let blocked = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(0, 300, 95, 86_400)),
    };
    let failed = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Err("timeout".to_string()),
    };

    assert!(run_profile_probe_is_ready(&ready, false));
    assert!(!run_profile_probe_is_ready(&blocked, false));
    assert!(!run_profile_probe_is_ready(&failed, false));
}

#[test]
fn run_preflight_reports_with_current_first_preserves_current_and_rotation_order() {
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/main"),
                    managed: true,
                    email: None,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/second"),
                    managed: true,
                    email: None,
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/third"),
                    managed: true,
                    email: None,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let current_report = RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(usage_with_main_windows(90, 3_600, 95, 86_400)),
    };

    let reports =
        run_preflight_reports_with_current_first(&state, "main", current_report.clone(), None);

    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0].name, "main");
    assert_eq!(reports[0].order_index, 0);
    assert_eq!(
        reports[0]
            .result
            .as_ref()
            .ok()
            .map(format_main_windows_compact),
        current_report
            .result
            .as_ref()
            .ok()
            .map(format_main_windows_compact)
    );
    assert_eq!(reports[1].name, "second");
    assert_eq!(reports[1].order_index, 1);
    assert_eq!(reports[2].name, "third");
    assert_eq!(reports[2].order_index, 2);
}

#[test]
fn quota_overview_sort_prioritizes_status_then_nearest_reset() {
    let reports = vec![
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(0, 3_600, 80, 86_400)),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 7_200, 95, 172_800)),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("boom".to_string()),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 1_800, 95, 259_200)),
            fetched_at: 1_700_000_000,
        },
    ];

    let names = sort_quota_reports_for_display(&reports)
        .into_iter()
        .map(|report| report.name.clone())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["ready-early", "ready-late", "blocked", "error"]);
}

#[test]
fn quota_watch_defaults_to_live_refresh_for_regular_views() {
    let profile_args = QuotaArgs {
        profile: Some("main".to_string()),
        all: false,
        detail: false,
        raw: false,
        watch: false,
        once: false,
        base_url: None,
    };
    assert!(quota_watch_enabled(&profile_args));

    let overview_args = QuotaArgs {
        all: true,
        ..profile_args
    };
    assert!(quota_watch_enabled(&overview_args));
}

#[test]
fn quota_watch_respects_once_and_raw_modes() {
    let once_args = QuotaArgs {
        profile: Some("main".to_string()),
        all: false,
        detail: false,
        raw: false,
        watch: false,
        once: true,
        base_url: None,
    };
    assert!(!quota_watch_enabled(&once_args));

    let raw_args = QuotaArgs {
        raw: true,
        watch: true,
        once: false,
        ..once_args
    };
    assert!(!quota_watch_enabled(&raw_args));
}

#[test]
fn quota_command_accepts_once_flag() {
    let command = parse_cli_command_from(["prodex", "quota", "--once"]).expect("quota command");
    let Commands::Quota(args) = command else {
        panic!("expected quota command");
    };
    assert!(args.once);
    assert!(!quota_watch_enabled(&args));
}

#[test]
fn audit_command_accepts_filters_and_json() {
    let command = parse_cli_command_from([
        "prodex",
        "audit",
        "--tail",
        "50",
        "--component",
        "profile",
        "--action",
        "use",
        "--json",
    ])
    .expect("audit command");
    let Commands::Audit(args) = command else {
        panic!("expected audit command");
    };
    assert_eq!(args.tail, 50);
    assert_eq!(args.component.as_deref(), Some("profile"));
    assert_eq!(args.action.as_deref(), Some("use"));
    assert!(args.json);
}

#[test]
fn bare_prodex_defaults_to_run_command() {
    let command = parse_cli_command_from(["prodex"]).expect("bare prodex should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert!(args.profile.is_none());
    assert!(!args.auto_rotate);
    assert!(!args.no_auto_rotate);
    assert!(!args.skip_quota_check);
    assert!(args.base_url.is_none());
    assert!(args.codex_args.is_empty());
}

#[test]
fn bare_prodex_with_codex_args_defaults_to_run_command() {
    let command = parse_cli_command_from(["prodex", "exec", "review this repo"])
        .expect("bare prodex codex args should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn cleanup_command_does_not_default_to_run() {
    let command = parse_cli_command_from(["prodex", "cleanup"]).expect("cleanup command");
    assert!(matches!(command, Commands::Cleanup));
}

#[test]
fn bare_prodex_accepts_run_options_before_codex_args() {
    let command =
        parse_cli_command_from(["prodex", "--profile", "second", "exec", "review this repo"])
            .expect("bare prodex should accept run options");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(args.profile.as_deref(), Some("second"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn profile_quota_watch_output_renders_snapshot_body_without_watch_header() {
    let output = render_profile_quota_watch_output(
        "main",
        "2026-03-22 10:00:00 WIB",
        Ok(usage_with_main_windows(63, 18_000, 12, 604_800)),
    );

    assert!(output.contains("Quota main"));
    assert!(!output.contains("Quota Watch"));
    assert!(!output.contains("Updated"));
    assert!(!output.contains("2026-03-22 10:00:00 WIB"));
    assert!(!output.ends_with('\n'));
}

#[test]
fn all_quota_watch_output_omits_watch_header_on_load_error() {
    let output = render_all_quota_watch_output(
        "2026-03-22 10:00:00 WIB",
        Err("load failed".to_string()),
        None,
        false,
    );

    assert!(output.contains("Quota"));
    assert!(!output.contains("Quota Watch"));
    assert!(!output.contains("Updated"));
    assert!(!output.contains("2026-03-22 10:00:00 WIB"));
    assert!(output.contains("load failed"));
    assert!(!output.ends_with('\n'));
}

#[test]
fn quota_reports_include_pool_summary_lines() {
    let alpha = usage_with_main_windows(90, 7_200, 95, 172_800);
    let beta = usage_with_main_windows(45, 1_800, 40, 86_400);
    let last_update = 1_700_000_123;
    let reports = vec![
        QuotaReport {
            name: "alpha".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(alpha.clone()),
            fetched_at: 1_700_000_100,
        },
        QuotaReport {
            name: "beta".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(beta.clone()),
            fetched_at: last_update,
        },
        QuotaReport {
            name: "api".to_string(),
            active: false,
            auth: AuthSummary {
                label: "api-key".to_string(),
                quota_compatible: false,
            },
            result: Err("auth mode is not quota-compatible".to_string()),
            fetched_at: 1_700_000_090,
        },
    ];

    let output = render_quota_reports_with_layout(&reports, false, None, 160);
    let five_hour_reset = required_main_window_snapshot(&beta, "5h")
        .expect("5h snapshot")
        .reset_at;
    let weekly_reset = required_main_window_snapshot(&beta, "weekly")
        .expect("weekly snapshot")
        .reset_at;

    assert!(output.contains("Available:"));
    assert!(output.contains("2/3 profile"));
    assert!(output.contains("Last Updated:"));
    assert!(output.contains(&format_precise_reset_time(Some(last_update))));
    assert!(!output.contains("Unavailable:"));
    assert!(output.contains("5h remaining pool:"));
    assert!(output.contains("Weekly remaining pool:"));
    assert!(output.contains(&format_info_pool_remaining(135, 2, Some(five_hour_reset))));
    assert!(output.contains(&format_info_pool_remaining(135, 2, Some(weekly_reset))));
    assert!(output.contains("\n\nPROFILE"));
}

#[test]
fn quota_reports_respect_line_budget_while_preserving_sort_order() {
    let reports = vec![
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(0, 3_600, 80, 86_400)),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 7_200, 95, 172_800)),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("boom".to_string()),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 1_800, 95, 259_200)),
            fetched_at: 1_700_000_000,
        },
    ];

    let output = render_quota_reports_with_line_limit(&reports, false, Some(16));

    assert!(output.contains("ready-early"));
    assert!(output.contains("ready-late"));
    assert!(!output.contains("blocked"));
    assert!(!output.contains("error"));
    assert!(output.contains("\n\nshowing top 2 of 4 profiles"));
}

#[test]
fn quota_reports_window_supports_scroll_offset_and_hint() {
    let reports = vec![
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(0, 3_600, 80, 86_400)),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 7_200, 95, 172_800)),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("boom".to_string()),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 1_800, 95, 259_200)),
            fetched_at: 1_700_000_000,
        },
    ];

    let window = render_quota_reports_window_with_layout(&reports, false, Some(16), 100, 1, true);

    assert_eq!(window.start_profile, 1);
    assert_eq!(window.total_profiles, 4);
    assert_eq!(window.shown_profiles, 2);
    assert_eq!(window.hidden_before, 1);
    assert_eq!(window.hidden_after, 1);
    assert!(window.output.contains("ready-late"));
    assert!(window.output.contains("blocked"));
    assert!(!window.output.contains("ready-early"));
    assert!(
        window
            .output
            .contains("\n\npress Up/Down to scroll profiles (2-3 of 4; 1 above, 1 below)")
    );
}

#[test]
fn quota_reports_fit_requested_width_in_narrow_layout() {
    let reports = vec![
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 1_800, 95, 259_200)),
            fetched_at: 1_700_000_000,
        },
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(0, 3_600, 80, 86_400)),
            fetched_at: 1_700_000_000,
        },
    ];

    let output = render_quota_reports_with_layout(&reports, false, None, 72);

    assert!(output.lines().all(|line| text_width(line) <= 72));
}

#[test]
fn rotates_profiles_after_current_profile() {
    let state = AppState {
        active_profile: Some("beta".to_string()),
        profiles: BTreeMap::from([
            (
                "alpha".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/alpha"),
                    managed: true,
                    email: None,
                },
            ),
            (
                "beta".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/beta"),
                    managed: true,
                    email: None,
                },
            ),
            (
                "gamma".to_string(),
                ProfileEntry {
                    codex_home: PathBuf::from("/tmp/gamma"),
                    managed: true,
                    email: None,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    };

    assert_eq!(
        profile_rotation_order(&state, "beta"),
        vec!["gamma".to_string(), "alpha".to_string()]
    );
}

#[test]
fn backend_api_base_url_maps_to_wham_usage() {
    assert_eq!(
        usage_url("https://chatgpt.com/backend-api"),
        "https://chatgpt.com/backend-api/wham/usage"
    );
}

#[test]
fn custom_base_url_maps_to_codex_usage() {
    assert_eq!(
        usage_url("http://127.0.0.1:8080"),
        "http://127.0.0.1:8080/api/codex/usage"
    );
}

#[test]
fn profile_name_is_derived_from_email() {
    assert_eq!(
        profile_name_from_email("Main+Ops@Example.com"),
        "main-ops_example.com"
    );
}

#[test]
fn unique_profile_name_adds_numeric_suffix() {
    let state = AppState {
        active_profile: None,
        profiles: BTreeMap::from([(
            "main_example.com".to_string(),
            ProfileEntry {
                codex_home: PathBuf::from("/tmp/existing"),
                managed: true,
                email: Some("other@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: PathBuf::from("/tmp/prodex-test"),
        state_file: PathBuf::from("/tmp/prodex-test/state.json"),
        managed_profiles_root: PathBuf::from("/tmp/prodex-test/profiles"),
        shared_codex_root: PathBuf::from("/tmp/prodex-test/default-codex"),
        legacy_shared_codex_root: PathBuf::from("/tmp/prodex-test/shared"),
    };

    assert_eq!(
        unique_profile_name_for_email(&paths, &state, "main@example.com"),
        "main_example.com-2"
    );
}

#[test]
fn unique_profile_name_reclaims_untracked_managed_directory() {
    let temp_dir = TestDir::new();
    let state = AppState::default();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let stale_dir = paths.managed_profiles_root.join("main_example.com");
    fs::create_dir_all(&stale_dir).expect("stale managed directory should exist");
    fs::write(stale_dir.join("stale.txt"), "old").expect("stale file should be written");

    assert_eq!(
        unique_profile_name_for_email(&paths, &state, "main@example.com"),
        "main_example.com"
    );
    assert!(
        !stale_dir.exists(),
        "untracked managed directory should be reclaimed before suffixing"
    );
}

#[test]
fn remove_profile_deletes_managed_home_by_default() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let paths = AppPaths::discover().expect("paths should resolve");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profile root should exist");
    let profile_home = paths.managed_profiles_root.join("main");
    fs::create_dir_all(&profile_home).expect("managed profile home should exist");
    fs::write(profile_home.join("auth.json"), "{}").expect("auth file should exist");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    write_versioned_runtime_sidecar(
        &runtime_usage_snapshots_file_path(&paths),
        &runtime_usage_snapshots_last_good_file_path(&paths),
        0,
        &BTreeMap::<String, RuntimeProfileUsageSnapshot>::new(),
    );
    write_versioned_runtime_sidecar(
        &runtime_scores_file_path(&paths),
        &runtime_scores_last_good_file_path(&paths),
        0,
        &BTreeMap::<String, RuntimeProfileHealth>::new(),
    );
    write_versioned_runtime_sidecar(
        &runtime_backoffs_file_path(&paths),
        &runtime_backoffs_last_good_file_path(&paths),
        0,
        &RuntimeProfileBackoffs::default(),
    );
    state.save(&paths).expect("state should save");

    handle_remove_profile(RemoveProfileArgs {
        name: "main".to_string(),
        delete_home: false,
    })
    .expect("managed profile remove should succeed");

    let reloaded = AppState::load(&paths).expect("state should reload");
    assert!(!reloaded.profiles.contains_key("main"));
    assert!(
        !profile_home.exists(),
        "managed profile home should be deleted even without --delete-home"
    );
}

#[test]
fn app_paths_discover_uses_prodex_root_for_default_shared_codex_home() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);

    let paths = AppPaths::discover().expect("paths should resolve");
    assert_eq!(paths.root, prodex_home);
    assert_eq!(paths.shared_codex_root, paths.root.join(".codex"));
    assert_eq!(paths.legacy_shared_codex_root, paths.root.join("shared"));
}

#[test]
fn app_paths_discover_resolves_relative_shared_codex_home_inside_prodex_root() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", ".codex-local");

    let paths = AppPaths::discover().expect("paths should resolve");
    assert_eq!(paths.shared_codex_root, paths.root.join(".codex-local"));
}

#[test]
fn select_default_codex_home_prefers_legacy_home_until_prodex_shared_home_exists() {
    let temp_dir = TestDir::new();
    let shared_codex_home = temp_dir.path.join("prodex/.codex");
    let legacy_codex_home = temp_dir.path.join("home/.codex");
    fs::create_dir_all(&legacy_codex_home).expect("legacy codex home should exist");

    assert_eq!(
        select_default_codex_home(&shared_codex_home, &legacy_codex_home, false),
        legacy_codex_home
    );

    fs::create_dir_all(&shared_codex_home).expect("prodex shared codex home should exist");
    assert_eq!(
        select_default_codex_home(&shared_codex_home, &legacy_codex_home, false),
        shared_codex_home
    );

    fs::remove_dir_all(&shared_codex_home).expect("shared codex home should be removed");
    assert_eq!(
        select_default_codex_home(&shared_codex_home, &legacy_codex_home, true),
        shared_codex_home
    );
}

#[test]
fn parses_email_from_chatgpt_id_token() {
    let id_token = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL3Byb2ZpbGUiOnsiZW1haWwiOiJ1c2VyQGV4YW1wbGUuY29tIn19.c2ln";

    assert_eq!(
        parse_email_from_id_token(id_token).expect("id token should parse"),
        Some("user@example.com".to_string())
    );
}

#[test]
fn usage_response_accepts_null_additional_rate_limits() {
    let usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "email": "user@example.com",
        "plan_type": "plus",
        "rate_limit": null,
        "code_review_rate_limit": null,
        "additional_rate_limits": null
    }))
    .expect("usage response should parse");

    assert!(usage.additional_rate_limits.is_empty());
}

#[test]
fn previous_response_owner_discovery_ignores_retry_backoff() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::from([(
            "second".to_string(),
            Local::now().timestamp().saturating_add(60),
        )]),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let excluded = BTreeSet::from(["main".to_string()]);

    assert_eq!(
        next_runtime_previous_response_candidate(
            &shared,
            &excluded,
            Some("resp-second"),
            RuntimeRouteKind::Responses,
        )
        .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn duplicate_previous_response_owner_verifies_do_not_requeue_persistence() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    create_codex_home_if_missing(&profile_home).expect("profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    let initial_second = Local::now().timestamp();
    while Local::now().timestamp() == initial_second {
        thread::sleep(Duration::from_millis(5));
    }

    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("first verification should succeed");
    wait_for_runtime_background_queues_idle();

    let first_log =
        fs::read_to_string(&shared.log_path).expect("runtime log should be readable after first bind");
    let binding_marker = "binding previous_response_owner profile=main response_id=resp-1";
    let first_binding_count = first_log.matches(binding_marker).count();
    let first_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(first_binding_count, 1, "first bind should be persisted once: {first_log}");
    assert_eq!(first_revision, 1, "first bind should persist once: {first_log}");

    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("duplicate verification should succeed");
    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("second duplicate verification should succeed");
    wait_for_runtime_background_queues_idle();

    let second_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after duplicate binds");
    assert_eq!(
        second_log.matches(binding_marker).count(),
        first_binding_count,
        "duplicate verifies should not re-log identical owner bindings: {second_log}"
    );
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        first_revision,
        "duplicate verifies should not requeue persistence: {second_log}"
    );
}

#[test]
fn previous_response_owner_profile_changes_still_persist() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    create_codex_home_if_missing(&main_home).expect("main profile home should be created");
    create_codex_home_if_missing(&second_home).expect("second profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    remember_runtime_successful_previous_response_owner(
        &shared,
        "main",
        Some("resp-2"),
        RuntimeRouteKind::Websocket,
    )
    .expect("first owner verification should succeed");
    wait_for_runtime_background_queues_idle();
    let initial_log =
        fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    let initial_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(initial_revision, 1, "first owner save should persist once: {initial_log}");

    remember_runtime_successful_previous_response_owner(
        &shared,
        "second",
        Some("resp-2"),
        RuntimeRouteKind::Responses,
    )
    .expect("owner change verification should succeed");
    wait_for_runtime_background_queues_idle();

    let updated_log =
        fs::read_to_string(&shared.log_path).expect("runtime log should be readable after rebinding");
    assert!(
        updated_log.contains("binding previous_response_owner profile=second response_id=resp-2"),
        "owner changes should still be logged and persisted: {updated_log}"
    );
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        initial_revision + 1,
        "owner changes should queue a fresh persistence update: {updated_log}"
    );

    let runtime = shared
        .runtime
        .lock()
        .expect("runtime state should remain lockable");
    assert_eq!(
        runtime
            .state
            .response_profile_bindings
            .get("resp-2")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
}

#[test]
fn duplicate_response_ids_do_not_requeue_persistence() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    create_codex_home_if_missing(&profile_home).expect("profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    let response_ids = vec!["resp-1".to_string()];
    remember_runtime_response_ids(
        &shared,
        "main",
        &response_ids,
        RuntimeRouteKind::Responses,
    )
    .expect("first response id bind should succeed");
    wait_for_runtime_background_queues_idle();

    let first_log =
        fs::read_to_string(&shared.log_path).expect("runtime log should be readable after first bind");
    let binding_marker = "binding response_ids profile=main count=1 first=Some(\"resp-1\")";
    let first_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(first_log.matches(binding_marker).count(), 1);
    assert_eq!(first_revision, 1, "first bind should persist once: {first_log}");

    remember_runtime_response_ids(
        &shared,
        "main",
        &response_ids,
        RuntimeRouteKind::Responses,
    )
    .expect("duplicate response id bind should succeed");
    remember_runtime_response_ids(
        &shared,
        "main",
        &response_ids,
        RuntimeRouteKind::Responses,
    )
    .expect("second duplicate response id bind should succeed");
    wait_for_runtime_background_queues_idle();

    let second_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after duplicate binds");
    assert_eq!(
        second_log.matches(binding_marker).count(),
        1,
        "duplicate response id binds should not re-log: {second_log}"
    );
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        first_revision,
        "duplicate response id binds should not requeue persistence: {second_log}"
    );
}

#[test]
fn duplicate_non_response_continuation_verifies_do_not_requeue_persistence() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    create_codex_home_if_missing(&profile_home).expect("profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    remember_runtime_turn_state(
        &shared,
        "main",
        Some("turn-1"),
        RuntimeRouteKind::Responses,
    )
    .expect("turn state verification should succeed");
    remember_runtime_session_id(
        &shared,
        "main",
        Some("session-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("session id verification should succeed");
    remember_runtime_compact_lineage(
        &shared,
        "main",
        Some("session-compact"),
        Some("turn-compact"),
        RuntimeRouteKind::Compact,
    )
    .expect("compact lineage verification should succeed");
    wait_for_runtime_background_queues_idle();

    let first_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after initial bindings");
    let first_revision = shared.state_save_revision.load(Ordering::SeqCst);
    assert_eq!(first_revision, 3, "initial binds should persist once each: {first_log}");
    assert_eq!(
        first_log
            .matches("binding turn_state profile=main value=turn-1")
            .count(),
        1,
        "turn state should be logged once: {first_log}"
    );
    assert_eq!(
        first_log
            .matches("binding session_id profile=main value=session-1")
            .count(),
        1,
        "session id should be logged once: {first_log}"
    );

    remember_runtime_turn_state(
        &shared,
        "main",
        Some("turn-1"),
        RuntimeRouteKind::Responses,
    )
    .expect("duplicate turn state verification should succeed");
    remember_runtime_session_id(
        &shared,
        "main",
        Some("session-1"),
        RuntimeRouteKind::Websocket,
    )
    .expect("duplicate session id verification should succeed");
    remember_runtime_compact_lineage(
        &shared,
        "main",
        Some("session-compact"),
        Some("turn-compact"),
        RuntimeRouteKind::Compact,
    )
    .expect("duplicate compact lineage verification should succeed");
    wait_for_runtime_background_queues_idle();

    let second_log = fs::read_to_string(&shared.log_path)
        .expect("runtime log should be readable after duplicate bindings");
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        first_revision,
        "duplicate verifies should not requeue persistence: {second_log}"
    );
    assert_eq!(
        second_log
            .matches("binding turn_state profile=main value=turn-1")
            .count(),
        1,
        "duplicate turn state verifies should not re-log: {second_log}"
    );
    assert_eq!(
        second_log
            .matches("binding session_id profile=main value=session-1")
            .count(),
        1,
        "duplicate session id verifies should not re-log: {second_log}"
    );
}

#[test]
fn runtime_affinity_touch_lookups_do_not_requeue_persistence_before_interval() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    create_codex_home_if_missing(&profile_home).expect("profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let compact_session_key = runtime_compact_session_lineage_key("session-compact");
    let compact_turn_state_key = runtime_compact_turn_state_lineage_key("turn-compact");
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-1".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "session-1".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
    };
    state.save(&paths).expect("state should save");

    let verified_status = |route: &str| RuntimeContinuationBindingStatus {
        state: RuntimeContinuationBindingLifecycle::Verified,
        confidence: 1,
        last_touched_at: Some(now),
        last_verified_at: Some(now),
        last_verified_route: Some(route.to_string()),
        last_not_found_at: None,
        not_found_streak: 0,
        success_count: 1,
        failure_count: 0,
    };

    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::from([
            (
                "turn-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                compact_turn_state_key.clone(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        session_id_bindings: BTreeMap::from([
            (
                "session-1".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                compact_session_key.clone(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        continuation_statuses: RuntimeContinuationStatuses {
            response: BTreeMap::from([(
                "resp-1".to_string(),
                verified_status("responses"),
            )]),
            turn_state: BTreeMap::from([
                ("turn-1".to_string(), verified_status("responses")),
                (
                    compact_turn_state_key.clone(),
                    verified_status("compact"),
                ),
            ]),
            session_id: BTreeMap::from([
                ("session-1".to_string(), verified_status("websocket")),
                (
                    compact_session_key.clone(),
                    verified_status("compact"),
                ),
            ]),
        },
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert_eq!(
        runtime_response_bound_profile(&shared, "resp-1", RuntimeRouteKind::Responses)
            .expect("response owner lookup should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        runtime_turn_state_bound_profile(&shared, "turn-1")
            .expect("turn state lookup should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        runtime_session_bound_profile(&shared, "session-1")
            .expect("session lookup should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        runtime_compact_followup_bound_profile(&shared, Some("turn-compact"), None)
            .expect("compact turn-state lookup should succeed"),
        Some(("main".to_string(), "turn_state"))
    );
    assert_eq!(
        runtime_compact_followup_bound_profile(&shared, None, Some("session-compact"))
            .expect("compact session lookup should succeed"),
        Some(("main".to_string(), "session_id"))
    );
    wait_for_runtime_background_queues_idle();

    let log = fs::read_to_string(&shared.log_path).unwrap_or_default();
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        0,
        "fresh touch lookups should not requeue persistence before the interval elapses: {log}"
    );
    assert!(
        !log.contains("response_touch:resp-1")
            && !log.contains("turn_state_touch:turn-1")
            && !log.contains("session_touch:session-1")
            && !log.contains("compact_turn_state_touch:turn-compact")
            && !log.contains("compact_session_touch:session-compact"),
        "touch lookups should stay in-memory until the persistence interval elapses: {log}"
    );
}

#[test]
fn runtime_rotation_proxy_can_start_even_if_selected_profile_auth_is_not_quota_compatible() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    create_codex_home_if_missing(&main_home).expect("main home should be created");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    assert!(should_enable_runtime_rotation_proxy(&state, "main", true));
}

#[test]
fn runtime_rotation_proxy_stays_disabled_without_any_quota_compatible_profile() {
    let temp_dir = TestDir::new();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/main"),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/second"),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    assert!(!should_enable_runtime_rotation_proxy(&state, "main", true));
}

#[test]
fn optimistic_current_candidate_skips_transport_backoff() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    mark_runtime_profile_transport_backoff(&shared, "main", RuntimeRouteKind::Responses, "test")
        .expect("transport backoff should be recorded");

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn precommit_budget_exhausts_by_attempt_limit_or_elapsed_time() {
    assert!(!runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        0,
        false,
        false
    ));
    assert!(runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
        false,
        false
    ));

    let started_at = Instant::now()
        .checked_sub(Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_BUDGET_MS + 1))
        .expect("elapsed start should be constructible");
    assert!(runtime_proxy_precommit_budget_exhausted(
        started_at, 0, false, false
    ));
    assert!(!runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
        true,
        false
    ));
}

#[test]
fn optimistic_current_candidate_requires_quota_evidence_when_alternatives_exist() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("responses candidate lookup should succeed"),
        None
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
        )
        .expect("websocket candidate lookup should succeed"),
        None
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("standard candidate lookup should succeed"),
        None
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
        )
        .expect("compact candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_recently_unhealthy_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
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
        profile_health: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                updated_at: Local::now().timestamp(),
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_busy_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
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
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT,
        )]),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_thin_long_lived_quota() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
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
                result: Ok(usage_with_main_windows(9, 18_000, 18, 604_800)),
            },
        )]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_cached_usage_exhausted_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileProbeCacheEntry {
                checked_at: Local::now().timestamp(),
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Ok(usage_with_main_windows(0, 18_000, 50, 604_800)),
            },
        )]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn direct_current_fallback_profile_bypasses_local_selection_penalties() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::from([("main".to_string(), now + 60)]),
        profile_transport_backoff_until: BTreeMap::from([(
            runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Standard),
            now + 60,
        )]),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT,
        )]),
        profile_health: BTreeMap::from([(
            runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_direct_current_fallback_profile(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("direct fallback lookup should succeed"),
        Some("main".to_string())
    );
}

#[test]
fn direct_current_fallback_profile_is_route_aware_for_heavy_routes() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
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
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT.saturating_sub(1),
        )]),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_direct_current_fallback_profile(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("standard direct fallback lookup should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        runtime_proxy_direct_current_fallback_profile(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("responses direct fallback lookup should succeed"),
        None
    );
}

#[test]
fn runtime_profile_inflight_hard_limit_detects_saturation() {
    let temp_dir = TestDir::new();
    let runtime = RuntimeRotationState {
        paths: AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        },
        state: AppState::default(),
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
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT,
        )]),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert!(
        runtime_profile_inflight_hard_limited_for_context(&shared, "main", "standard_http")
            .expect("hard inflight lookup should succeed")
    );
    assert!(
        !runtime_profile_inflight_hard_limited_for_context(&shared, "other", "standard_http")
            .expect("hard inflight lookup should succeed")
    );
}

#[test]
fn runtime_profile_inflight_hard_limit_uses_weighted_admission_cost() {
    let temp_dir = TestDir::new();
    let runtime = RuntimeRotationState {
        paths: AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        },
        state: AppState::default(),
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
        profile_inflight: BTreeMap::from([(
            "main".to_string(),
            RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT.saturating_sub(1),
        )]),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert!(
        runtime_profile_inflight_hard_limited_for_context(&shared, "main", "responses_http")
            .expect("weighted hard inflight lookup should succeed")
    );
    assert!(
        !runtime_profile_inflight_hard_limited_for_context(&shared, "main", "standard_http")
            .expect("weighted hard inflight lookup should succeed")
    );
}

#[test]
fn runtime_profile_inflight_weight_prioritizes_long_lived_routes() {
    assert_eq!(runtime_profile_inflight_weight("standard_http"), 1);
    assert_eq!(runtime_profile_inflight_weight("compact_http"), 1);
    assert_eq!(runtime_profile_inflight_weight("responses_http"), 2);
    assert_eq!(runtime_profile_inflight_weight("websocket_session"), 2);
}

#[test]
fn acquire_runtime_profile_inflight_guard_uses_weighted_units() {
    let temp_dir = TestDir::new();
    let runtime = RuntimeRotationState {
        paths: AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        },
        state: AppState::default(),
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    let standard = acquire_runtime_profile_inflight_guard(&shared, "main", "standard_http")
        .expect("standard inflight guard should succeed");
    let responses = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("responses inflight guard should succeed");

    assert_eq!(
        shared
            .runtime
            .lock()
            .expect("runtime should lock")
            .profile_inflight
            .get("main")
            .copied(),
        Some(3),
        "weighted inflight should count long-lived routes heavier than unary routes"
    );

    drop(responses);
    drop(standard);

    assert!(
        shared
            .runtime
            .lock()
            .expect("runtime should lock")
            .profile_inflight
            .get("main")
            .is_none(),
        "weighted inflight release should fully drain the profile count"
    );
}

#[test]
fn transport_backoff_escalates_for_repeated_failures() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    mark_runtime_profile_transport_backoff(&shared, "main", RuntimeRouteKind::Responses, "first")
        .expect("first transport backoff should succeed");
    let first_until = shared
        .runtime
        .lock()
        .expect("runtime should lock")
        .profile_transport_backoff_until
        .get(&runtime_profile_transport_backoff_key(
            "main",
            RuntimeRouteKind::Responses,
        ))
        .copied()
        .expect("first transport backoff should exist");

    mark_runtime_profile_transport_backoff(&shared, "main", RuntimeRouteKind::Responses, "second")
        .expect("second transport backoff should succeed");
    let second_until = shared
        .runtime
        .lock()
        .expect("runtime should lock")
        .profile_transport_backoff_until
        .get(&runtime_profile_transport_backoff_key(
            "main",
            RuntimeRouteKind::Responses,
        ))
        .copied()
        .expect("second transport backoff should exist");

    assert!(
        second_until > first_until,
        "transport backoff should escalate"
    );
}

#[test]
fn local_proxy_overload_backoff_activates_and_expires() {
    let temp_dir = TestDir::new();
    let runtime = RuntimeRotationState {
        paths: AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        },
        state: AppState::default(),
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert!(!runtime_proxy_in_local_overload_backoff(&shared));
    mark_runtime_proxy_local_overload(&shared, "test");
    assert!(runtime_proxy_in_local_overload_backoff(&shared));

    shared
        .local_overload_backoff_until
        .store(0, Ordering::SeqCst);
    assert!(!runtime_proxy_in_local_overload_backoff(&shared));
}

#[test]
fn next_runtime_response_candidate_skips_transport_backoff_when_alternative_is_ready() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::from([(
            runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Responses),
            now.saturating_add(60),
        )]),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn responses_selection_ignores_websocket_transport_backoff() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::from([(
            runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Websocket),
            now.saturating_add(60),
        )]),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("responses candidate selection should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        next_runtime_response_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
        )
        .expect("websocket candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn next_runtime_response_candidate_falls_back_to_soonest_transport_recovery() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::from([
            (
                runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Responses),
                now.saturating_add(90),
            ),
            (
                runtime_profile_transport_backoff_key("second", RuntimeRouteKind::Responses),
                now.saturating_add(30),
            ),
        ]),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn next_runtime_response_candidate_prefers_healthier_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn compact_health_penalty_does_not_degrade_responses_selection() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_health_key("main", RuntimeRouteKind::Compact),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("responses optimistic candidate should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        next_runtime_response_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
        )
        .expect("compact candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn compact_bad_pairing_does_not_degrade_responses_selection() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_bad_pairing_key("main", RuntimeRouteKind::Compact),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate(&shared, &BTreeSet::new())
            .expect("responses optimistic candidate should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        next_runtime_response_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
        )
        .expect("compact candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn websocket_bad_pairing_lightly_degrades_responses_selection() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_bad_pairing_key("main", RuntimeRouteKind::Websocket),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("responses candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn next_runtime_response_candidate_prefers_less_loaded_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::from([("main".to_string(), 2), ("second".to_string(), 0)]),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn next_runtime_response_candidate_prefers_healthier_quota_window_mix() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(4, 18_000, 30, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(25, 18_000, 12, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn next_runtime_response_candidate_prefers_lower_latency_penalty() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: now,
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage_with_main_windows(100, 18_000, 100, 604_800)),
                },
            ),
        ]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_performance_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 9,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn commit_runtime_proxy_profile_selection_clears_profile_health() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
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
        profile_route_circuit_open_until: BTreeMap::from([(
            runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
            now + RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS,
        )]),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileHealth {
                    score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                    updated_at: now,
                },
            ),
            (
                runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
                RuntimeProfileHealth {
                    score: 1,
                    updated_at: now,
                },
            ),
            (
                runtime_profile_route_circuit_reopen_key("main", RuntimeRouteKind::Responses),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: now,
                },
            ),
        ]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    commit_runtime_proxy_profile_selection(&shared, "main", RuntimeRouteKind::Responses)
        .expect("profile commit should succeed");

    assert!(
        shared
            .runtime
            .lock()
            .expect("runtime should lock")
            .profile_health
            .get("main")
            .is_none(),
        "successful commit should clear temporary health penalty"
    );
    let runtime = shared.runtime.lock().expect("runtime should lock");
    assert!(
        !runtime
            .profile_route_circuit_open_until
            .contains_key(&runtime_profile_route_circuit_key(
                "main",
                RuntimeRouteKind::Responses
            )),
        "successful commit should clear the matching route circuit"
    );
    assert!(
        !runtime
            .profile_health
            .contains_key(&runtime_profile_route_health_key(
                "main",
                RuntimeRouteKind::Responses
            )),
        "successful commit should clear the matching route health penalty"
    );
    assert!(
        !runtime
            .profile_health
            .contains_key(&runtime_profile_route_circuit_reopen_key(
                "main",
                RuntimeRouteKind::Responses
            )),
        "successful commit should clear the matching route circuit reopen stage"
    );
}

#[test]
fn commit_runtime_proxy_profile_selection_skips_persist_when_nothing_changed() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    commit_runtime_proxy_profile_selection(&shared, "main", RuntimeRouteKind::Responses)
        .expect("profile commit should succeed");

    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        0,
        "unchanged commit should not enqueue a state save"
    );
}

#[test]
fn commit_runtime_proxy_profile_selection_switches_runtime_but_not_global_profile_for_compact() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    let switched =
        commit_runtime_proxy_profile_selection(&shared, "second", RuntimeRouteKind::Compact)
            .expect("compact profile commit should succeed");

    assert!(
        switched,
        "compact commit should switch the runtime current profile"
    );
    let runtime = shared.runtime.lock().expect("runtime should lock");
    assert_eq!(runtime.current_profile, "second");
    assert_eq!(runtime.state.active_profile.as_deref(), Some("main"));
}

#[test]
fn commit_runtime_proxy_profile_selection_can_skip_current_profile_tracking() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    let switched = commit_runtime_proxy_profile_selection_with_policy(
        &shared,
        "second",
        RuntimeRouteKind::Websocket,
        false,
    )
    .expect("profile commit should succeed");

    assert!(
        !switched,
        "tracked current profile should stay on the heuristic profile"
    );
    assert_eq!(shared.state_save_revision.load(Ordering::SeqCst), 0);
    let runtime = shared.runtime.lock().expect("runtime should lock");
    assert_eq!(runtime.current_profile, "main");
    assert_eq!(runtime.state.active_profile.as_deref(), Some("main"));
    assert!(
        runtime.state.last_run_selected_at.is_empty(),
        "continuation commit should not promote the global active profile"
    );
}

#[test]
fn commit_runtime_proxy_profile_selection_recovers_only_matching_route_profile_health() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
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
        profile_health: BTreeMap::from([
            (
                runtime_profile_route_health_key("main", RuntimeRouteKind::Websocket),
                RuntimeProfileHealth {
                    score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                    updated_at: now,
                },
            ),
            (
                runtime_profile_route_health_key("main", RuntimeRouteKind::Compact),
                RuntimeProfileHealth {
                    score: RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                    updated_at: now,
                },
            ),
        ]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    commit_runtime_proxy_profile_selection(&shared, "main", RuntimeRouteKind::Websocket)
        .expect("profile commit should succeed");

    assert_eq!(
        shared
            .runtime
            .lock()
            .expect("runtime should lock")
            .profile_health
            .get(&runtime_profile_route_health_key(
                "main",
                RuntimeRouteKind::Websocket
            ))
            .map(|entry| entry.score),
        Some(
            RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
                .saturating_sub(RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE)
        ),
        "successful commit should only partially recover heavier route penalties"
    );
    assert!(
        shared
            .runtime
            .lock()
            .expect("runtime should lock")
            .profile_health
            .get(&runtime_profile_route_health_key(
                "main",
                RuntimeRouteKind::Compact
            ))
            .is_some(),
        "successful commit should keep unrelated route health penalty intact"
    );
}

#[test]
fn commit_runtime_proxy_profile_selection_clears_matching_route_bad_pairing() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
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
        profile_health: BTreeMap::from([
            (
                runtime_profile_route_bad_pairing_key("main", RuntimeRouteKind::Websocket),
                RuntimeProfileHealth {
                    score: RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                    updated_at: now,
                },
            ),
            (
                runtime_profile_route_bad_pairing_key("main", RuntimeRouteKind::Compact),
                RuntimeProfileHealth {
                    score: RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                    updated_at: now,
                },
            ),
        ]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    commit_runtime_proxy_profile_selection(&shared, "main", RuntimeRouteKind::Websocket)
        .expect("profile commit should succeed");

    assert!(
        shared
            .runtime
            .lock()
            .expect("runtime should lock")
            .profile_health
            .get(&runtime_profile_route_bad_pairing_key(
                "main",
                RuntimeRouteKind::Websocket
            ))
            .is_none(),
        "successful commit should clear bad pairing memory for the successful route"
    );
    assert!(
        shared
            .runtime
            .lock()
            .expect("runtime should lock")
            .profile_health
            .get(&runtime_profile_route_bad_pairing_key(
                "main",
                RuntimeRouteKind::Compact
            ))
            .is_some(),
        "successful commit should keep unrelated route bad pairing memory intact"
    );
}

#[test]
fn commit_runtime_proxy_profile_selection_accelerates_recovery_after_success_streak() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let route_key = runtime_profile_route_health_key("main", RuntimeRouteKind::Responses);
    let runtime = RuntimeRotationState {
        paths,
        state,
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
        profile_health: BTreeMap::from([(
            route_key.clone(),
            RuntimeProfileHealth {
                score: 5,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    commit_runtime_proxy_profile_selection(&shared, "main", RuntimeRouteKind::Responses)
        .expect("first profile commit should succeed");
    let first_remaining = shared
        .runtime
        .lock()
        .expect("runtime should lock")
        .profile_health
        .get(&route_key)
        .map(|entry| entry.score)
        .expect("first success should keep partial penalty");
    assert_eq!(first_remaining, 3);

    commit_runtime_proxy_profile_selection(&shared, "main", RuntimeRouteKind::Responses)
        .expect("second profile commit should succeed");
    assert!(
        shared
            .runtime
            .lock()
            .expect("runtime should lock")
            .profile_health
            .get(&route_key)
            .is_none(),
        "consecutive successes should accelerate route recovery"
    );
}

#[test]
fn runtime_doctor_json_value_includes_selection_markers() {
    let mut summary = RuntimeDoctorSummary::default();
    summary.line_count = 3;
    summary.marker_counts.insert("selection_pick", 2);
    summary.marker_counts.insert("selection_skip_current", 1);
    summary
        .marker_counts
        .insert("previous_response_not_found", 2);
    summary.marker_counts.insert("compact_followup_owner", 1);
    summary
        .marker_counts
        .insert("compact_fresh_fallback_blocked", 1);
    summary.first_timestamp = Some("2026-03-25 00:00:00.000 +07:00".to_string());
    summary.last_timestamp = Some("2026-03-25 00:00:05.000 +07:00".to_string());
    summary.facet_counts.insert(
        "route".to_string(),
        BTreeMap::from([("responses".to_string(), 2)]),
    );
    summary.facet_counts.insert(
        "quota_source".to_string(),
        BTreeMap::from([("persisted_snapshot".to_string(), 1)]),
    );
    summary.previous_response_not_found_by_route =
        BTreeMap::from([("responses".to_string(), 1), ("websocket".to_string(), 1)]);
    summary.previous_response_not_found_by_transport =
        BTreeMap::from([("http".to_string(), 1), ("websocket".to_string(), 1)]);
    summary.marker_last_fields.insert(
        "selection_pick",
        BTreeMap::from([
            ("profile".to_string(), "second".to_string()),
            ("route".to_string(), "responses".to_string()),
            ("quota_source".to_string(), "persisted_snapshot".to_string()),
        ]),
    );
    summary.diagnosis = "Recent selection decisions were logged.".to_string();
    summary.persisted_verified_continuations = 2;
    summary.persisted_warm_continuations = 1;
    summary.persisted_suspect_continuations = 1;
    summary.persisted_continuation_journal_response_bindings = 3;
    summary.persisted_continuation_journal_session_bindings = 2;
    summary.persisted_continuation_journal_turn_state_bindings = 1;
    summary.persisted_continuation_journal_session_id_bindings = 4;
    summary.state_save_queue_backlog = Some(2);
    summary.state_save_lag_ms = Some(17);
    summary.continuation_journal_save_backlog = Some(1);
    summary.continuation_journal_save_lag_ms = Some(9);
    summary.profile_probe_refresh_backlog = Some(3);
    summary.profile_probe_refresh_lag_ms = Some(5);
    summary.continuation_journal_saved_at = Some(123);
    summary.suspect_continuation_bindings = vec!["turn-second:suspect".to_string()];
    summary.failure_class_counts = BTreeMap::from([
        ("admission".to_string(), 3),
        ("persistence".to_string(), 1),
        ("transport".to_string(), 2),
    ]);
    summary.recovered_continuation_journal_file = true;

    let value = runtime_doctor_json_value(&summary);
    assert_eq!(value["line_count"], 3);
    assert_eq!(value["first_timestamp"], "2026-03-25 00:00:00.000 +07:00");
    assert_eq!(value["last_timestamp"], "2026-03-25 00:00:05.000 +07:00");
    assert_eq!(value["marker_counts"]["selection_pick"], 2);
    assert_eq!(value["marker_counts"]["selection_skip_current"], 1);
    assert_eq!(value["marker_counts"]["previous_response_not_found"], 2);
    assert_eq!(value["marker_counts"]["compact_followup_owner"], 1);
    assert_eq!(value["marker_counts"]["compact_fresh_fallback_blocked"], 1);
    assert_eq!(
        value["previous_response_not_found_by_route"]["responses"],
        1
    );
    assert_eq!(
        value["previous_response_not_found_by_route"]["websocket"],
        1
    );
    assert_eq!(value["previous_response_not_found_by_transport"]["http"], 1);
    assert_eq!(
        value["previous_response_not_found_by_transport"]["websocket"],
        1
    );
    assert_eq!(value["facet_counts"]["route"]["responses"], 2);
    assert_eq!(
        value["facet_counts"]["quota_source"]["persisted_snapshot"],
        1
    );
    assert_eq!(
        value["marker_last_fields"]["selection_pick"]["profile"],
        "second"
    );
    assert_eq!(
        value["marker_last_fields"]["selection_pick"]["quota_source"],
        "persisted_snapshot"
    );
    assert_eq!(value["persisted_verified_continuations"], 2);
    assert_eq!(value["persisted_warm_continuations"], 1);
    assert_eq!(value["persisted_suspect_continuations"], 1);
    assert_eq!(value["persisted_continuation_journal_response_bindings"], 3);
    assert_eq!(value["persisted_continuation_journal_session_bindings"], 2);
    assert_eq!(
        value["persisted_continuation_journal_turn_state_bindings"],
        1
    );
    assert_eq!(
        value["persisted_continuation_journal_session_id_bindings"],
        4
    );
    assert_eq!(value["state_save_queue_backlog"], 2);
    assert_eq!(value["state_save_lag_ms"], 17);
    assert_eq!(value["continuation_journal_save_backlog"], 1);
    assert_eq!(value["continuation_journal_save_lag_ms"], 9);
    assert_eq!(value["profile_probe_refresh_backlog"], 3);
    assert_eq!(value["profile_probe_refresh_lag_ms"], 5);
    assert_eq!(value["continuation_journal_saved_at"], 123);
    assert_eq!(
        value["suspect_continuation_bindings"][0],
        "turn-second:suspect"
    );
    assert_eq!(value["failure_class_counts"]["admission"], 3);
    assert_eq!(value["failure_class_counts"]["persistence"], 1);
    assert_eq!(value["failure_class_counts"]["transport"], 2);
    summary.startup_audit_pressure = "elevated".to_string();
    summary.persisted_retry_backoffs = 2;
    summary.persisted_transport_backoffs = 1;
    summary.persisted_route_circuits = 3;
    summary.persisted_usage_snapshots = 4;
    summary.stale_persisted_usage_snapshots = 1;
    summary.recovered_state_file = true;
    summary.recovered_scores_file = false;
    summary.recovered_usage_snapshots_file = true;
    summary.recovered_backoffs_file = false;
    summary.last_good_backups_present = 3;
    summary.degraded_routes = vec!["main/responses circuit=open until=123".to_string()];
    summary.orphan_managed_dirs = vec!["ghost_profile".to_string()];
    summary.profiles = vec![RuntimeDoctorProfileSummary {
        profile: "main".to_string(),
        quota_freshness: "stale".to_string(),
        quota_age_seconds: 420,
        retry_backoff_until: Some(100),
        transport_backoff_until: Some(200),
        routes: vec![RuntimeDoctorRouteSummary {
            route: "responses".to_string(),
            circuit_state: "open".to_string(),
            circuit_until: Some(200),
            transport_backoff_until: Some(200),
            health_score: 4,
            bad_pairing_score: 2,
            performance_score: 3,
            quota_band: "quota_critical".to_string(),
            five_hour_status: "quota_ready".to_string(),
            weekly_status: "quota_critical".to_string(),
        }],
    }];

    let value = runtime_doctor_json_value(&summary);
    assert_eq!(
        value["diagnosis"],
        "Recent selection decisions were logged."
    );
    assert_eq!(value["startup_audit_pressure"], "elevated");
    assert_eq!(value["persisted_retry_backoffs"], 2);
    assert_eq!(value["persisted_route_circuits"], 3);
    assert_eq!(value["persisted_usage_snapshots"], 4);
    assert_eq!(value["stale_persisted_usage_snapshots"], 1);
    assert_eq!(value["recovered_state_file"], true);
    assert_eq!(value["recovered_usage_snapshots_file"], true);
    assert_eq!(value["recovered_continuation_journal_file"], true);
    assert_eq!(value["last_good_backups_present"], 3);
    assert_eq!(
        value["degraded_routes"][0],
        "main/responses circuit=open until=123"
    );
    assert_eq!(value["orphan_managed_dirs"][0], "ghost_profile");
    assert_eq!(value["profiles"][0]["profile"], "main");
    assert_eq!(value["profiles"][0]["quota_freshness"], "stale");
    assert_eq!(value["profiles"][0]["routes"][0]["route"], "responses");
    assert_eq!(value["profiles"][0]["routes"][0]["performance_score"], 3);
    assert_eq!(
        value["profiles"][0]["routes"][0]["transport_backoff_until"],
        200
    );
}

#[test]
fn runtime_doctor_fields_surface_queue_lag_and_failure_classes() {
    let summary = RuntimeDoctorSummary {
        log_path: Some(PathBuf::from("/tmp/prodex-runtime.log")),
        pointer_exists: true,
        log_exists: true,
        line_count: 8,
        state_save_queue_backlog: Some(4),
        state_save_lag_ms: Some(21),
        continuation_journal_save_backlog: Some(2),
        continuation_journal_save_lag_ms: Some(11),
        profile_probe_refresh_backlog: Some(6),
        profile_probe_refresh_lag_ms: Some(7),
        persisted_suspect_continuations: 2,
        suspect_continuation_bindings: vec![
            "resp-main:suspect".to_string(),
            "turn-main:suspect".to_string(),
        ],
        failure_class_counts: BTreeMap::from([
            ("admission".to_string(), 2),
            ("continuation".to_string(), 1),
            ("transport".to_string(), 3),
        ]),
        diagnosis: "test diagnosis".to_string(),
        ..RuntimeDoctorSummary::default()
    };

    let fields = runtime_doctor_fields_for_summary(
        &summary,
        std::path::Path::new("/tmp/prodex-runtime-latest.path"),
    );
    let fields = fields.into_iter().collect::<BTreeMap<_, _>>();

    assert_eq!(
        fields.get("State save backlog").map(String::as_str),
        Some("4")
    );
    assert_eq!(fields.get("State save lag").map(String::as_str), Some("21"));
    assert_eq!(
        fields.get("Cont journal backlog").map(String::as_str),
        Some("2")
    );
    assert_eq!(
        fields.get("Cont journal lag").map(String::as_str),
        Some("11")
    );
    assert_eq!(fields.get("Probe backlog").map(String::as_str), Some("6"));
    assert_eq!(fields.get("Probe lag").map(String::as_str), Some("7"));
    assert_eq!(
        fields.get("Failure classes").map(String::as_str),
        Some("admission=2, continuation=1, transport=3")
    );
    assert_eq!(
        fields.get("Suspect continuations").map(String::as_str),
        Some("count=2 bindings=resp-main:suspect, turn-main:suspect")
    );
}

#[test]
fn collect_orphan_managed_profile_dirs_ignores_tracked_and_fresh_dirs() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    let tracked = paths.managed_profiles_root.join("tracked");
    fs::create_dir_all(&tracked).expect("tracked dir should exist");
    fs::write(tracked.join("auth.json"), "{}").expect("tracked auth should be written");

    let orphan = paths.managed_profiles_root.join("orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan auth should be written");

    let fresh = paths.managed_profiles_root.join("fresh");
    fs::create_dir_all(&fresh).expect("fresh dir should exist");
    fs::write(fresh.join("auth.json"), "{}").expect("fresh auth should be written");

    let state = AppState {
        active_profile: Some("tracked".to_string()),
        profiles: BTreeMap::from([(
            "tracked".to_string(),
            ProfileEntry {
                codex_home: tracked,
                managed: true,
                email: Some("tracked@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let future_now = SystemTime::now()
        + Duration::from_secs(ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS as u64 + 5);
    assert_eq!(
        collect_orphan_managed_profile_dirs_at(&paths, &state, future_now),
        vec!["fresh".to_string(), "orphan".to_string()]
    );
}

#[test]
fn runtime_doctor_state_collects_persisted_degradation_and_orphans() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = paths.managed_profiles_root.join("main");
    fs::create_dir_all(&main_home).expect("main home should exist");
    fs::write(main_home.join("auth.json"), "{}").expect("main auth should be written");
    let orphan = paths.managed_profiles_root.join("orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan auth should be written");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");
    let usage_snapshots = BTreeMap::from([(
        "main".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: Local::now().timestamp(),
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 50,
            five_hour_reset_at: 123,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 50,
            weekly_reset_at: 456,
        },
    )]);
    let mut saved_usage_snapshots = false;
    for _ in 0..20 {
        match save_runtime_usage_snapshots(&paths, &usage_snapshots) {
            Ok(()) => {
                saved_usage_snapshots = true;
                break;
            }
            Err(err) => {
                if !err
                    .to_string()
                    .contains("failed to read /tmp/prodex-runtime-test")
                {
                    panic!("usage snapshots should save: {err:#}");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    assert!(saved_usage_snapshots, "usage snapshots should save");
    save_runtime_profile_scores(
        &paths,
        &BTreeMap::from([(
            "__route_bad_pairing__:responses:main".to_string(),
            RuntimeProfileHealth {
                score: 3,
                updated_at: Local::now().timestamp(),
            },
        )]),
    )
    .expect("scores should save");
    save_runtime_profile_backoffs(
        &paths,
        &RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([(
                "main".to_string(),
                Local::now().timestamp() + 30,
            )]),
            transport_backoff_until: BTreeMap::new(),
            route_circuit_open_until: BTreeMap::from([(
                "__route_circuit__:responses:main".to_string(),
                Local::now().timestamp() + 30,
            )]),
        },
    )
    .expect("backoffs should save");
    let journal_saved_at = Local::now().timestamp();
    save_runtime_continuation_journal(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: journal_saved_at,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Suspect,
                        confidence: 1,
                        last_touched_at: Some(journal_saved_at),
                        last_verified_at: None,
                        last_verified_route: None,
                        last_not_found_at: Some(journal_saved_at),
                        not_found_streak: 1,
                        success_count: 0,
                        failure_count: 1,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        journal_saved_at,
    )
    .expect("continuation journal should save");

    let _guard = TestEnvVarGuard::set("PRODEX_HOME", paths.root.to_str().unwrap());
    let mut summary = RuntimeDoctorSummary::default();
    collect_runtime_doctor_state(&paths, &mut summary);

    assert_eq!(summary.persisted_retry_backoffs, 1);
    assert_eq!(summary.persisted_route_circuits, 1);
    assert_eq!(summary.persisted_usage_snapshots, 1);
    assert_eq!(summary.persisted_continuation_journal_response_bindings, 1);
    assert_eq!(summary.persisted_suspect_continuations, 1);
    assert_eq!(summary.persisted_dead_continuations, 0);
    assert_eq!(
        summary.continuation_journal_saved_at,
        Some(journal_saved_at)
    );
    assert_eq!(
        summary.suspect_continuation_bindings,
        vec!["resp-main:suspect".to_string()]
    );
    assert!(
        summary
            .degraded_routes
            .iter()
            .any(|line| line.contains("main/responses"))
    );
}

#[test]
fn optimistic_current_candidate_skips_persisted_exhausted_snapshot() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: Local::now().timestamp(),
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: Local::now().timestamp() + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 80,
                weekly_reset_at: Local::now().timestamp() + 86_400,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_skips_route_performance_penalty() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
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
                result: Ok(usage_with_main_windows(90, 18_000, 90, 604_800)),
            },
        )]),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_performance_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 4,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_allows_single_profile_persisted_snapshot_fast_path() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: Local::now().timestamp(),
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 85,
                five_hour_reset_at: Local::now().timestamp() + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 90,
                weekly_reset_at: Local::now().timestamp() + 86_400,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("responses candidate lookup should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Websocket,
        )
        .expect("websocket candidate lookup should succeed"),
        Some("main".to_string())
    );
    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("standard candidate lookup should succeed"),
        Some("main".to_string())
    );
}

#[test]
fn optimistic_current_candidate_allows_standard_fast_path_with_persisted_snapshot_even_with_alternatives()
 {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
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
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
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
        profile_usage_snapshots: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 80,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 80,
                weekly_reset_at: now + 86_400,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("standard candidate lookup should succeed"),
        Some("main".to_string())
    );
}

#[test]
fn optimistic_current_candidate_allows_single_profile_standard_with_unknown_quota() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let runtime = RuntimeRotationState {
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
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
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
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("standard candidate lookup should succeed"),
        Some("main".to_string())
    );
}

#[test]
fn affinity_candidate_skips_persisted_exhausted_session_owner() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-123".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::from([(
            "sess-123".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 80,
                    five_hour_reset_at: now + 18_000,
                    weekly_status: RuntimeQuotaWindowStatus::Exhausted,
                    weekly_remaining_percent: 0,
                    weekly_reset_at: now + 300,
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 90,
                    five_hour_reset_at: now + 18_000,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 95,
                    weekly_reset_at: now + 604_800,
                },
            ),
        ]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            None,
            None,
            None,
            Some("main"),
            false,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn optimistic_current_candidate_skips_open_route_circuit() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let runtime = RuntimeRotationState {
        paths,
        state: AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
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
        profile_usage_snapshots: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: Local::now().timestamp(),
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 80,
                five_hour_reset_at: Local::now().timestamp() + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 80,
                weekly_reset_at: Local::now().timestamp() + 86_400,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::from([(
            runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
            Local::now().timestamp() + 60,
        )]),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Responses,
        )
        .expect("candidate lookup should succeed"),
        None
    );
}

#[test]
fn optimistic_current_candidate_ignores_auth_failure_backoff_after_auth_json_changes() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    let cached_usage_auth = load_runtime_profile_usage_auth_cache_entry(&main_home)
        .expect("usage auth cache entry should load");

    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let runtime = RuntimeRotationState {
        paths,
        state: AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        },
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::from([("main".to_string(), cached_usage_auth)]),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("candidate lookup should succeed"),
        None
    );

    write_auth_json(&main_home.join("auth.json"), "main-account-refreshed");

    assert_eq!(
        runtime_proxy_optimistic_current_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )
        .expect("candidate lookup should succeed after auth.json changes"),
        Some("main".to_string())
    );
}

#[test]
fn note_runtime_profile_auth_failure_applies_stronger_penalty_for_401_than_403() {
    let temp_dir_401 = TestDir::new();
    let temp_dir_403 = TestDir::new();
    let profile_home_401 = temp_dir_401.path.join("homes/main");
    let profile_home_403 = temp_dir_403.path.join("homes/main");
    write_auth_json(&profile_home_401.join("auth.json"), "main-account");
    write_auth_json(&profile_home_403.join("auth.json"), "main-account");

    let shared_401 = runtime_rotation_proxy_shared(
        &temp_dir_401,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir_401.path.join("prodex"),
                state_file: temp_dir_401.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir_401.path.join("prodex/profiles"),
                shared_codex_root: temp_dir_401.path.join("shared"),
                legacy_shared_codex_root: temp_dir_401.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: profile_home_401,
                        managed: true,
                        email: Some("main@example.com".to_string()),
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
        usize::MAX,
    );
    let shared_403 = runtime_rotation_proxy_shared(
        &temp_dir_403,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir_403.path.join("prodex"),
                state_file: temp_dir_403.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir_403.path.join("prodex/profiles"),
                shared_codex_root: temp_dir_403.path.join("shared"),
                legacy_shared_codex_root: temp_dir_403.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: profile_home_403,
                        managed: true,
                        email: Some("main@example.com".to_string()),
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
        usize::MAX,
    );

    note_runtime_profile_auth_failure(&shared_401, "main", RuntimeRouteKind::Responses, 401);
    note_runtime_profile_auth_failure(&shared_403, "main", RuntimeRouteKind::Responses, 403);

    let score_401 = shared_401
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_health
        .get(&runtime_profile_auth_failure_key("main"))
        .expect("401 auth failure score should exist")
        .score;
    let score_403 = shared_403
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_health
        .get(&runtime_profile_auth_failure_key("main"))
        .expect("403 auth failure score should exist")
        .score;

    assert!(score_401 > score_403);
    wait_for_runtime_background_queues_idle();
}

#[test]
fn next_runtime_response_candidate_skips_auth_failed_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let runtime = RuntimeRotationState {
        paths,
        state: AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
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
        profile_usage_snapshots: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 80,
                    five_hour_reset_at: now + 300,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 80,
                    weekly_reset_at: now + 86_400,
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 90,
                    five_hour_reset_at: now + 300,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 90,
                    weekly_reset_at: now + 86_400,
                },
            ),
        ]),
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn next_runtime_response_candidate_sync_probes_cold_start_when_existing_candidate_is_auth_failed() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let runtime = RuntimeRotationState {
        paths,
        state: AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        },
        upstream_base_url: backend.base_url(),
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );

    let usage_accounts = backend.usage_accounts();
    assert!(
        !usage_accounts.is_empty(),
        "cold-start selection should probe the newly selected profile"
    );
    assert!(
        usage_accounts
            .iter()
            .all(|account| account == "second-account"),
        "cold-start selection should only probe the alternate owner: {usage_accounts:?}"
    );
    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        runtime.profile_probe_cache.contains_key("second"),
        "cold-start selection should cache the newly probed profile"
    );
    assert!(
        runtime.profile_usage_snapshots.contains_key("second"),
        "cold-start selection should persist the newly probed usage snapshot"
    );
}

#[test]
fn next_runtime_response_candidate_skips_sync_cold_start_probe_during_pressure_mode() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let runtime = RuntimeRotationState {
        paths,
        state: AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: true,
                        email: Some("second@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        },
        upstream_base_url: backend.base_url(),
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
    };
    let shared = RuntimeRotationProxyShared {
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
        local_overload_backoff_until: Arc::new(AtomicU64::new(
            Local::now().timestamp().max(0) as u64 + 60,
        )),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        next_runtime_response_candidate(&shared, &BTreeSet::new())
            .expect("candidate lookup should succeed"),
        None
    );
    assert!(
        backend.usage_accounts().is_empty(),
        "pressure mode should defer cold-start probing to the background queue"
    );

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime.profile_probe_cache.contains_key("second"),
        "pressure mode should avoid writing a fresh sync probe result"
    );
}

#[test]
fn responses_session_affinity_skips_profiles_without_usable_quota_data() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-unknown".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "second".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::from([(
            "sess-unknown".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([(
            "second".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 90,
                five_hour_reset_at: now + 18_000,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 95,
                weekly_reset_at: now + 604_800,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            None,
            None,
            None,
            Some("main"),
            false,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn responses_compact_followup_affinity_allows_owner_without_runtime_quota_data() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            Some("second"),
            None,
            None,
            None,
            false,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn previous_response_discovery_skips_exhausted_current_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 80,
                    five_hour_reset_at: now + 18_000,
                    weekly_status: RuntimeQuotaWindowStatus::Exhausted,
                    weekly_remaining_percent: 0,
                    weekly_reset_at: now + 300,
                },
            ),
            (
                "second".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 90,
                    five_hour_reset_at: now + 18_000,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 95,
                    weekly_reset_at: now + 604_800,
                },
            ),
        ]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert_eq!(
        select_runtime_response_candidate_for_route(
            &shared,
            &BTreeSet::new(),
            None,
            None,
            None,
            None,
            true,
            Some("resp-second"),
            RuntimeRouteKind::Responses,
        )
        .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn merge_runtime_usage_snapshots_keeps_newer_entries() {
    let now = Local::now().timestamp();
    let existing = BTreeMap::from([(
        "main".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: now - 20,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 70,
            five_hour_reset_at: now + 100,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 80,
            weekly_reset_at: now + 200,
        },
    )]);
    let incoming = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now - 10,
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Critical,
                weekly_remaining_percent: 5,
                weekly_reset_at: now + 400,
            },
        ),
        (
            "stale".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now - 10,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 100,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 100,
                weekly_reset_at: now + 400,
            },
        ),
    ]);
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: PathBuf::from("/tmp/main"),
            managed: true,
            email: None,
        },
    )]);

    let merged = merge_runtime_usage_snapshots(&existing, &incoming, &profiles);
    assert_eq!(merged.len(), 1);
    assert_eq!(
        merged
            .get("main")
            .expect("main snapshot should exist")
            .checked_at,
        now - 10
    );
    assert_eq!(
        merged
            .get("main")
            .expect("main snapshot should exist")
            .five_hour_status,
        RuntimeQuotaWindowStatus::Exhausted
    );
}

#[test]
fn runtime_profile_selection_jitter_is_deterministic_for_same_sequence() {
    let temp_dir = TestDir::new();
    let shared = RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(42)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
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
        })),
    };

    let first = runtime_profile_selection_jitter(&shared, "main", RuntimeRouteKind::Responses);
    let second = runtime_profile_selection_jitter(&shared, "main", RuntimeRouteKind::Responses);
    assert_eq!(first, second);
}

#[test]
fn runtime_profile_transport_health_penalty_weights_connect_failures_higher() {
    assert_eq!(
        runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::ReadTimeout),
        RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
    );
    assert_eq!(
        runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::ConnectTimeout),
        RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY
    );
    assert_eq!(
        runtime_profile_transport_health_penalty(RuntimeTransportFailureKind::BrokenPipe),
        RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
    );
}

#[test]
fn app_state_save_merges_existing_runtime_bindings() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let existing = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/main"),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/second"),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now - 20)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-existing".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 20,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-existing".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 20,
            },
        )]),
    };
    existing
        .save(&paths)
        .expect("initial state save should succeed");

    let desired = AppState {
        active_profile: Some("second".to_string()),
        profiles: existing.profiles.clone(),
        last_run_selected_at: BTreeMap::from([("second".to_string(), now - 10)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    desired
        .save(&paths)
        .expect("merged state save should succeed");

    let loaded = AppState::load(&paths).expect("state should reload");
    assert_eq!(loaded.active_profile.as_deref(), Some("second"));
    assert_eq!(
        loaded
            .response_profile_bindings
            .get("resp-existing")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert_eq!(
        loaded
            .session_profile_bindings
            .get("sess-existing")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert_eq!(
        loaded.last_run_selected_at.get("main").copied(),
        Some(now - 20)
    );
    assert_eq!(
        loaded.last_run_selected_at.get("second").copied(),
        Some(now - 10)
    );
}

#[test]
fn app_state_housekeeping_prunes_stale_entries_on_save() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let stale_last_run = now - APP_STATE_LAST_RUN_RETENTION_SECONDS - 5;
    let stale_response_binding = now - 365 * 24 * 60 * 60;
    let stale_session_binding = now - APP_STATE_SESSION_BINDING_RETENTION_SECONDS - 5;
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::from([
            ("main".to_string(), now),
            ("ghost".to_string(), stale_last_run),
        ]),
        response_profile_bindings: BTreeMap::from([
            (
                "resp-fresh".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "resp-stale".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_response_binding,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([
            (
                "sess-fresh".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "sess-stale".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_session_binding,
                },
            ),
        ]),
    };
    state.save(&paths).expect("state should save");

    let loaded = AppState::load(&paths).expect("state should reload");
    let raw = fs::read_to_string(&paths.state_file).expect("state file should be readable");
    assert!(loaded.last_run_selected_at.contains_key("main"));
    assert!(!loaded.last_run_selected_at.contains_key("ghost"));
    assert!(loaded.response_profile_bindings.contains_key("resp-fresh"));
    assert!(loaded.response_profile_bindings.contains_key("resp-stale"));
    assert!(loaded.session_profile_bindings.contains_key("sess-fresh"));
    assert!(!loaded.session_profile_bindings.contains_key("sess-stale"));
    assert!(raw.contains("resp-fresh"));
    assert!(raw.contains("resp-stale"));
    assert!(!raw.contains("sess-stale"));
}

#[test]
fn app_state_load_compacts_stale_entries_in_memory() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(
        paths
            .state_file
            .parent()
            .expect("state file should have a parent"),
    )
    .expect("state dir should exist");
    let now = Local::now().timestamp();
    let stale_last_run = now - APP_STATE_LAST_RUN_RETENTION_SECONDS - 5;
    let stale_session_binding = now - APP_STATE_SESSION_BINDING_RETENTION_SECONDS - 5;
    let stale_response_binding = now - 365 * 24 * 60 * 60;
    let raw = serde_json::json!({
        "active_profile": "main",
        "profiles": {
            "main": {
                "codex_home": temp_dir.path.join("homes/main"),
                "managed": true,
                "email": "main@example.com"
            }
        },
        "last_run_selected_at": {
            "main": now,
            "ghost": stale_last_run
        },
        "response_profile_bindings": {
            "resp-stale": {
                "profile_name": "main",
                "bound_at": stale_response_binding
            }
        },
        "session_profile_bindings": {
            "sess-stale": {
                "profile_name": "main",
                "bound_at": stale_session_binding
            }
        }
    });
    fs::write(
        &paths.state_file,
        serde_json::to_string_pretty(&raw).expect("raw json should serialize"),
    )
    .expect("raw state should write");

    let loaded = AppState::load(&paths).expect("state should load");
    assert_eq!(loaded.active_profile.as_deref(), Some("main"));
    assert!(loaded.last_run_selected_at.contains_key("main"));
    assert!(!loaded.last_run_selected_at.contains_key("ghost"));
    assert!(loaded.response_profile_bindings.contains_key("resp-stale"));
    assert!(loaded.session_profile_bindings.is_empty());
}

#[test]
fn app_state_response_bindings_are_not_pruned_just_for_size() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let mut response_profile_bindings = BTreeMap::new();
    for index in 0..(RESPONSE_PROFILE_BINDING_LIMIT + 3) {
        response_profile_bindings.insert(
            format!("resp-{index:06}-{}", "x".repeat(64)),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now + index as i64,
            },
        );
    }
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings,
        session_profile_bindings: BTreeMap::new(),
    };

    let compacted = compact_app_state(state, now);
    assert_eq!(
        compacted.response_profile_bindings.len(),
        RESPONSE_PROFILE_BINDING_LIMIT + 3
    );
    assert!(compacted.response_profile_bindings.contains_key(&format!(
        "resp-{:06}-{}",
        0,
        "x".repeat(64)
    )));
}

#[test]
fn runtime_sidecar_housekeeping_prunes_stale_entries() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let stale = now - RUNTIME_SCORE_RETENTION_SECONDS - 5;

    let scores = compact_runtime_profile_scores(
        BTreeMap::from([
            (
                runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
                RuntimeProfileHealth {
                    score: 4,
                    updated_at: now,
                },
            ),
            (
                runtime_profile_route_bad_pairing_key("main", RuntimeRouteKind::Compact),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: stale,
                },
            ),
        ]),
        &profiles,
        now,
    );
    assert!(scores.contains_key(&runtime_profile_route_health_key(
        "main",
        RuntimeRouteKind::Responses
    )));
    assert!(!scores.contains_key(&runtime_profile_route_bad_pairing_key(
        "main",
        RuntimeRouteKind::Compact
    )));

    let snapshots = compact_runtime_usage_snapshots(
        BTreeMap::from([
            (
                "main".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: now,
                    five_hour_status: RuntimeQuotaWindowStatus::Ready,
                    five_hour_remaining_percent: 90,
                    five_hour_reset_at: now + 300,
                    weekly_status: RuntimeQuotaWindowStatus::Ready,
                    weekly_remaining_percent: 95,
                    weekly_reset_at: now + 600,
                },
            ),
            (
                "ghost".to_string(),
                RuntimeProfileUsageSnapshot {
                    checked_at: stale,
                    five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                    five_hour_remaining_percent: 0,
                    five_hour_reset_at: now + 300,
                    weekly_status: RuntimeQuotaWindowStatus::Exhausted,
                    weekly_remaining_percent: 0,
                    weekly_reset_at: now + 600,
                },
            ),
        ]),
        &profiles,
        now,
    );
    assert!(snapshots.contains_key("main"));
    assert!(!snapshots.contains_key("ghost"));

    let backoffs = compact_runtime_profile_backoffs(
        RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([
                ("main".to_string(), now + 60),
                ("ghost".to_string(), now + 60),
            ]),
            transport_backoff_until: BTreeMap::from([(
                runtime_profile_transport_backoff_key("main", RuntimeRouteKind::Responses),
                now - 1,
            )]),
            route_circuit_open_until: BTreeMap::from([
                (
                    runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
                    now + 60,
                ),
                (
                    runtime_profile_route_circuit_key("ghost", RuntimeRouteKind::Responses),
                    now + 60,
                ),
            ]),
        },
        &profiles,
        now,
    );
    assert!(backoffs.retry_backoff_until.contains_key("main"));
    assert!(!backoffs.retry_backoff_until.contains_key("ghost"));
    assert!(backoffs.transport_backoff_until.is_empty());
    assert!(
        backoffs
            .route_circuit_open_until
            .contains_key(&runtime_profile_route_circuit_key(
                "main",
                RuntimeRouteKind::Responses
            ))
    );
    assert!(
        !backoffs
            .route_circuit_open_until
            .contains_key(&runtime_profile_route_circuit_key(
                "ghost",
                RuntimeRouteKind::Responses
            ))
    );
}

#[test]
fn runtime_log_housekeeping_prunes_old_logs_and_stale_pointer() {
    let temp_dir = TestDir::new();
    let old_one = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-1.log"));
    let old_two = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-2.log"));
    let keep_one = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-3.log"));
    let keep_two = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-4.log"));
    let keep_three = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-5.log"));
    let keep_four = temp_dir
        .path
        .join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-111-6.log"));
    fs::write(&old_one, "old").expect("old log should write");
    fs::write(&old_two, "old").expect("old log should write");
    fs::write(&keep_one, "keep").expect("keep log should write");
    fs::write(&keep_two, "keep").expect("keep log should write");
    fs::write(&keep_three, "keep").expect("keep log should write");
    fs::write(&keep_four, "keep").expect("keep log should write");

    let pointer = temp_dir.path.join(RUNTIME_PROXY_LATEST_LOG_POINTER);
    fs::write(
        &pointer,
        format!("{}\n", temp_dir.path.join("missing.log").display()),
    )
    .expect("pointer should write");

    cleanup_runtime_proxy_logs_in_dir(&temp_dir.path, SystemTime::now());
    cleanup_runtime_proxy_latest_pointer(&pointer);

    assert!(!old_one.exists());
    assert!(!old_two.exists());
    assert!(keep_one.exists());
    assert!(keep_two.exists());
    assert!(keep_three.exists());
    assert!(keep_four.exists());
    assert!(!pointer.exists());
}

#[test]
fn stale_login_dir_housekeeping_removes_old_temp_login_homes() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    let stale_login = paths.root.join(".login-123-1-0");
    fs::create_dir_all(&stale_login).expect("stale login dir should exist");

    let simulated_now = SystemTime::now()
        .checked_add(Duration::from_secs(
            (PROD_EX_TMP_LOGIN_RETENTION_SECONDS + 5).max(1) as u64,
        ))
        .expect("simulated clock should be valid");
    cleanup_stale_login_dirs_at(&paths, simulated_now);

    assert!(!stale_login.exists());
}

#[test]
fn perform_prodex_cleanup_removes_safe_local_artifacts() {
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    fs::create_dir_all(&runtime_log_dir).expect("runtime log dir should exist");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    for index in 0..=RUNTIME_PROXY_LOG_RETENTION_COUNT {
        let path = runtime_log_dir.join(format!(
            "{RUNTIME_PROXY_LOG_FILE_PREFIX}-cleanup-{index}.log"
        ));
        fs::write(path, "log").expect("runtime log should write");
    }
    let pointer = runtime_log_dir.join(RUNTIME_PROXY_LATEST_LOG_POINTER);
    fs::write(
        &pointer,
        format!("{}\n", runtime_log_dir.join("missing.log").display()),
    )
    .expect("pointer should write");

    let stale_login = paths.root.join(".login-123-1-0");
    fs::create_dir_all(&stale_login).expect("stale login dir should exist");

    let tracked = paths.managed_profiles_root.join("tracked");
    fs::create_dir_all(&tracked).expect("tracked dir should exist");
    fs::write(tracked.join("auth.json"), "{}").expect("tracked auth should be written");

    let orphan = paths.managed_profiles_root.join("orphan");
    fs::create_dir_all(&orphan).expect("orphan dir should exist");
    fs::write(orphan.join("auth.json"), "{}").expect("orphan auth should be written");

    let state = AppState {
        active_profile: Some("tracked".to_string()),
        profiles: BTreeMap::from([(
            "tracked".to_string(),
            ProfileEntry {
                codex_home: tracked.clone(),
                managed: true,
                email: Some("tracked@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let stale_broker_key = "cleanup-stale";
    save_runtime_broker_registry(
        &paths,
        stale_broker_key,
        &RuntimeBrokerRegistry {
            pid: 999_999_999,
            listen_addr: "127.0.0.1:1".to_string(),
            started_at: 1,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "tracked".to_string(),
            instance_token: "stale-instance".to_string(),
            admin_token: "stale-admin".to_string(),
            openai_mount_path: None,
        },
    )
    .expect("stale runtime broker registry should save");
    let stale_lease_dir = runtime_broker_lease_dir(&paths, stale_broker_key);
    fs::create_dir_all(&stale_lease_dir).expect("stale lease dir should exist");
    let stale_lease = stale_lease_dir.join("999999999-stale.lease");
    let live_lease = stale_lease_dir.join(format!("{}-live.lease", std::process::id()));
    fs::write(&stale_lease, "stale").expect("stale lease should write");
    fs::write(&live_lease, "live").expect("live lease should write");

    let lease_only_key = "cleanup-lease-only";
    let lease_only_dir = runtime_broker_lease_dir(&paths, lease_only_key);
    fs::create_dir_all(&lease_only_dir).expect("lease-only dir should exist");
    let lease_only_stale = lease_only_dir.join("999999998-stale.lease");
    fs::write(&lease_only_stale, "stale").expect("lease-only stale lease should write");

    let future_offset_seconds = (ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS
        .max(PROD_EX_TMP_LOGIN_RETENTION_SECONDS)
        + 5) as u64;
    let simulated_now = SystemTime::now()
        .checked_add(Duration::from_secs(future_offset_seconds))
        .expect("simulated clock should be valid");
    let summary =
        perform_prodex_cleanup_at(&paths, &state, &runtime_log_dir, &pointer, simulated_now)
            .expect("cleanup should succeed");

    let log_count = RUNTIME_PROXY_LOG_RETENTION_COUNT + 1;
    let expected_runtime_logs_removed =
        if future_offset_seconds as i64 >= RUNTIME_PROXY_LOG_RETENTION_SECONDS {
            log_count
        } else {
            1
        };
    assert_eq!(summary.runtime_logs_removed, expected_runtime_logs_removed);
    assert_eq!(summary.stale_runtime_log_pointer_removed, 1);
    assert_eq!(summary.stale_login_dirs_removed, 1);
    assert_eq!(summary.orphan_managed_profile_dirs_removed, 1);
    assert_eq!(summary.dead_runtime_broker_leases_removed, 2);
    assert_eq!(summary.dead_runtime_broker_registries_removed, 1);
    assert_eq!(
        summary.total_removed(),
        expected_runtime_logs_removed + 6
    );

    assert!(!pointer.exists(), "stale runtime pointer should be removed");
    assert!(!stale_login.exists(), "stale login dir should be removed");
    assert!(!orphan.exists(), "orphan managed dir should be removed");
    assert!(tracked.exists(), "tracked managed dir should remain");
    assert_eq!(
        prodex_runtime_log_paths_in_dir(&runtime_log_dir).len(),
        log_count.saturating_sub(expected_runtime_logs_removed)
    );
    assert!(
        !runtime_broker_registry_file_path(&paths, stale_broker_key).exists(),
        "stale runtime broker registry should be removed"
    );
    assert!(
        !runtime_broker_registry_last_good_file_path(&paths, stale_broker_key).exists(),
        "stale runtime broker registry backup should be removed"
    );
    assert!(!stale_lease.exists(), "dead lease should be removed");
    assert!(
        !lease_only_stale.exists(),
        "dead lease without registry should be removed"
    );
    assert!(live_lease.exists(), "live lease should remain");
}

#[test]
fn runtime_state_snapshot_save_preserves_concurrent_profiles() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let existing = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/main"),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: temp_dir.path.join("homes/second"),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([("second".to_string(), now - 20)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    existing
        .save(&paths)
        .expect("initial state save should succeed");

    let snapshot = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now - 10)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
    };
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_snapshot_if_latest(
            &paths,
            &snapshot,
            &runtime_continuation_store_from_app_state(&snapshot),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &RuntimeProfileBackoffs::default(),
            1,
            &revision,
        )
        .expect("runtime snapshot save should succeed")
    );

    let loaded = AppState::load(&paths).expect("state should reload");
    assert_eq!(loaded.active_profile.as_deref(), Some("main"));
    assert!(loaded.profiles.contains_key("second"));
    assert_eq!(
        loaded
            .response_profile_bindings
            .get("resp-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert_eq!(
        loaded
            .session_profile_bindings
            .get("sess-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert_eq!(
        loaded.last_run_selected_at.get("second").copied(),
        Some(now - 20)
    );
    assert_eq!(
        loaded.last_run_selected_at.get("main").copied(),
        Some(now - 10)
    );
}

#[test]
fn runtime_state_save_scheduler_persists_latest_snapshot() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([
        (
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        ),
        (
            "second".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/second"),
                managed: true,
                email: Some("second@example.com".to_string()),
            },
        ),
    ]);
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state: AppState::default(),
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
        })),
    };

    let first_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now - 20)]),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    schedule_runtime_state_save(
        &shared,
        first_state.clone(),
        runtime_continuation_store_from_app_state(&first_state),
        BTreeMap::from([(
            runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
                updated_at: now - 20,
            },
        )]),
        BTreeMap::new(),
        RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([("main".to_string(), now + 60)]),
            transport_backoff_until: BTreeMap::new(),
            route_circuit_open_until: BTreeMap::new(),
        },
        paths.clone(),
        "first",
    );
    let second_state = AppState {
        active_profile: Some("second".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("second".to_string(), now - 10)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now - 10,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now - 10,
            },
        )]),
    };
    schedule_runtime_state_save(
        &shared,
        second_state.clone(),
        runtime_continuation_store_from_app_state(&second_state),
        BTreeMap::from([(
            runtime_profile_route_health_key("second", RuntimeRouteKind::Compact),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                updated_at: now - 10,
            },
        )]),
        BTreeMap::new(),
        RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::new(),
            transport_backoff_until: BTreeMap::from([(
                runtime_profile_transport_backoff_key("second", RuntimeRouteKind::Responses),
                now + 120,
            )]),
            route_circuit_open_until: BTreeMap::new(),
        },
        paths.clone(),
        "second",
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
            && state
                .response_profile_bindings
                .get("resp-second")
                .is_some_and(|binding| binding.profile_name == "second")
            && state
                .session_profile_bindings
                .get("sess-second")
                .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
    assert_eq!(
        persisted.last_run_selected_at.get("second").copied(),
        Some(now - 10)
    );
    assert_eq!(
        persisted
            .session_profile_bindings
            .get("sess-second")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
    let persisted_scores =
        load_runtime_profile_scores(&paths, &profiles).expect("runtime scores should reload");
    let persisted_backoffs =
        load_runtime_profile_backoffs(&paths, &profiles).expect("runtime backoffs should reload");
    assert!(
        persisted_scores.contains_key(&runtime_profile_route_health_key(
            "second",
            RuntimeRouteKind::Compact
        )),
        "latest queued runtime scores should persist alongside state"
    );
    assert!(
        persisted_backoffs
            .transport_backoff_until
            .get(&runtime_profile_transport_backoff_key(
                "second",
                RuntimeRouteKind::Responses,
            ))
        .is_some_and(|until| *until > Local::now().timestamp())
    );
}

#[test]
fn runtime_backoffs_load_legacy_last_good_backup_when_primary_is_invalid() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);

    let now = Local::now().timestamp();
    let legacy_backup = serde_json::json!({
        "retry_backoff_until": {
            "main": now + 600
        },
        "transport_backoff_until": {}
    });
    fs::write(
        runtime_backoffs_last_good_file_path(&paths),
        serde_json::to_string_pretty(&legacy_backup)
            .expect("legacy backup should serialize cleanly"),
    )
    .expect("legacy backup should be writable");
    fs::write(&runtime_backoffs_file_path(&paths), "{ not valid json")
        .expect("broken primary backoffs should be writable");

    let loaded = load_runtime_profile_backoffs_with_recovery(&paths, &profiles)
        .expect("legacy backup recovery should succeed");

    assert!(loaded.recovered_from_backup);
    assert_eq!(loaded.value.retry_backoff_until.get("main"), Some(&(now + 600)));
    assert!(loaded.value.transport_backoff_until.is_empty());
    assert!(loaded.value.route_circuit_open_until.is_empty());
}

#[test]
fn runtime_state_snapshot_save_returns_error_on_injected_failure() {
    let temp_dir = TestDir::new();
    let _guard = TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE", "1");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let snapshot = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let latest_revision = AtomicU64::new(1);

    let err = save_runtime_state_snapshot_if_latest(
        &paths,
        &snapshot,
        &runtime_continuation_store_from_app_state(&snapshot),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &RuntimeProfileBackoffs::default(),
        1,
        &latest_revision,
    )
    .expect_err("injected save failure should bubble up");
    assert!(
        err.to_string()
            .contains("injected runtime state save failure")
    );
}

#[test]
fn app_state_load_uses_last_good_backup_when_primary_is_invalid() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-1".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let backup_json =
        serde_json::to_string_pretty(&state).expect("state backup should serialize cleanly");
    fs::write(state_last_good_file_path(&paths), backup_json)
        .expect("last-good backup should be writable");
    fs::write(&paths.state_file, "{ not valid json").expect("broken primary state should write");

    let loaded = AppState::load_with_recovery(&paths).expect("backup recovery should succeed");

    assert!(loaded.recovered_from_backup);
    assert_eq!(loaded.value.active_profile.as_deref(), Some("main"));
    assert_eq!(
        loaded
            .value
            .response_profile_bindings
            .get("resp-1")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
}

#[test]
fn runtime_continuations_load_legacy_last_good_backup_when_primary_is_invalid() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let backup_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-legacy".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::write(
        runtime_continuations_last_good_file_path(&paths),
        serde_json::to_string_pretty(&backup_store)
            .expect("legacy continuation backup should serialize"),
    )
    .expect("legacy continuation backup should write");
    fs::write(runtime_continuations_file_path(&paths), "{ not valid json")
        .expect("broken continuation primary should write");

    let loaded = load_runtime_continuations_with_recovery(&paths, &profiles)
        .expect("legacy continuation backup recovery should succeed");

    assert!(loaded.recovered_from_backup);
    assert_eq!(
        loaded
            .value
            .response_profile_bindings
            .get("resp-legacy")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
}

#[test]
fn runtime_continuations_reject_stale_generation_overwrite() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let initial_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-old".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp() - 10,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &initial_store, &profiles)
        .expect("initial continuation save should succeed");

    let wrapped_primary: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("initial continuation primary should be readable"),
    )
    .expect("initial continuation primary should be valid json");
    assert_eq!(wrapped_primary["generation"].as_u64(), Some(1));

    let newer_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-new".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    write_versioned_runtime_sidecar(
        &runtime_continuations_file_path(&paths),
        &runtime_continuations_last_good_file_path(&paths),
        2,
        &newer_store,
    );

    let stale_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp() - 20,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    let err = save_runtime_continuations_for_profiles(&paths, &stale_store, &profiles)
        .expect_err("stale continuation save should be fenced");
    assert!(
        err.to_string().contains("stale runtime sidecar generation"),
        "unexpected stale-save error: {err:#}"
    );

    let wrapped_after: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("continuation primary should still be readable"),
    )
    .expect("continuation primary should still be valid json");
    assert_eq!(wrapped_after["generation"].as_u64(), Some(2));
    assert_eq!(
        wrapped_after["value"]["response_profile_bindings"]["resp-new"]["profile_name"],
        serde_json::Value::String("main".to_string())
    );
    assert!(
        !wrapped_after["value"]["response_profile_bindings"]
            .as_object()
            .expect("response_profile_bindings should be an object")
            .contains_key("resp-stale"),
        "stale writer must not overwrite newer continuation state"
    );
}

#[test]
fn runtime_state_snapshot_save_retries_stale_continuation_generation() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    initial_state
        .save(&paths)
        .expect("initial state save should succeed");

    let initial_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-initial".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &initial_store, &profiles)
        .expect("initial continuation save should succeed");

    let external_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-external".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    write_versioned_runtime_sidecar(
        &runtime_continuations_file_path(&paths),
        &runtime_continuations_last_good_file_path(&paths),
        2,
        &external_store,
    );

    let snapshot = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-local".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_snapshot_if_latest(
            &paths,
            &snapshot,
            &runtime_continuation_store_from_app_state(&snapshot),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &RuntimeProfileBackoffs::default(),
            1,
            &revision,
        )
        .expect("state snapshot save should succeed after stale retry")
    );

    let loaded = load_runtime_continuations_with_recovery(&paths, &profiles)
        .expect("continuations should reload")
        .value;
    assert_eq!(
        loaded
            .response_profile_bindings
            .get("resp-local")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    let wrapped: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("continuation primary should be readable"),
    )
    .expect("continuation primary should remain valid json");
    assert_eq!(wrapped["generation"].as_u64(), Some(3));
}

#[test]
fn runtime_state_snapshot_retry_does_not_resurrect_released_response_binding() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    initial_state
        .save(&paths)
        .expect("initial state save should succeed");

    let initial_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &initial_store, &profiles)
        .expect("initial continuation save should succeed");

    let released_store = RuntimeContinuationStore {
        statuses: RuntimeContinuationStatuses {
            response: BTreeMap::from([("resp-stale".to_string(), dead_continuation_status(now))]),
            ..RuntimeContinuationStatuses::default()
        },
        ..RuntimeContinuationStore::default()
    };
    write_versioned_runtime_sidecar(
        &runtime_continuations_file_path(&paths),
        &runtime_continuations_last_good_file_path(&paths),
        2,
        &released_store,
    );

    let snapshot = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_snapshot_if_latest(
            &paths,
            &snapshot,
            &runtime_continuation_store_from_app_state(&snapshot),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &RuntimeProfileBackoffs::default(),
            1,
            &revision,
        )
        .expect("state snapshot save should succeed after stale retry")
    );

    let loaded = load_runtime_continuations_with_recovery(&paths, &profiles)
        .expect("continuations should reload")
        .value;
    assert!(
        !loaded.response_profile_bindings.contains_key("resp-stale"),
        "released response binding must not be resurrected"
    );
    assert_eq!(
        loaded
            .statuses
            .response
            .get("resp-stale")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );

    let state = AppState::load(&paths).expect("state should reload");
    assert!(
        !state.response_profile_bindings.contains_key("resp-stale"),
        "state snapshot must not rewrite the released response binding"
    );
}

#[test]
fn runtime_continuation_journal_save_retries_stale_generation() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let initial = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-initial".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &initial, now - 30)
        .expect("initial journal save should succeed");

    let external = RuntimeContinuationJournal {
        saved_at: now - 10,
        continuations: RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-external".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now - 10,
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
    };
    write_versioned_runtime_sidecar(
        &runtime_continuation_journal_file_path(&paths),
        &runtime_continuation_journal_last_good_file_path(&paths),
        2,
        &external,
    );

    let incoming = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-local".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &incoming, now)
        .expect("journal save should retry stale generation");

    let loaded = load_runtime_continuation_journal_with_recovery(&paths, &state.profiles)
        .expect("journal should reload")
        .value;
    assert_eq!(loaded.saved_at, now);
    assert_eq!(
        loaded
            .continuations
            .response_profile_bindings
            .get("resp-local")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    let wrapped: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuation_journal_file_path(&paths))
            .expect("journal primary should be readable"),
    )
    .expect("journal primary should remain valid json");
    assert_eq!(wrapped["generation"].as_u64(), Some(3));
}

#[test]
fn runtime_continuation_journal_retry_does_not_resurrect_released_response_binding() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let initial = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &initial, now - 30)
        .expect("initial journal save should succeed");

    let external = RuntimeContinuationJournal {
        saved_at: now - 5,
        continuations: RuntimeContinuationStore {
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-stale".to_string(),
                    dead_continuation_status(now),
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
    };
    write_versioned_runtime_sidecar(
        &runtime_continuation_journal_file_path(&paths),
        &runtime_continuation_journal_last_good_file_path(&paths),
        2,
        &external,
    );

    let incoming = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &incoming, now)
        .expect("journal save should retry stale generation");

    let loaded = load_runtime_continuation_journal_with_recovery(&paths, &state.profiles)
        .expect("journal should reload")
        .value;
    assert_eq!(loaded.saved_at, now);
    assert!(
        !loaded
            .continuations
            .response_profile_bindings
            .contains_key("resp-stale"),
        "released response binding must not be resurrected in the journal"
    );
    assert_eq!(
        loaded
            .continuations
            .statuses
            .response
            .get("resp-stale")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn merge_runtime_continuation_store_keeps_compact_session_release_tombstone() {
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: PathBuf::from("/tmp/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let now = Local::now().timestamp();
    let key = runtime_compact_session_lineage_key("sess-compact");

    let existing = RuntimeContinuationStore {
        session_id_bindings: BTreeMap::from([(
            key.clone(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        statuses: RuntimeContinuationStatuses {
            session_id: BTreeMap::from([(
                key.clone(),
                RuntimeContinuationBindingStatus {
                    state: RuntimeContinuationBindingLifecycle::Verified,
                    confidence: 2,
                    last_touched_at: Some(now - 30),
                    last_verified_at: Some(now - 30),
                    last_verified_route: Some("compact".to_string()),
                    last_not_found_at: None,
                    not_found_streak: 0,
                    success_count: 1,
                    failure_count: 0,
                },
            )]),
            ..RuntimeContinuationStatuses::default()
        },
        ..RuntimeContinuationStore::default()
    };
    let incoming = RuntimeContinuationStore {
        statuses: RuntimeContinuationStatuses {
            session_id: BTreeMap::from([(
                key.clone(),
                RuntimeContinuationBindingStatus {
                    last_verified_route: Some("compact".to_string()),
                    ..dead_continuation_status(now)
                },
            )]),
            ..RuntimeContinuationStatuses::default()
        },
        ..RuntimeContinuationStore::default()
    };

    let merged = merge_runtime_continuation_store(&existing, &incoming, &profiles);
    assert!(
        !merged.session_id_bindings.contains_key(&key),
        "compact session binding should be removed when a newer release tombstone exists"
    );
    assert_eq!(
        merged
            .statuses
            .session_id
            .get(&key)
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_proxy_pressure_mode_shrinks_precommit_budget() {
    let started_at = Instant::now()
        .checked_sub(Duration::from_millis(
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS + 5,
        ))
        .expect("checked_sub should succeed");

    assert!(runtime_proxy_precommit_budget_exhausted(
        started_at, 0, false, true
    ));
    assert!(!runtime_proxy_precommit_budget_exhausted(
        started_at, 0, false, false
    ));
}

#[test]
fn turn_state_affinity_prefers_bound_profile() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::from([(
            "turn-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let turn_state_profile = runtime_turn_state_bound_profile(&shared, "turn-second")
        .expect("turn-state lookup should succeed");

    assert_eq!(
        select_runtime_response_candidate(
            &shared,
            &BTreeSet::new(),
            None,
            turn_state_profile.as_deref(),
            None,
            false,
        )
        .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn turn_state_affinity_ignores_inflight_and_health_penalties() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::from([(
            "turn-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::from([(
            "second".to_string(),
            RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT + 1,
        )]),
        profile_health: BTreeMap::from([(
            "second".to_string(),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_HEALTH_MAX_SCORE,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let turn_state_profile = runtime_turn_state_bound_profile(&shared, "turn-second")
        .expect("turn-state lookup should succeed");

    assert_eq!(
        select_runtime_response_candidate(
            &shared,
            &BTreeSet::new(),
            None,
            turn_state_profile.as_deref(),
            None,
            false,
        )
        .expect("candidate selection should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn response_affinity_touch_persists_recent_use_for_housekeeping() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let stale_touch = now - RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS - 2;
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: stale_touch,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    state
        .save(&paths)
        .expect("initial state save should succeed");

    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state: state.clone(),
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
        })),
    };

    let owner = runtime_response_bound_profile(&shared, "resp-main", RuntimeRouteKind::Responses)
        .expect("response binding lookup should succeed");
    assert_eq!(owner.as_deref(), Some("main"));

    let persisted = wait_for_state(&paths, |state| {
        state
            .response_profile_bindings
            .get("resp-main")
            .is_some_and(|binding| binding.bound_at > stale_touch)
    });
    assert!(
        persisted
            .response_profile_bindings
            .get("resp-main")
            .is_some_and(|binding| binding.bound_at > stale_touch)
    );
}

#[test]
fn response_affinity_skips_recent_negative_cache_for_same_route() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state,
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
            profile_health: BTreeMap::from([(
                runtime_previous_response_negative_cache_key(
                    "resp-main",
                    "main",
                    RuntimeRouteKind::Responses,
                ),
                RuntimeProfileHealth {
                    score: 1,
                    updated_at: now,
                },
            )]),
        })),
    };

    let owner = runtime_response_bound_profile(&shared, "resp-main", RuntimeRouteKind::Responses)
        .expect("response binding lookup should succeed");
    assert_eq!(owner, None);
}

#[test]
fn response_affinity_skips_dead_continuation_status() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Dead,
                        confidence: 0,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now - 30),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: Some(now - 5),
                        not_found_streak: 2,
                        success_count: 1,
                        failure_count: 2,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };

    let owner = runtime_response_bound_profile(&shared, "resp-main", RuntimeRouteKind::Responses)
        .expect("response binding lookup should succeed");
    assert_eq!(owner, None);
}

#[test]
fn response_affinity_skips_stale_verified_continuation_status() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let stale_at = now - RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS - 1;
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: stale_at,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 3,
                        last_touched_at: Some(stale_at),
                        last_verified_at: Some(stale_at),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };

    let owner = runtime_response_bound_profile(&shared, "resp-main", RuntimeRouteKind::Responses)
        .expect("response binding lookup should succeed");
    assert_eq!(owner, None);
    assert_eq!(
        shared
            .runtime
            .lock()
            .expect("runtime lock should succeed")
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Warm)
    );
}

#[test]
fn session_affinity_skips_stale_verified_continuation_status() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let stale_at = now - RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS - 1;
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: stale_at,
            },
        )]),
    };

    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state,
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::from([(
                "sess-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_at,
                },
            )]),
            continuation_statuses: RuntimeContinuationStatuses {
                session_id: BTreeMap::from([(
                    "sess-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 3,
                        last_touched_at: Some(stale_at),
                        last_verified_at: Some(stale_at),
                        last_verified_route: Some("compact".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };

    let owner =
        runtime_session_bound_profile(&shared, "sess-main").expect("session lookup should succeed");
    assert_eq!(owner, None);
    assert_eq!(
        shared
            .runtime
            .lock()
            .expect("runtime lock should succeed")
            .continuation_statuses
            .session_id
            .get("sess-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Warm)
    );
}

#[test]
fn previous_response_affinity_release_requires_repeated_not_found() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state,
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
        })),
    };

    assert!(
        !release_runtime_previous_response_affinity(
            &shared,
            "main",
            Some("resp-main"),
            None,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("first not-found should defer hard release")
    );
    assert!(
        shared
            .runtime
            .lock()
            .expect("runtime lock")
            .state
            .response_profile_bindings
            .contains_key("resp-main")
    );

    assert!(
        release_runtime_previous_response_affinity(
            &shared,
            "main",
            Some("resp-main"),
            None,
            None,
            RuntimeRouteKind::Responses,
        )
        .expect("second not-found should release affinity")
    );
    assert!(
        !shared
            .runtime
            .lock()
            .expect("runtime lock")
            .state
            .response_profile_bindings
            .contains_key("resp-main")
    );
    assert_eq!(
        shared
            .runtime
            .lock()
            .expect("runtime lock")
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_continuation_status_pruning_uses_evidence_over_age() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let now = Local::now().timestamp();
    let stale_bound_at = now - 365 * 24 * 60 * 60;
    let mut statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
        Some(RuntimeRouteKind::Responses),
    ));
    assert_eq!(
        statuses
            .response
            .get("resp-main")
            .and_then(|status| status.last_verified_route.as_deref()),
        Some("responses")
    );
    assert!(runtime_mark_continuation_status_suspect(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now + 1,
    ));

    let retained = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_bound_at,
                },
            )]),
            statuses: statuses.clone(),
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );
    assert!(retained.response_profile_bindings.contains_key("resp-main"));
    assert_eq!(
        retained
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Suspect)
    );

    assert!(runtime_mark_continuation_status_suspect(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now + 2,
    ));

    let pruned = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_bound_at,
                },
            )]),
            statuses,
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );
    assert!(!pruned.response_profile_bindings.contains_key("resp-main"));
    assert_eq!(
        pruned
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_dead_continuation_tombstone_blocks_stale_binding_resurrection() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let now = Local::now().timestamp();
    let mut tombstone_statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut tombstone_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now - 2,
        Some(RuntimeRouteKind::Responses),
    ));
    assert!(runtime_mark_continuation_status_dead(
        &mut tombstone_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
    ));

    let merged = merge_runtime_continuation_store(
        &RuntimeContinuationStore {
            statuses: tombstone_statuses,
            ..RuntimeContinuationStore::default()
        },
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now - 1,
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert!(
        !merged.response_profile_bindings.contains_key("resp-main"),
        "dead tombstone should prune stale resurrected binding"
    );
    assert_eq!(
        merged
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_dead_continuation_tombstone_overrides_same_second_verified_status() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let now = Local::now().timestamp();
    let mut existing_statuses = RuntimeContinuationStatuses::default();
    let mut incoming_statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut existing_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
        Some(RuntimeRouteKind::Responses),
    ));
    assert!(runtime_mark_continuation_status_dead(
        &mut incoming_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
    ));

    let merged = merge_runtime_continuation_store(
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            statuses: existing_statuses,
            ..RuntimeContinuationStore::default()
        },
        &RuntimeContinuationStore {
            statuses: incoming_statuses,
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert!(
        !merged.response_profile_bindings.contains_key("resp-main"),
        "same-second dead tombstone should still prune the released binding"
    );
    assert_eq!(
        merged
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_newer_binding_overrides_older_dead_tombstone() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let now = Local::now().timestamp();
    let mut statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now - 5,
        Some(RuntimeRouteKind::Responses),
    ));
    assert!(runtime_mark_continuation_status_dead(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now - 3,
    ));

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            statuses,
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert_eq!(
        compacted
            .response_profile_bindings
            .get("resp-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert!(
        !compacted.statuses.response.contains_key("resp-main"),
        "older dead tombstone should not suppress a newer binding"
    );
}

#[test]
fn runtime_continuation_store_compaction_prunes_response_bindings_to_limit() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let now = Local::now().timestamp();
    let mut response_profile_bindings = BTreeMap::new();
    for index in 0..(RESPONSE_PROFILE_BINDING_LIMIT + 1) {
        response_profile_bindings.insert(
            format!("resp-{index:06}"),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - (RESPONSE_PROFILE_BINDING_LIMIT + 1) as i64 + index as i64,
            },
        );
    }
    let hot_response_id = "resp-000000".to_string();
    let displaced_response_id = "resp-000001".to_string();

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings,
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    hot_response_id.clone(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 3,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert_eq!(
        compacted.response_profile_bindings.len(),
        RESPONSE_PROFILE_BINDING_LIMIT
    );
    assert!(
        compacted
            .response_profile_bindings
            .contains_key(&hot_response_id),
        "verified hot response binding should survive compaction even when it is oldest"
    );
    assert!(
        compacted
            .response_profile_bindings
            .contains_key(&format!("resp-{0:06}", RESPONSE_PROFILE_BINDING_LIMIT)),
        "newest cold response binding should still be retained"
    );
    assert!(
        !compacted
            .response_profile_bindings
            .contains_key(&displaced_response_id),
        "colder bindings should be pruned ahead of the oldest verified binding"
    );
}

#[test]
fn runtime_continuation_store_compaction_keeps_verified_hot_binding_over_newer_cold_binding() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
        },
    )]);
    let now = Local::now().timestamp();

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([
                (
                    "resp-hot".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now - 3,
                    },
                ),
                (
                    "resp-cold-1".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now - 2,
                    },
                ),
                (
                    "resp-cold-2".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now - 1,
                    },
                ),
            ]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-hot".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
                        success_count: 3,
                        failure_count: 0,
                        not_found_streak: 0,
                        last_touched_at: Some(now),
                        last_not_found_at: None,
                        last_verified_at: Some(now),
                        last_verified_route: Some("responses".to_string()),
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    let mut retained = compacted
        .response_profile_bindings
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    retained.sort();

    assert_eq!(retained.len(), 3);
    assert!(retained.contains(&"resp-hot".to_string()));

    let mut pruned = RuntimeContinuationStore {
        response_profile_bindings: compacted.response_profile_bindings,
        statuses: compacted.statuses,
        ..RuntimeContinuationStore::default()
    };
    prune_runtime_continuation_response_bindings(
        &mut pruned.response_profile_bindings,
        &pruned.statuses.response,
        2,
    );
    assert!(pruned.response_profile_bindings.contains_key("resp-hot"));
    assert!(
        !pruned.response_profile_bindings.contains_key("resp-cold-1"),
        "the cold binding should be pruned before the older verified binding"
    );
}

#[test]
fn session_affinity_prefers_bound_profile_for_compact_requests() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: backend.base_url(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::from([(
            "sess-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-second".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let _response = proxy_runtime_standard_request(1, &request, &shared)
        .expect("session-bound compact request should succeed");

    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_pressure_mode_sheds_fresh_compact_requests_before_upstream() {
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: backend.base_url(),
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
    };
    let pressure_until = Local::now().timestamp().saturating_add(60).max(0) as u64;
    let shared = RuntimeRotationProxyShared {
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
        local_overload_backoff_until: Arc::new(AtomicU64::new(pressure_until)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    let response = proxy_runtime_standard_request(1, &request, &shared)
        .expect("fresh compact request should receive a local response");
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(status, 503);
    assert!(
        body.contains("Fresh compact requests are temporarily deferred"),
        "unexpected compact pressure response body: {body}"
    );
    assert!(
        backend.responses_accounts().is_empty(),
        "fresh compact request should be shed before reaching upstream"
    );
}

#[test]
fn runtime_sse_tap_reader_keeps_response_affinity_when_prelude_splits_event() {
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths: paths.clone(),
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "second".to_string(),
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
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    let prelude =
            b"event: response.created\r\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second";
    let remainder =
            b"\"}}\r\n\r\nevent: response.completed\r\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-second\"}}\r\n\r\n";
    let mut reader = RuntimeSseTapReader::new(
        Cursor::new(prelude.to_vec()).chain(Cursor::new(remainder.to_vec())),
        shared.clone(),
        "second".to_string(),
        prelude,
        &[],
    );
    let mut body = Vec::new();
    reader
        .read_to_end(&mut body)
        .expect("split SSE payload should be readable");

    let persisted = wait_for_state(&paths, |state| {
        state
            .response_profile_bindings
            .get("resp-second")
            .is_none_or(|binding| binding.profile_name != "main")
    });
    assert!(
        persisted
            .response_profile_bindings
            .get("resp-second")
            .is_none_or(|binding| binding.profile_name != "main"),
        "stale previous_response_id should not stay pinned to the wrong owner"
    );
}

#[test]
fn section_headers_use_cli_width() {
    assert_eq!(
        text_width(&section_header_with_width("Quota Overview", CLI_WIDTH)),
        CLI_WIDTH
    );
}

#[test]
fn field_lines_do_not_exceed_cli_width() {
    let label_width = panel_label_width(
            &[(
                "Path".to_string(),
                "/tmp/some/really/long/path/that/should/still/stay/inside/the/configured/cli/width/when/rendered"
                    .to_string(),
            )],
            CLI_WIDTH,
        );
    let fields = format_field_lines_with_layout(
        "Path",
        "/tmp/some/really/long/path/that/should/still/stay/inside/the/configured/cli/width/when/rendered",
        CLI_WIDTH,
        label_width,
    );

    assert!(fields.iter().all(|line| text_width(line) <= CLI_WIDTH));
}

#[test]
fn section_headers_expand_to_requested_width() {
    assert_eq!(text_width(&section_header_with_width("Doctor", 72)), 72);
}

#[test]
fn field_lines_respect_requested_width() {
    let width = 72;
    let fields = vec![(
        "Profiles root".to_string(),
        "/tmp/some/really/long/path/that/needs/to/wrap/narrower".to_string(),
    )];
    let label_width = panel_label_width(&fields, width);
    let lines = format_field_lines_with_layout(
        "Profiles root",
        "/tmp/some/really/long/path/that/needs/to/wrap/narrower",
        width,
        label_width,
    );

    assert!(lines.iter().all(|line| text_width(line) <= width));
}

#[test]
fn runtime_proxy_injects_codex_backend_overrides() {
    let args = runtime_proxy_codex_args(
        "127.0.0.1:4455".parse().expect("socket addr"),
        &[OsString::from("exec"), OsString::from("hello")],
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert_eq!(rendered[0], "-c");
    assert!(rendered[1].contains("chatgpt_base_url=\"http://127.0.0.1:4455/backend-api\""));
    assert_eq!(rendered[2], "-c");
    assert_eq!(
        rendered[3],
        format!(
            "openai_base_url=\"http://127.0.0.1:4455{}\"",
            RUNTIME_PROXY_OPENAI_MOUNT_PATH
        )
    );
    assert_eq!(&rendered[4..], ["exec", "hello"]);
}

#[test]
fn runtime_proxy_maps_openai_prefix_to_upstream_backend_api() {
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            &format!("{}/responses", RUNTIME_PROXY_OPENAI_MOUNT_PATH)
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
}

#[test]
fn runtime_proxy_injects_custom_openai_mount_path_overrides() {
    let args = runtime_proxy_codex_args_with_mount_path(
        "127.0.0.1:4455".parse().expect("socket addr"),
        "/backend-api/prodex/v0.2.99",
        &[OsString::from("exec")],
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        rendered[3],
        "openai_base_url=\"http://127.0.0.1:4455/backend-api/prodex/v0.2.99\""
    );
}

#[test]
fn runtime_proxy_maps_legacy_versioned_openai_prefix_to_upstream_backend_api() {
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/prodex/v0.2.99/responses"
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
}

#[test]
fn runtime_proxy_accepts_legacy_openai_prefix() {
    assert!(is_runtime_responses_path("/backend-api/codex/responses"));
    assert!(is_runtime_compact_path(
        "/backend-api/codex/responses/compact"
    ));
    assert!(is_runtime_responses_path(
        "/backend-api/prodex/v0.2.99/responses"
    ));
    assert!(is_runtime_compact_path(
        "/backend-api/prodex/v0.2.99/responses/compact"
    ));
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/codex/responses"
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
}

#[test]
fn runtime_proxy_broker_key_uses_stable_mount_path() {
    let current_key = runtime_broker_key("https://chatgpt.com/backend-api", false);

    let stable_key = {
        let mut hasher = DefaultHasher::new();
        "https://chatgpt.com/backend-api".hash(&mut hasher);
        false.hash(&mut hasher);
        "/backend-api/prodex".hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };

    let versioned_key = {
        let mut hasher = DefaultHasher::new();
        "https://chatgpt.com/backend-api".hash(&mut hasher);
        false.hash(&mut hasher);
        "/backend-api/prodex/v0.2.99".hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };

    assert_eq!(current_key, stable_key);
    assert_ne!(current_key, versioned_key);
}

#[test]
fn runtime_doctor_summary_counts_recent_runtime_markers() {
    let summary = summarize_runtime_log_tail(
            br#"[2026-03-20 12:00:00.000 +07:00] request=1 transport=http first_upstream_chunk bytes=128
[2026-03-20 12:00:00.010 +07:00] request=1 transport=http first_local_chunk profile=main bytes=128 elapsed_ms=10
[2026-03-20 12:00:00.015 +07:00] runtime_proxy_admission_wait_started transport=http path=/backend-api/codex/responses budget_ms=120 poll_ms=10 reason=responses
[2026-03-20 12:00:00.020 +07:00] profile_transport_backoff profile=main route=responses until=123 seconds=15 context=stream_read_error
[2026-03-20 12:00:00.030 +07:00] profile_health profile=main score=4 delta=4 reason=stream_read_error
[2026-03-20 12:00:00.035 +07:00] selection_skip_affinity route=responses affinity=session profile=main reason=quota_exhausted quota_source=persisted_snapshot
[2026-03-20 12:00:00.040 +07:00] runtime_proxy_active_limit_reached transport=http path=/backend-api/codex/responses active=12 limit=12
[2026-03-20 12:00:00.050 +07:00] runtime_proxy_lane_limit_reached transport=http path=/backend-api/codex/responses lane=responses active=9 limit=9
[2026-03-20 12:00:00.055 +07:00] runtime_proxy_admission_recovered transport=http path=/backend-api/codex/responses waited_ms=20
[2026-03-20 12:00:00.060 +07:00] profile_inflight_saturated profile=main hard_limit=8
[2026-03-20 12:00:00.070 +07:00] runtime_proxy_queue_overloaded transport=http path=/backend-api/codex/responses reason=long_lived_queue_full
[2026-03-20 12:00:00.072 +07:00] runtime_proxy_queue_wait_started transport=http path=/backend-api/codex/responses budget_ms=120 poll_ms=10 reason=long_lived_queue_full
[2026-03-20 12:00:00.074 +07:00] runtime_proxy_queue_recovered transport=http path=/backend-api/codex/responses waited_ms=18
[2026-03-20 12:00:00.075 +07:00] state_save_queued revision=2 reason=session_id:main backlog=3 ready_in_ms=5
[2026-03-20 12:00:00.078 +07:00] continuation_journal_save_queued reason=session_id:main backlog=2
[2026-03-20 12:00:00.080 +07:00] profile_probe_refresh_queued profile=second reason=queued backlog=4
[2026-03-20 12:00:00.085 +07:00] state_save_skipped revision=2 reason=session_id:main lag_ms=7
[2026-03-20 12:00:00.086 +07:00] continuation_journal_save_ok saved_at=123 reason=session_id:main lag_ms=11
[2026-03-20 12:00:00.090 +07:00] profile_probe_refresh_start profile=second
[2026-03-20 12:00:00.095 +07:00] profile_probe_refresh_ok profile=second lag_ms=13
[2026-03-20 12:00:00.100 +07:00] profile_probe_refresh_error profile=third lag_ms=8 error=timeout
[2026-03-20 12:00:00.094 +07:00] profile_circuit_open profile=main route=responses until=123 reason=stream_read_error score=4
[2026-03-20 12:00:00.095 +07:00] profile_circuit_half_open_probe profile=main route=responses until=128 health=3
[2026-03-20 12:00:00.096 +07:00] websocket_reuse_watchdog profile=main event=read_error elapsed_ms=33 committed=true
[2026-03-20 12:00:00.097 +07:00] request=2 transport=http route=responses previous_response_not_found profile=second response_id=resp-second retry_index=0
[2026-03-20 12:00:00.098 +07:00] request=3 transport=websocket route=websocket previous_response_not_found profile=second response_id=resp-second retry_index=0
[2026-03-20 12:00:00.099 +07:00] local_writer_error request=1 transport=http profile=main stage=chunk_flush chunks=1 bytes=128 elapsed_ms=20 error=broken_pipe
[2026-03-20 12:00:00.105 +07:00] runtime_proxy_startup_audit missing_managed_dirs=1 stale_response_bindings=2 stale_session_bindings=1 active_profile_missing_dir=false
"#,
        );

    assert!((27..=28).contains(&summary.line_count));
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_queue_overloaded"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_active_limit_reached"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_lane_limit_reached"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_admission_wait_started"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_admission_recovered"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_inflight_saturated"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_transport_backoff"),
        1
    );
    assert_eq!(runtime_doctor_marker_count(&summary, "profile_health"), 1);
    assert_eq!(
        runtime_doctor_marker_count(&summary, "selection_skip_affinity"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_queue_wait_started"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_queue_recovered"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "first_upstream_chunk"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "first_local_chunk"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_start"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_ok"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "state_save_queued"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "continuation_journal_save_queued"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "continuation_journal_save_ok"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "state_save_skipped"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_error"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_circuit_open"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_circuit_half_open_probe"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "websocket_reuse_watchdog"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "previous_response_not_found"),
        2
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "local_writer_error"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_startup_audit"),
        1
    );
    assert_eq!(
        runtime_doctor_top_facet(&summary, "quota_source").as_deref(),
        Some("persisted_snapshot (1)")
    );
    assert_eq!(summary.state_save_queue_backlog, Some(3));
    assert_eq!(summary.state_save_lag_ms, Some(7));
    assert_eq!(summary.continuation_journal_save_backlog, Some(2));
    assert_eq!(summary.continuation_journal_save_lag_ms, Some(11));
    assert_eq!(summary.profile_probe_refresh_backlog, Some(4));
    assert_eq!(summary.profile_probe_refresh_lag_ms, Some(13));
    assert_eq!(
        summary.failure_class_counts,
        BTreeMap::from([
            ("admission".to_string(), 6),
            ("continuation".to_string(), 2),
            ("persistence".to_string(), 1),
            ("quota".to_string(), 5),
            ("transport".to_string(), 1),
        ])
    );
    assert_eq!(
        summary.previous_response_not_found_by_route,
        BTreeMap::from([("responses".to_string(), 1), ("websocket".to_string(), 1)])
    );
    assert_eq!(
        summary.previous_response_not_found_by_transport,
        BTreeMap::from([("http".to_string(), 1), ("websocket".to_string(), 1)])
    );
    assert_eq!(
        summary
            .previous_response_not_found_by_route
            .get("websocket"),
        Some(&1)
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("state_save_queued")
            .and_then(|fields| fields.get("backlog"))
            .map(String::as_str),
        Some("3")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("continuation_journal_save_ok")
            .and_then(|fields| fields.get("lag_ms"))
            .map(String::as_str),
        Some("11")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("profile_probe_refresh_error")
            .and_then(|fields| fields.get("lag_ms"))
            .map(String::as_str),
        Some("8")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("selection_skip_affinity")
            .and_then(|fields| fields.get("affinity"))
            .map(String::as_str),
        Some("session")
    );
    assert!(
        summary
            .last_marker_line
            .as_deref()
            .is_some_and(|line| line.contains("runtime_proxy_startup_audit"))
    );
}

#[test]
fn attempt_runtime_responses_request_skips_exhausted_profile_before_send() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths: paths.clone(),
        state,
        upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: Local::now().timestamp(),
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 81,
                five_hour_reset_at: Local::now().timestamp() + 3600,
                weekly_status: RuntimeQuotaWindowStatus::Exhausted,
                weekly_remaining_percent: 0,
                weekly_reset_at: Local::now().timestamp() + 300,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: Vec::new(),
        body: br#"{"input":[]}"#.to_vec(),
    };

    match attempt_runtime_responses_request(1, &request, &shared, "main", None)
        .expect("responses attempt should succeed")
    {
        RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name,
            reason,
        } => {
            assert_eq!(profile_name, "main");
            assert_eq!(reason, "quota_exhausted_before_send");
        }
        _ => panic!("expected exhausted pre-send responses skip"),
    }
}

#[test]
fn attempt_runtime_standard_request_skips_exhausted_profile_before_send() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let runtime = RuntimeRotationState {
        paths: paths.clone(),
        state,
        upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: Local::now().timestamp(),
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: Local::now().timestamp() + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 90,
                weekly_reset_at: Local::now().timestamp() + 86_400,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![("session_id".to_string(), "sess-123".to_string())],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    match attempt_runtime_standard_request(1, &request, &shared, "main", false)
        .expect("standard attempt should succeed")
    {
        RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
            assert_eq!(profile_name, "main");
        }
        _ => panic!("expected exhausted pre-send compact skip"),
    }
}

#[test]
fn runtime_proxy_active_request_limit_is_enforced_and_released() {
    let temp_dir = TestDir::new();
    let shared = RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
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
        })),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 1,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(1),
    };

    let first = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("first slot should be available");
    assert!(
        try_acquire_runtime_proxy_active_request_slot(
            &shared,
            "http",
            "/backend-api/codex/responses",
        )
        .is_err(),
        "second slot should be rejected once limit is reached"
    );
    drop(first);
    assert!(
        try_acquire_runtime_proxy_active_request_slot(
            &shared,
            "http",
            "/backend-api/codex/responses",
        )
        .is_ok(),
        "slot should be available again after the first guard drops"
    );
}

#[test]
fn runtime_proxy_lane_limit_is_enforced_without_blocking_other_lanes() {
    let temp_dir = TestDir::new();
    let shared = RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
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
        })),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 8,
        lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
            responses: 1,
            compact: 2,
            websocket: 2,
            standard: 2,
        }),
    };

    let responses_guard = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("responses slot should be available");
    assert!(
        matches!(
            try_acquire_runtime_proxy_active_request_slot(
                &shared,
                "http",
                "/backend-api/codex/responses",
            ),
            Err(RuntimeProxyAdmissionRejection::LaneLimit(
                RuntimeRouteKind::Responses
            ))
        ),
        "second responses slot should be rejected by lane limit"
    );
    assert!(
        try_acquire_runtime_proxy_active_request_slot(
            &shared,
            "http",
            "/backend-api/codex/responses/compact",
        )
        .is_ok(),
        "compact lane should still be allowed when only responses is saturated"
    );
    drop(responses_guard);
    assert!(
        try_acquire_runtime_proxy_active_request_slot(
            &shared,
            "http",
            "/backend-api/codex/responses",
        )
        .is_ok(),
        "responses slot should recover after the first guard drops"
    );
}

#[test]
fn runtime_proxy_active_request_wait_recovers_after_short_burst() {
    let _budget_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS", "200");
    let temp_dir = TestDir::new();
    let shared = RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
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
        })),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 1,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(1),
    };

    let first = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("first slot should be available");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(first);
    });

    let started_at = Instant::now();
    let second = acquire_runtime_proxy_active_request_slot_with_wait(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("second slot should recover after a short wait");
    assert!(
        started_at.elapsed() < Duration::from_millis(100),
        "slot wait should wake promptly after release instead of waiting for poll timeout"
    );
    drop(second);
    release.join().expect("release thread should join");
}

#[test]
fn runtime_proxy_preserves_codex_headers_on_http_responses_request() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-123")
        .header("x-openai-subagent", "compact-remote")
        .header(
            "x-codex-turn-metadata",
            r#"{"source":"resume","session_id":"sess-123"}"#,
        )
        .header("x-codex-beta-features", "remote-sync,realtime")
        .header("User-Agent", "codex-cli/0.117.0")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");

    assert!(
        response.status().is_success(),
        "unexpected status: {}",
        response.status()
    );

    let headers = backend.responses_headers();
    let first = headers
        .first()
        .expect("backend should capture request headers");
    assert_eq!(
        first.get("session_id").map(String::as_str),
        Some("sess-123")
    );
    assert_eq!(
        first.get("x-openai-subagent").map(String::as_str),
        Some("compact-remote")
    );
    assert_eq!(
        first.get("x-codex-turn-metadata").map(String::as_str),
        Some(r#"{"source":"resume","session_id":"sess-123"}"#)
    );
    assert_eq!(
        first.get("x-codex-beta-features").map(String::as_str),
        Some("remote-sync,realtime")
    );
    assert_eq!(
        first.get("user-agent").map(String::as_str),
        Some("codex-cli/0.117.0")
    );
    assert_eq!(
        first.get("chatgpt-account-id").map(String::as_str),
        Some("second-account")
    );
}

#[test]
fn runtime_proxy_preserves_codex_headers_on_websocket_responses_request() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_websocket();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let mut request = format!("ws://{}/backend-api/codex/responses", proxy.listen_addr)
        .into_client_request()
        .expect("websocket request should build");
    request.headers_mut().insert(
        "session_id",
        "sess-123".parse().expect("valid header value"),
    );
    request.headers_mut().insert(
        "x-openai-subagent",
        "compact-remote".parse().expect("valid header value"),
    );
    request.headers_mut().insert(
        "x-codex-turn-metadata",
        r#"{"source":"resume","session_id":"sess-123"}"#
            .parse()
            .expect("valid header value"),
    );
    request.headers_mut().insert(
        "x-codex-beta-features",
        "remote-sync,realtime".parse().expect("valid header value"),
    );
    request.headers_mut().insert(
        "User-Agent",
        "codex-cli/0.117.0".parse().expect("valid header value"),
    );

    let (mut socket, _response) =
        tungstenite::connect(request).expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("runtime proxy websocket request should be sent");

    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) if is_runtime_terminal_event(&text) => break,
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Text(_) | WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    let headers = backend.responses_headers();
    let first = headers
        .first()
        .expect("backend should capture websocket request headers");
    assert_eq!(
        first.get("session_id").map(String::as_str),
        Some("sess-123")
    );
    assert_eq!(
        first.get("x-openai-subagent").map(String::as_str),
        Some("compact-remote")
    );
    assert_eq!(
        first.get("x-codex-turn-metadata").map(String::as_str),
        Some(r#"{"source":"resume","session_id":"sess-123"}"#)
    );
    assert_eq!(
        first.get("x-codex-beta-features").map(String::as_str),
        Some("remote-sync,realtime")
    );
    assert_eq!(
        first.get("user-agent").map(String::as_str),
        Some("codex-cli/0.117.0")
    );
    assert_eq!(
        first.get("chatgpt-account-id").map(String::as_str),
        Some("second-account")
    );
}

#[test]
fn runtime_proxy_preserves_websocket_request_client_metadata_payload() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_websocket();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let mut request = format!("ws://{}/backend-api/codex/responses", proxy.listen_addr)
        .into_client_request()
        .expect("websocket request should build");
    request.headers_mut().insert(
        "session_id",
        "sess-main".parse().expect("valid header value"),
    );

    let (mut socket, _response) =
        tungstenite::connect(request).expect("runtime proxy websocket handshake should succeed");
    let payload = serde_json::json!({
        "input": [],
        "client_metadata": {
            "w3c_trace_context": {
                "traceparent": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            },
            "custom_marker": "keep-me"
        },
        "other_field": {
            "nested": true
        }
    })
    .to_string();
    socket
        .send(WsMessage::Text(payload.into()))
        .expect("runtime proxy websocket request should be sent");

    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) if is_runtime_terminal_event(&text) => break,
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Text(_) | WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    let request = backend
        .websocket_requests()
        .into_iter()
        .last()
        .expect("backend should capture websocket request payload");
    assert!(request.contains("\"client_metadata\""));
    assert!(request.contains("\"w3c_trace_context\""));
    assert!(request.contains("\"traceparent\""));
    assert!(request.contains("\"custom_marker\":\"keep-me\""));
    assert!(request.contains("\"other_field\":{\"nested\":true}"));
}

#[test]
fn runtime_proxy_reloads_auth_json_between_http_requests() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    let auth_path = profile_home.join("auth.json");
    write_auth_json(&auth_path, "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let first = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("first runtime proxy request should succeed");
    assert!(
        first.status().is_success(),
        "unexpected first status: {}",
        first.status()
    );

    write_auth_json(&auth_path, "third-account");

    let second = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("second runtime proxy request should succeed");
    assert!(
        second.status().is_success(),
        "unexpected second status: {}",
        second.status()
    );

    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string(), "third-account".to_string()]
    );
}

#[test]
fn runtime_proxy_retries_quota_blocked_response_on_another_profile() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    assert!(body.contains("\"response.created\""));
    assert!(!body.contains("main quota exhausted"));
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
}

#[test]
fn runtime_proxy_retries_usage_limited_response_on_another_profile() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let startup_usage_accounts = sorted_backend_usage_accounts(&backend);
    assert_eq!(
        startup_usage_accounts,
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    assert!(body.contains("\"response.created\""));
    assert!(!body.contains("You've hit your usage limit"));
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
}

#[test]
fn runtime_proxy_retries_usage_limited_response_after_output_item_added_on_another_profile() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_delayed_quota_after_output_item_added();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    assert!(body.contains("\"resp-second\""));
    assert!(!body.contains("You've hit your usage limit"));
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
}

#[test]
fn runtime_proxy_standard_request_retries_usage_limited_response_on_another_profile() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    wait_for_runtime_background_queues_idle();
    let response = Client::builder()
        .build()
        .expect("client")
        .get(format!("http://{}/backend-api/status", proxy.listen_addr))
        .send()
        .expect("runtime proxy standard request should succeed");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert!(status.is_success(), "unexpected standard status: {status}");
    assert!(body.contains("\"status\":\"ok\""));
    assert!(body.contains("\"account_id\":\"second-account\""));
    assert!(!body.contains("usage limit"));
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_standard_request_preserves_plain_429_when_not_explicit_quota() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_plain_429();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    wait_for_runtime_background_queues_idle();
    let response = Client::builder()
        .build()
        .expect("client")
        .get(format!("http://{}/backend-api/status", proxy.listen_addr))
        .send()
        .expect("runtime proxy standard request should succeed");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert_eq!(status, reqwest::StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body, "Too Many Requests");
    assert_eq!(backend.responses_accounts(), vec!["main-account".to_string()]);
}

#[test]
fn runtime_proxy_preserves_function_call_output_affinity_when_previous_response_missing() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body(
            r#"{"previous_response_id":"resp-missing","input":[{"type":"function_call_output","call_id":"call_123","output":"ok"}]}"#,
        )
        .send()
        .expect("runtime proxy request should succeed");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert_eq!(status.as_u16(), 400, "unexpected status: {status}");
    assert!(
        body.contains("\"previous_response_not_found\""),
        "function call output request should preserve previous_response failure instead of degrading to fresh: {body}"
    );
}

#[test]
fn runtime_proxy_websocket_preserves_function_call_output_affinity_when_previous_response_missing()
{
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_websocket();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text(
            r#"{"previous_response_id":"resp-missing","input":[{"type":"function_call_output","call_id":"call_123","output":"ok"}]}"#
                .to_string()
                .into(),
        ))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text)
                    || text.contains("\"previous_response_not_found\"");
                payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"previous_response_not_found\"")),
        "function call output websocket request should preserve previous_response failure instead of degrading to fresh: {payloads:?}"
    );
}

#[test]
fn runtime_proxy_releases_quota_blocked_session_affinity_and_rotates() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-123".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-123")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    assert!(body.contains("\"response.created\""));
    assert!(!body.contains("You've hit your usage limit"));
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
            && state
                .session_profile_bindings
                .get("sess-123")
                .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
    assert_eq!(
        persisted
            .session_profile_bindings
            .get("sess-123")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
}

#[test]
fn runtime_proxy_releases_quota_blocked_previous_response_affinity_and_degrades_to_fresh_http() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    let now = Local::now().timestamp();

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 1,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to save continuation sidecar");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");

    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-main\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert_eq!(status, reqwest::StatusCode::OK);
    assert!(
        body.contains("\"resp-second\""),
        "unexpected HTTP continuation fallback body: {body}"
    );
    assert!(
        !body.contains("You've hit your usage limit"),
        "quota-blocked previous_response should degrade before surfacing usage limit: {body}"
    );
    assert!(
        !body.contains("\"previous_response_not_found\""),
        "previous_response fallback should stay pre-commit: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        !state.response_profile_bindings.contains_key("resp-main")
            && state
                .response_profile_bindings
                .get("resp-second")
                .is_some_and(|binding| binding.profile_name == "second")
    });
    assert!(
        !persisted
            .response_profile_bindings
            .contains_key("resp-main")
    );
    assert_eq!(
        persisted
            .response_profile_bindings
            .get("resp-second")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
    let continuations = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .statuses
            .response
            .get("resp-main")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
    });
    assert_eq!(
        continuations
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_proxy_releases_previous_response_affinity_for_http_requests_when_owner_snapshot_hits_critical_floor(
) {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    let now = Local::now().timestamp();

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 1,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to save continuation sidecar");
    save_runtime_usage_snapshots(
        &paths,
        &BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Critical,
                five_hour_remaining_percent: 1,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 70,
                weekly_reset_at: now + 86_400,
            },
        )]),
    )
    .expect("failed to save runtime usage snapshots");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");

    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-main\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert_eq!(status, reqwest::StatusCode::OK);
    assert!(
        body.contains("\"resp-second\""),
        "unexpected HTTP critical-floor fallback body: {body}"
    );
    assert!(
        !body.contains("You've hit your usage limit"),
        "critical-floor owner should be blocked before surfacing usage limit: {body}"
    );
    assert!(
        !body.contains("\"previous_response_not_found\""),
        "previous_response fallback should stay pre-commit: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_stale_critical_floor_snapshot_skips_current_profile_on_fresh_http_requests() {
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");
    save_runtime_usage_snapshots(
        &paths,
        &BTreeMap::from([(
            "main".to_string(),
            stale_critical_runtime_usage_snapshot(now),
        )]),
    )
    .expect("failed to seed stale runtime usage snapshots");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    wait_for_runtime_background_queues_idle();
    let before_request = backend.responses_accounts().len();

    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    let request_accounts = backend.responses_accounts()[before_request..].to_vec();
    assert!(
        !request_accounts.is_empty(),
        "stale critical-floor snapshot should still make a fresh request"
    );
    assert_eq!(
        request_accounts.last().map(String::as_str),
        Some("second-account"),
        "stale critical-floor snapshot should finish on the fallback profile: {request_accounts:?}"
    );
    assert!(
        !body.contains("You've hit your usage limit"),
        "stale critical-floor snapshot should not surface usage limit: {body}"
    );
}

#[test]
fn runtime_proxy_quarantines_usage_limited_profile_across_restart_for_fresh_http_requests() {
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");

    let client = Client::builder().build().expect("client");
    {
        let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
            .expect("runtime proxy should start");
        wait_for_runtime_background_queues_idle();
        let before_first_request = backend.responses_accounts().len();

        let response = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send()
            .expect("runtime proxy request should succeed");
        let body = response.text().expect("response body should be readable");
        let first_request_accounts = backend.responses_accounts()[before_first_request..].to_vec();
        assert_eq!(
            first_request_accounts.last().map(String::as_str),
            Some("second-account"),
            "initial quota-limited request should finish on the fallback profile: {first_request_accounts:?}"
        );
        assert!(
            !body.contains("You've hit your usage limit"),
            "quota-limited request should not surface usage limit: {body}"
        );
    }

    state.save(&paths).expect("failed to restore active profile");
    wait_for_runtime_background_queues_idle();
    let before_second_request = backend.responses_accounts().len();

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("replacement runtime proxy should start");
    wait_for_runtime_background_queues_idle();
    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("replacement runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    let second_request_accounts = backend.responses_accounts()[before_second_request..].to_vec();
    assert_eq!(
        second_request_accounts,
        vec!["second-account".to_string()],
        "quarantined profile should not be reused for a follow-up fresh request: {second_request_accounts:?}"
    );
    assert!(
        !body.contains("You've hit your usage limit"),
        "quarantined profile should not surface usage limit after restart: {body}"
    );
}

#[test]
fn runtime_proxy_releases_quota_blocked_compact_session_affinity_and_rotates() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-compact".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-compact")
        .body("{\"input\":[],\"instructions\":\"compact\"}")
        .send()
        .expect("runtime proxy compact request should succeed");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert!(status.is_success(), "unexpected compact status: {status}");
    assert_eq!(body, "{\"output\":[]}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        state
            .session_profile_bindings
            .get("sess-compact")
            .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(
        persisted
            .session_profile_bindings
            .get("sess-compact")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
}

#[test]
fn exhausted_usage_snapshot_preserves_persisted_affinity_bindings() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-1".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-123".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    };
    state.save(&paths).expect("failed to save initial state");

    let runtime = RuntimeRotationState {
        paths: paths.clone(),
        state: state.clone(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::from([(
            "turn-123".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        session_id_bindings: state.session_profile_bindings.clone(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };
    let exhausted_usage = usage_with_main_windows(81, 3600, 0, 300);

    update_runtime_profile_probe_cache_with_usage(&shared, "main", exhausted_usage)
        .expect("usage snapshot update should succeed");

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    let refreshed_probe = runtime
        .profile_probe_cache
        .get("main")
        .expect("probe cache should be refreshed");
    assert_eq!(
        runtime_profile_probe_cache_freshness(refreshed_probe, Local::now().timestamp()),
        RuntimeProbeCacheFreshness::Fresh
    );
    assert_eq!(
        runtime
            .profile_usage_snapshots
            .get("main")
            .map(|snapshot| (snapshot.five_hour_status, snapshot.weekly_status)),
        Some((
            RuntimeQuotaWindowStatus::Ready,
            RuntimeQuotaWindowStatus::Exhausted
        ))
    );
    assert!(
        runtime
            .state
            .response_profile_bindings
            .contains_key("resp-1")
    );
    assert!(
        runtime
            .state
            .session_profile_bindings
            .contains_key("sess-123")
    );
    assert_eq!(
        runtime
            .turn_state_bindings
            .get("turn-123")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert!(runtime.session_id_bindings.contains_key("sess-123"));
}

#[test]
fn runtime_doctor_detects_upstream_without_local_chunk_in_sampled_tail() {
    let tail = concat!(
        "[2026-03-25 00:00:00.000 +07:00] request=1 route=responses transport=http first_upstream_chunk profile=main\n",
        "[2026-03-25 00:00:01.000 +07:00] request=1 route=responses transport=http stream_read_error profile=main reason=connection_reset\n",
    );

    let summary = summarize_runtime_log_tail(tail.as_bytes());
    assert_eq!(
        runtime_doctor_marker_count(&summary, "first_upstream_chunk"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "first_local_chunk"),
        0
    );
    assert_eq!(
        runtime_doctor_top_facet(&summary, "route").as_deref(),
        Some("responses (2)")
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("stream_read_error")
            .and_then(|fields| fields.get("reason"))
            .map(String::as_str),
        Some("connection_reset")
    );
}

#[test]
fn runtime_doctor_collect_summary_reports_route_circuit_diagnosis() {
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    fs::create_dir_all(&runtime_log_dir).expect("runtime log dir should be created");
    let _runtime_log_dir_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_log_dir.to_str().unwrap());
    let pointer = runtime_proxy_latest_log_pointer_path();
    let log_path = runtime_log_dir.join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-doctor.log"));
    fs::write(
        &log_path,
        concat!(
            "[2026-03-26 00:00:00.000 +07:00] profile_circuit_open profile=main route=responses until=200 reason=stream_read_error score=4\n",
            "[2026-03-26 00:00:00.050 +07:00] first_upstream_chunk route=responses transport=http profile=main\n",
        ),
    )
    .expect("runtime log should be written");
    fs::write(&pointer, format!("{}\n", log_path.display())).expect("pointer should be written");

    let summary = collect_runtime_doctor_summary();
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_circuit_open"),
        1
    );
    assert_eq!(summary.transport_pressure, "elevated");
    assert!(
        summary.diagnosis.contains("circuit breaker") || summary.diagnosis.contains("writer stall")
    );
}

#[test]
fn runtime_doctor_collect_summary_falls_back_to_newer_log_when_pointer_is_stale() {
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    fs::create_dir_all(&runtime_log_dir).expect("runtime log dir should be created");
    let _runtime_log_dir_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_log_dir.to_str().unwrap());
    let pointer = runtime_proxy_latest_log_pointer_path();
    let stale_log = runtime_log_dir.join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-stale.log"));
    let fresh_log = runtime_log_dir.join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-fresh.log"));
    fs::write(
        &stale_log,
        "[2026-03-26 00:00:00.000 +07:00] profile_circuit_open profile=main route=responses until=200 reason=stream_read_error score=4\n",
    )
    .expect("stale runtime log should be written");
    fs::write(&pointer, format!("{}\n", stale_log.display())).expect("pointer should be written");
    thread::sleep(Duration::from_millis(20));
    fs::write(
        &fresh_log,
        "[2026-03-26 00:00:01.000 +07:00] stream_read_error route=responses transport=http profile=main reason=connection_reset\n",
    )
    .expect("fresh runtime log should be written");

    let summary = collect_runtime_doctor_summary();
    assert_eq!(summary.log_path.as_ref(), Some(&fresh_log));
    assert_eq!(
        runtime_doctor_marker_count(&summary, "stream_read_error"),
        1,
        "doctor should summarize the newer log instead of the stale pointer target"
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_circuit_open"),
        0,
        "doctor should not summarize the stale pointed log when a newer log exists"
    );
    assert!(
        summary.diagnosis.contains("pointer was stale"),
        "doctor diagnosis should surface pointer fallback: {}",
        summary.diagnosis
    );
}

#[test]
fn startup_audit_prunes_stale_sidecars_for_missing_managed_profile() {
    let temp_dir = TestDir::new();
    let valid_home = temp_dir.path.join("homes/valid");
    write_auth_json(&valid_home.join("auth.json"), "valid-account");
    let missing_home = temp_dir.path.join("homes/missing");
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let runtime = RuntimeRotationState {
        paths,
        state: AppState {
            active_profile: Some("valid".to_string()),
            profiles: BTreeMap::from([
                (
                    "valid".to_string(),
                    ProfileEntry {
                        codex_home: valid_home,
                        managed: true,
                        email: Some("valid@example.com".to_string()),
                    },
                ),
                (
                    "missing".to_string(),
                    ProfileEntry {
                        codex_home: missing_home,
                        managed: true,
                        email: Some("missing@example.com".to_string()),
                    },
                ),
            ]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::from([(
                "resp-missing".to_string(),
                ResponseProfileBinding {
                    profile_name: "missing".to_string(),
                    bound_at: 1,
                },
            )]),
            session_profile_bindings: BTreeMap::from([(
                "sess-missing".to_string(),
                ResponseProfileBinding {
                    profile_name: "missing".to_string(),
                    bound_at: 1,
                },
            )]),
        },
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "valid".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::from([(
            "turn-missing".to_string(),
            ResponseProfileBinding {
                profile_name: "missing".to_string(),
                bound_at: 1,
            },
        )]),
        session_id_bindings: BTreeMap::from([(
            "sess-missing".to_string(),
            ResponseProfileBinding {
                profile_name: "missing".to_string(),
                bound_at: 1,
            },
        )]),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::from([(
            "missing".to_string(),
            RuntimeProfileProbeCacheEntry {
                checked_at: Local::now().timestamp(),
                auth: AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Err("stale".to_string()),
            },
        )]),
        profile_usage_snapshots: BTreeMap::from([(
            "missing".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: Local::now().timestamp(),
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: Local::now().timestamp() + 300,
                weekly_status: RuntimeQuotaWindowStatus::Exhausted,
                weekly_remaining_percent: 0,
                weekly_reset_at: Local::now().timestamp() + 600,
            },
        )]),
        profile_retry_backoff_until: BTreeMap::from([(
            "missing".to_string(),
            Local::now().timestamp() + 60,
        )]),
        profile_transport_backoff_until: BTreeMap::from([(
            "missing".to_string(),
            Local::now().timestamp() + 60,
        )]),
        profile_route_circuit_open_until: BTreeMap::from([(
            runtime_profile_route_circuit_key("missing", RuntimeRouteKind::Responses),
            Local::now().timestamp() + 60,
        )]),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_health_key("missing", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 4,
                updated_at: Local::now().timestamp(),
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    audit_runtime_proxy_startup_state(&shared);

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key("resp-missing")
    );
    assert!(
        !runtime
            .state
            .session_profile_bindings
            .contains_key("sess-missing")
    );
    assert!(!runtime.turn_state_bindings.contains_key("turn-missing"));
    assert!(!runtime.session_id_bindings.contains_key("sess-missing"));
    assert!(!runtime.profile_probe_cache.contains_key("missing"));
    assert!(!runtime.profile_usage_snapshots.contains_key("missing"));
    assert!(!runtime.profile_retry_backoff_until.contains_key("missing"));
    assert!(
        !runtime
            .profile_transport_backoff_until
            .contains_key("missing")
    );
    assert!(!runtime.profile_route_circuit_open_until.contains_key(
        &runtime_profile_route_circuit_key("missing", RuntimeRouteKind::Responses)
    ));
    assert!(
        !runtime
            .profile_health
            .contains_key(&runtime_profile_route_health_key(
                "missing",
                RuntimeRouteKind::Responses
            ))
    );
}

#[test]
fn reserve_runtime_profile_route_circuit_half_open_probe_is_single_flight() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
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
        profile_route_circuit_open_until: BTreeMap::from([(
            runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
            now - 1,
        )]),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: 4,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    assert!(
        reserve_runtime_profile_route_circuit_half_open_probe(
            &shared,
            "main",
            RuntimeRouteKind::Responses,
        )
        .expect("first half-open reservation should succeed")
    );
    {
        let runtime = shared.runtime.lock().expect("runtime lock should succeed");
        assert!(
            runtime
                .profile_route_circuit_open_until
                .get(&runtime_profile_route_circuit_key(
                    "main",
                    RuntimeRouteKind::Responses,
                ))
                .is_some_and(|until| *until > now)
        );
    }
    assert!(
        !reserve_runtime_profile_route_circuit_half_open_probe(
            &shared,
            "main",
            RuntimeRouteKind::Responses,
        )
        .expect("second half-open reservation should be blocked")
    );
}

#[test]
fn reserve_runtime_profile_route_circuit_half_open_probe_clears_stale_reopen_stage() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
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
        profile_route_circuit_open_until: BTreeMap::from([(
            runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
            now - 1,
        )]),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([
            (
                runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
                RuntimeProfileHealth {
                    score: 1,
                    updated_at: now - RUNTIME_PROFILE_HEALTH_DECAY_SECONDS,
                },
            ),
            (
                runtime_profile_route_circuit_reopen_key("main", RuntimeRouteKind::Responses),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: now,
                },
            ),
        ]),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    assert!(
        reserve_runtime_profile_route_circuit_half_open_probe(
            &shared,
            "main",
            RuntimeRouteKind::Responses,
        )
        .expect("stale circuit should clear cleanly")
    );

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(!runtime.profile_route_circuit_open_until.contains_key(
        &runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses)
    ));
    assert!(
        !runtime
            .profile_health
            .contains_key(&runtime_profile_route_circuit_reopen_key(
                "main",
                RuntimeRouteKind::Responses
            ))
    );
}

#[test]
fn bump_runtime_profile_health_score_escalates_reopened_route_circuit() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
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
        profile_route_circuit_open_until: BTreeMap::from([(
            runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
            now + RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS,
        )]),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD,
                updated_at: now,
            },
        )]),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    bump_runtime_profile_health_score(
        &shared,
        "main",
        RuntimeRouteKind::Responses,
        1,
        "stream_read_error",
    )
    .expect("route health bump should succeed");

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    let reopen_stage = runtime
        .profile_health
        .get(&runtime_profile_route_circuit_reopen_key(
            "main",
            RuntimeRouteKind::Responses,
        ))
        .expect("reopen stage should be tracked")
        .score;
    let circuit_until = *runtime
        .profile_route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(
            "main",
            RuntimeRouteKind::Responses,
        ))
        .expect("route circuit should remain open");

    assert_eq!(reopen_stage, 1);
    assert!(
        circuit_until
            >= Local::now()
                .timestamp()
                .saturating_add(runtime_profile_circuit_open_seconds(
                    RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 1,
                    reopen_stage,
                ))
                - 1,
        "reopened circuit should back off longer than the half-open probe window"
    );
}

#[test]
fn route_circuit_backoffs_scale_with_health_score() {
    assert_eq!(
        runtime_profile_circuit_open_seconds(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD, 0),
        RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS
    );
    assert_eq!(
        runtime_profile_circuit_open_seconds(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 1, 0),
        (RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS * 2).min(RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS)
    );
    assert!(
        runtime_profile_circuit_open_seconds(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD, 2)
            > runtime_profile_circuit_open_seconds(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD, 0)
    );
    assert_eq!(
        runtime_profile_circuit_half_open_probe_seconds(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD),
        RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS
    );
    assert_eq!(
        runtime_profile_circuit_half_open_probe_seconds(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2),
        (RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS * 4)
            .min(RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS)
    );
}

#[test]
fn reserve_runtime_profile_route_circuit_half_open_probe_scales_wait_with_health() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    let now = Local::now().timestamp();
    let runtime = RuntimeRotationState {
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
        profile_route_circuit_open_until: BTreeMap::from([(
            runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses),
            now - 1,
        )]),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::from([(
            runtime_profile_route_health_key("main", RuntimeRouteKind::Responses),
            RuntimeProfileHealth {
                score: RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2,
                updated_at: now,
            },
        )]),
    };
    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(runtime)),
    };

    let reservation_started_at = Local::now().timestamp();
    assert!(
        reserve_runtime_profile_route_circuit_half_open_probe(
            &shared,
            "main",
            RuntimeRouteKind::Responses,
        )
        .expect("half-open reservation should succeed")
    );
    let reservation_finished_at = Local::now().timestamp();
    let probe_seconds = runtime_profile_circuit_half_open_probe_seconds(
        RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD + 2,
    );
    let actual_until = shared
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(
            "main",
            RuntimeRouteKind::Responses,
        ))
        .copied()
        .expect("circuit reservation should be recorded");
    assert!(
        actual_until >= reservation_started_at.saturating_add(probe_seconds)
            && actual_until <= reservation_finished_at.saturating_add(probe_seconds),
        "actual_until={actual_until} should fall within reservation window {}..={}",
        reservation_started_at.saturating_add(probe_seconds),
        reservation_finished_at.saturating_add(probe_seconds),
    );
}

#[test]
fn runtime_proxy_retries_overloaded_compact_on_another_profile() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[],\"instructions\":\"compact\"}")
        .send()
        .expect("runtime proxy compact request should succeed");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert!(status.is_success(), "unexpected compact status: {status}");
    assert_eq!(body, "{\"output\":[]}");
    let responses_accounts = backend.responses_accounts();
    assert_eq!(responses_accounts.len(), 3);
    assert_eq!(responses_accounts[0], "main-account");
    assert_eq!(responses_accounts[1], "main-account");
    assert!(matches!(
        responses_accounts[2].as_str(),
        "second-account" | "third-account"
    ));

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("main")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("main"));
}

#[test]
fn runtime_proxy_preserves_bound_profile_for_overloaded_compact_requests() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            "sess-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-main")
        .body("{\"input\":[],\"instructions\":\"compact\"}")
        .send()
        .expect("runtime proxy compact request should complete");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert!(
        status.is_success(),
        "bound compact should rotate pre-commit after owner overload: {status}"
    );
    assert_eq!(body, "{\"output\":[]}");
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string()
        ]
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("main")
            && state
                .session_profile_bindings
                .get("sess-main")
                .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("main"));
    assert_eq!(
        persisted
            .session_profile_bindings
            .get("sess-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
}

#[test]
fn runtime_proxy_persists_compact_lineage_after_overload_retry() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");
    let compact = client
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-compact")
        .body("{\"input\":[],\"instructions\":\"compact\"}")
        .send()
        .expect("compact request should succeed");
    let compact_turn_state = compact
        .headers()
        .get("x-codex-turn-state")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .expect("compact response should expose turn state");
    let compact_owner_profile =
        runtime_proxy_backend_profile_name_for_compact_turn_state(&compact_turn_state)
            .expect("compact turn state should map to a backend profile");
    let compact_owner_account =
        runtime_proxy_backend_account_id_for_profile_name(compact_owner_profile)
            .expect("compact owner should map to a backend account");
    assert!(compact.status().is_success());
    assert_eq!(
        compact
            .text()
            .expect("compact response body should be readable"),
        "{\"output\":[]}"
    );

    let continuations = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .session_id_bindings
            .get(&runtime_compact_session_lineage_key("sess-compact"))
            .is_some_and(|binding| binding.profile_name == compact_owner_profile)
            && continuations
                .turn_state_bindings
                .get(&runtime_compact_turn_state_lineage_key(&compact_turn_state))
                .is_some_and(|binding| binding.profile_name == compact_owner_profile)
    });
    assert_eq!(
        continuations
            .session_id_bindings
            .get(&runtime_compact_session_lineage_key("sess-compact"))
            .map(|binding| binding.profile_name.as_str()),
        Some(compact_owner_profile)
    );
    assert_eq!(
        continuations
            .turn_state_bindings
            .get(&runtime_compact_turn_state_lineage_key(&compact_turn_state))
            .map(|binding| binding.profile_name.as_str()),
        Some(compact_owner_profile)
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "main-account".to_string(),
            compact_owner_account.to_string(),
        ]
    );
}

#[test]
fn runtime_proxy_reuses_compact_owner_for_followup_until_response_commits() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");
    let compact = client
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-compact")
        .body("{\"input\":[],\"instructions\":\"compact\"}")
        .send()
        .expect("compact request should succeed");
    let compact_turn_state = compact
        .headers()
        .get("x-codex-turn-state")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .expect("compact response should expose turn state");
    let compact_owner_profile =
        runtime_proxy_backend_profile_name_for_compact_turn_state(&compact_turn_state)
            .expect("compact turn state should map to a backend profile");
    let compact_owner_account =
        runtime_proxy_backend_account_id_for_profile_name(compact_owner_profile)
            .expect("compact owner should map to a backend account");
    let followup_response_id =
        runtime_proxy_backend_initial_response_id_for_profile_name(compact_owner_profile)
            .expect("compact owner should expose an initial response id");
    assert!(compact.status().is_success());
    assert_eq!(
        compact
            .text()
            .expect("compact response body should be readable"),
        "{\"output\":[]}"
    );

    let followup = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-compact")
        .header("x-codex-turn-state", compact_turn_state.as_str())
        .body("{\"input\":[]}")
        .send()
        .expect("follow-up request should succeed");
    let followup_body = followup
        .text()
        .expect("follow-up response body should be readable");
    assert!(
        followup_body.contains(&format!("\"{followup_response_id}\"")),
        "unexpected compact follow-up body: {followup_body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "main-account".to_string(),
            compact_owner_account.to_string(),
            compact_owner_account.to_string(),
        ]
    );
}

#[test]
fn runtime_proxy_restores_compact_followup_owner_across_restart() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    initial_state
        .save(&paths)
        .expect("failed to save initial state");

    let client = Client::builder().build().expect("client");
    let (compact_turn_state, compact_owner_profile, compact_owner_account, followup_response_id) = {
        let proxy =
            start_runtime_rotation_proxy(&paths, &initial_state, "main", backend.base_url(), false)
                .expect("runtime proxy should start");
        let compact = client
            .post(format!(
                "http://{}/backend-api/codex/responses/compact",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .header("session_id", "sess-compact")
            .body("{\"input\":[],\"instructions\":\"compact\"}")
            .send()
            .expect("compact request should succeed");
        let compact_turn_state = compact
            .headers()
            .get("x-codex-turn-state")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
            .expect("compact response should expose turn state");
        let compact_owner_profile =
            runtime_proxy_backend_profile_name_for_compact_turn_state(&compact_turn_state)
                .expect("compact turn state should map to a backend profile");
        let compact_owner_account =
            runtime_proxy_backend_account_id_for_profile_name(compact_owner_profile)
                .expect("compact owner should map to a backend account");
        let followup_response_id =
            runtime_proxy_backend_initial_response_id_for_profile_name(compact_owner_profile)
                .expect("compact owner should expose an initial response id");
        assert!(compact.status().is_success());
        assert_eq!(
            compact
                .text()
                .expect("compact response body should be readable"),
            "{\"output\":[]}"
        );
        (
            compact_turn_state,
            compact_owner_profile,
            compact_owner_account,
            followup_response_id,
        )
    };

    let _ = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .session_id_bindings
            .get(&runtime_compact_session_lineage_key("sess-compact"))
            .is_some_and(|binding| binding.profile_name == compact_owner_profile)
    });

    let mut resumed_state = AppState::load(&paths).expect("state should reload");
    resumed_state.active_profile = Some("third".to_string());
    resumed_state
        .save(&paths)
        .expect("failed to save resumed state");

    let resumed_proxy =
        start_runtime_rotation_proxy(&paths, &resumed_state, "third", backend.base_url(), false)
            .expect("resumed runtime proxy should start");
    let followup = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            resumed_proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-compact")
        .header("x-codex-turn-state", compact_turn_state.as_str())
        .body("{\"input\":[]}")
        .send()
        .expect("follow-up request should succeed");
    let followup_body = followup
        .text()
        .expect("follow-up response body should be readable");
    assert!(
        followup_body.contains(&format!("\"{followup_response_id}\"")),
        "unexpected compact follow-up body after restart: {followup_body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "main-account".to_string(),
            compact_owner_account.to_string(),
            compact_owner_account.to_string(),
        ]
    );
}

#[test]
fn runtime_proxy_uses_current_profile_without_extra_runtime_quota_probe() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let startup_usage_accounts = sorted_backend_usage_accounts(&backend);
    assert_eq!(
        startup_usage_accounts,
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    assert!(body.contains("\"response.created\""));
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );

    let mut usage_accounts_after_request = backend.usage_accounts();
    usage_accounts_after_request.sort();
    assert_eq!(usage_accounts_after_request, startup_usage_accounts);
}

#[test]
fn runtime_proxy_sheds_long_lived_queue_overload_fast() {
    let _wait_budget_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS", "20");

    let temp_dir = TestDir::new();
    let shared = RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
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
        })),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
    };

    let (sender, _receiver) = mpsc::sync_channel::<u8>(1);
    sender.send(1).expect("queue should accept first item");
    let started_at = Instant::now();
    let result = wait_for_runtime_proxy_queue_capacity(
        2u8,
        &shared,
        "http",
        "/backend-api/codex/responses",
        |value| match sender.try_send(value) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(returned_value)) => {
                Err((RuntimeProxyQueueRejection::Full, returned_value))
            }
            Err(TrySendError::Disconnected(returned_value)) => {
                Err((RuntimeProxyQueueRejection::Disconnected, returned_value))
            }
        },
    );

    assert!(
        matches!(result, Err((RuntimeProxyQueueRejection::Full, 2))),
        "queue overload should fail fast once the bounded wait budget is exhausted"
    );
    assert!(
        started_at.elapsed() < Duration::from_millis(500),
        "queue overload response took too long: {:?}",
        started_at.elapsed()
    );
}

#[test]
fn runtime_proxy_absorbs_brief_long_lived_queue_burst() {
    let _wait_budget_guard = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
        "250",
    );

    let temp_dir = TestDir::new();
    let shared = RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
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
        })),
        log_path: temp_dir.path.join("runtime-proxy.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
    };

    let (sender, receiver) = mpsc::sync_channel::<u8>(1);
    let receiver = Arc::new(Mutex::new(receiver));
    sender.send(1).expect("queue should accept first item");

    let release_receiver = Arc::clone(&receiver);
    let release_wait = Arc::clone(&shared.lane_admission.wait);
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        let drained = release_receiver
            .lock()
            .expect("receiver mutex should not be poisoned")
            .recv_timeout(Duration::from_secs(1))
            .expect("queue should drain after short burst");
        assert_eq!(drained, 1);
        let (mutex, condvar) = &*release_wait;
        let _guard = mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        condvar.notify_all();
    });

    let started_at = Instant::now();
    assert!(
        wait_for_runtime_proxy_queue_capacity(
            2u8,
            &shared,
            "http",
            "/backend-api/codex/responses",
            |value| match sender.try_send(value) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(returned_value)) => {
                    Err((RuntimeProxyQueueRejection::Full, returned_value))
                }
                Err(TrySendError::Disconnected(returned_value)) => {
                    Err((RuntimeProxyQueueRejection::Disconnected, returned_value))
                }
            },
        )
        .is_ok(),
        "queue wait should recover once the short-lived burst drains"
    );
    assert!(
        started_at.elapsed() < Duration::from_millis(200),
        "queue wait should recover promptly after capacity is signaled: {:?}",
        started_at.elapsed()
    );

    release.join().expect("release thread should join");
    let queued = receiver
        .lock()
        .expect("receiver mutex should not be poisoned")
        .recv_timeout(Duration::from_secs(1))
        .expect("recovered item should reach the queue");
    assert_eq!(queued, 2);
}

#[test]
fn unchanged_runtime_probe_result_does_not_requeue_persistence() {
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    create_codex_home_if_missing(&profile_home).expect("profile home should be created");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let usage = usage_with_main_windows(81, 3600, 100, 604_800);
    let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
    let runtime = RuntimeRotationState {
        paths,
        state,
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::from([("main".to_string(), snapshot)]),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, usize::MAX);

    apply_runtime_profile_probe_result(
        &shared,
        "main",
        AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        Ok(usage),
    )
    .expect("unchanged probe result should apply");
    wait_for_runtime_background_queues_idle();

    let log = fs::read_to_string(&shared.log_path).unwrap_or_default();
    assert_eq!(
        shared.state_save_revision.load(Ordering::SeqCst),
        0,
        "unchanged probe results should not requeue persistence immediately: {log}"
    );
    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(runtime.profile_probe_cache.contains_key("main"));
    assert!(runtime.profile_usage_snapshots.contains_key("main"));
}

#[test]
fn runtime_proxy_reuses_rotated_profile_without_reprobing_quota() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let startup_usage_accounts = sorted_backend_usage_accounts(&backend);
    assert_eq!(
        startup_usage_accounts,
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let first = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("first runtime proxy request should succeed");
    let first_body = first
        .text()
        .expect("first response body should be readable");
    assert!(first_body.contains("\"response.created\""));

    let second = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("second runtime proxy request should succeed");
    let second_body = second
        .text()
        .expect("second response body should be readable");
    assert!(second_body.contains("\"response.created\""));

    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string(),
        ]
    );

    let mut usage_accounts_after_requests = backend.usage_accounts();
    usage_accounts_after_requests.sort();
    assert_eq!(usage_accounts_after_requests, startup_usage_accounts);
}

#[test]
fn runtime_proxy_passes_through_upstream_http_error_response() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
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
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");

    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = response.text().expect("response body should be readable");

    assert_eq!(status, reqwest::StatusCode::OK);
    assert!(content_type.contains("text/event-stream"));
    assert!(body.contains("\"type\":\"response.failed\""));
    assert!(body.contains("\"code\":\"insufficient_quota\""));
    assert!(body.contains("main quota exhausted"));
}

#[test]
fn runtime_proxy_passes_through_plain_429_without_rotating_profiles() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_plain_429();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");

    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert_eq!(status, reqwest::StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body, "Too Many Requests");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string()]
    );
}

#[test]
fn runtime_proxy_passes_through_unauthorized_response_and_quarantines_profile_for_next_fresh_request()
 {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_unauthorized_main();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let first = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("first runtime proxy request should succeed");
    assert_eq!(first.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        first.text().expect("first body should be readable"),
        "{\"error\":\"unauthorized\"}"
    );

    let second = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("second runtime proxy request should succeed");
    let second_body = second.text().expect("second body should be readable");
    assert!(
        second_body.contains("\"resp-second\""),
        "unexpected second body after auth quarantine: {second_body}"
    );

    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
}

#[test]
fn inspect_runtime_sse_buffer_holds_on_created_only_prelude() {
    let buffered = concat!(
        "event: response.created\r\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second\"}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    match inspect_runtime_sse_buffer(&buffered).expect("inspection should succeed") {
        RuntimeSseInspectionProgress::Hold { response_ids } => {
            assert_eq!(response_ids, vec!["resp-second".to_string()]);
        }
        other => panic!("unexpected inspection result: {other:?}"),
    }
}

#[test]
fn inspect_runtime_sse_buffer_detects_retryable_error_after_hold_frames() {
    let buffered = concat!(
        "event: response.created\r\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second\"}}\r\n",
        "\r\n",
        "event: response.failed\r\n",
        "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"previous_response_not_found\",\"message\":\"missing\"}}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    assert!(matches!(
        inspect_runtime_sse_buffer(&buffered).expect("inspection should succeed"),
        RuntimeSseInspectionProgress::PreviousResponseNotFound
    ));
}

#[test]
fn inspect_runtime_sse_buffer_detects_quota_blocked_after_hold_frames() {
    let buffered = concat!(
        "event: response.created\r\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second\"}}\r\n",
        "\r\n",
        "event: response.failed\r\n",
        "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"insufficient_quota\",\"message\":\"main quota exhausted\"}}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    assert!(matches!(
        inspect_runtime_sse_buffer(&buffered).expect("inspection should succeed"),
        RuntimeSseInspectionProgress::QuotaBlocked
    ));
}

#[test]
fn extract_runtime_response_ids_accepts_top_level_response_fields() {
    let response_id_only = extract_runtime_response_ids_from_payload(
        "{\"type\":\"response.output_item.done\",\"response_id\":\"resp-second-next\"}",
    );
    assert_eq!(response_id_only, vec!["resp-second-next".to_string()]);

    let realtime_object = serde_json::json!({
        "object": "realtime.response",
        "id": "resp-realtime"
    });
    assert_eq!(
        extract_runtime_response_ids_from_value(&realtime_object),
        vec!["resp-realtime".to_string()]
    );
}

#[test]
fn runtime_prefetch_send_with_wait_recovers_after_short_backpressure() {
    let _timeout_guard = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS",
        "50",
    );
    let _retry_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS", "1");
    let (sender, receiver) = mpsc::sync_channel::<RuntimePrefetchChunk>(1);
    sender
        .try_send(RuntimePrefetchChunk::Data(vec![1]))
        .expect("initial queue fill should succeed");
    let shared = Arc::new(RuntimePrefetchSharedState::default());
    let async_runtime = TokioRuntimeBuilder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("async runtime");
    let drain = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        let first = receiver
            .recv_timeout(Duration::from_millis(200))
            .expect("first chunk should drain");
        let second = receiver
            .recv_timeout(Duration::from_millis(200))
            .expect("second chunk should arrive after backpressure clears");
        (first, second)
    });

    let outcome = async_runtime.block_on(runtime_prefetch_send_with_wait(
        &sender,
        &shared,
        vec![2, 3],
    ));
    assert!(matches!(
        outcome,
        RuntimePrefetchSendOutcome::Sent { retries, .. } if retries > 0
    ));

    let (first, second) = drain.join().expect("drain thread should succeed");
    assert!(matches!(first, RuntimePrefetchChunk::Data(data) if data == vec![1]));
    assert!(matches!(second, RuntimePrefetchChunk::Data(data) if data == vec![2, 3]));
}

#[test]
fn runtime_prefetch_send_with_wait_times_out_when_backpressure_persists() {
    let _timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS", "5");
    let _retry_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS", "1");
    let (sender, _receiver) = mpsc::sync_channel::<RuntimePrefetchChunk>(1);
    sender
        .try_send(RuntimePrefetchChunk::Data(vec![1]))
        .expect("initial queue fill should succeed");
    let shared = Arc::new(RuntimePrefetchSharedState::default());
    let async_runtime = TokioRuntimeBuilder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("async runtime");

    let outcome = async_runtime.block_on(runtime_prefetch_send_with_wait(
        &sender,
        &shared,
        vec![2, 3],
    ));
    assert!(matches!(
        outcome,
        RuntimePrefetchSendOutcome::TimedOut { message }
        if message.contains("bounded capacity")
    ));
}

#[test]
fn runtime_prefetch_send_with_wait_times_out_when_buffered_bytes_stay_over_limit() {
    let _timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS", "5");
    let _retry_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS", "1");
    let _buffer_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES", "3");
    let (sender, _receiver) = mpsc::sync_channel::<RuntimePrefetchChunk>(4);
    let shared = Arc::new(RuntimePrefetchSharedState::default());
    shared.queued_bytes.store(2, Ordering::SeqCst);
    let async_runtime = TokioRuntimeBuilder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("async runtime");

    let outcome = async_runtime.block_on(runtime_prefetch_send_with_wait(
        &sender,
        &shared,
        vec![2, 3],
    ));
    assert!(matches!(
        outcome,
        RuntimePrefetchSendOutcome::TimedOut { message }
        if message.contains("safe limit")
    ));
}

#[test]
fn runtime_prefetch_reader_surfaces_terminal_backpressure_error_after_disconnect() {
    let (sender, receiver) = mpsc::sync_channel::<RuntimePrefetchChunk>(1);
    let shared = Arc::new(RuntimePrefetchSharedState::default());
    let async_runtime = TokioRuntimeBuilder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("async runtime");
    let worker = async_runtime.spawn(async {
        std::future::pending::<()>().await;
    });
    runtime_prefetch_set_terminal_error(
        &shared,
        std::io::ErrorKind::WouldBlock,
        "runtime prefetch backlog exceeded bounded capacity (2)",
    );
    drop(sender);
    let mut reader = RuntimePrefetchReader {
        receiver,
        shared,
        backlog: std::collections::VecDeque::new(),
        pending: Cursor::new(Vec::new()),
        finished: false,
        worker_abort: worker.abort_handle(),
    };

    let mut buffer = [0_u8; 16];
    let err = reader
        .read(&mut buffer)
        .expect_err("terminal prefetch error should surface to the reader");
    assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
    assert!(err.to_string().contains("bounded capacity"));
    drop(reader);
    let err = async_runtime
        .block_on(worker)
        .expect_err("prefetch worker should be aborted when the reader drops");
    assert!(err.is_cancelled());
}

#[test]
fn runtime_prefetch_stream_drop_aborts_worker() {
    let (sender, receiver) = mpsc::sync_channel::<RuntimePrefetchChunk>(1);
    let shared = Arc::new(RuntimePrefetchSharedState::default());
    let async_runtime = TokioRuntimeBuilder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("async runtime");
    let worker = async_runtime.spawn(async {
        std::future::pending::<()>().await;
    });
    drop(sender);

    let stream = RuntimePrefetchStream {
        receiver: Some(receiver),
        shared,
        backlog: std::collections::VecDeque::new(),
        worker_abort: Some(worker.abort_handle()),
    };

    drop(stream);
    let err = async_runtime
        .block_on(worker)
        .expect_err("prefetch worker should be aborted when the stream drops");
    assert!(err.is_cancelled());
}

#[test]
fn quota_message_extraction_recurses_into_nested_json_values() {
    let body = serde_json::json!({
        "status": 429,
        "errors": [
            {
                "meta": {
                    "detail": "You've hit your usage limit. To get more access now, send a request to your admin or try again at Apr 3rd, 2026 9:25 AM."
                }
            }
        ]
    })
    .to_string();

    let message = extract_runtime_proxy_quota_message(body.as_bytes())
        .expect("nested quota message should be detected");

    assert!(message.contains("You've hit your usage limit"));
}

#[test]
fn quota_message_extraction_falls_back_to_text_body() {
    let body =
        b"You've hit your usage limit. To get more access now, send a request to your admin or try again at Apr 3rd, 2026 9:25 AM.";

    let message =
        extract_runtime_proxy_quota_message(body).expect("text quota message should be detected");

    assert!(message.contains("You've hit your usage limit"));
}

#[test]
fn quota_message_extraction_detects_usage_limit_reached_type() {
    let body = serde_json::json!({
        "error": {
            "type": "usage_limit_reached",
            "message": "The usage limit has been reached",
            "plan_type": "team",
            "resets_at": 1775183113_i64
        }
    })
    .to_string();

    let message = extract_runtime_proxy_quota_message(body.as_bytes())
        .expect("usage_limit_reached type should be detected");

    assert_eq!(message, "The usage limit has been reached");
}

#[test]
fn runtime_proxy_returns_503_for_local_candidate_exhaustion_without_upstream_429() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");

    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should complete");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert_eq!(status, reqwest::StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("\"code\":\"service_unavailable\""));
    assert!(!body.contains("\"code\":\"insufficient_quota\""));
    assert!(backend.responses_accounts().is_empty());
}

#[test]
fn runtime_proxy_aborts_stalled_http_fallback_before_long_hang() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_initial_body_stall();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
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
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");

    let client = Client::builder().build().expect("client");
    let started = std::time::Instant::now();
    let result = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send();
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "stalled HTTP fallback took too long: {elapsed:?}"
    );
    match result {
        Ok(response) => {
            assert_eq!(
                response.status(),
                reqwest::StatusCode::OK,
                "runtime proxy should surface a dropped stream, not a synthetic HTTP error"
            );
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_string();
            assert!(
                content_type.contains("text/event-stream"),
                "runtime proxy should keep responses transport semantics on failure"
            );
        }
        Err(_) => {}
    }
}

#[test]
fn runtime_proxy_keeps_healthy_long_http_stream_alive() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_slow_stream();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");

    let client = Client::builder().build().expect("client");
    let started = std::time::Instant::now();
    let response_started = started;
    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let response_ready = response_started.elapsed();
    let body = response.text().expect("response body should be readable");
    let elapsed = started.elapsed();

    assert!(
        response_ready < Duration::from_millis(125),
        "runtime proxy waited too long before starting HTTP stream passthrough: {response_ready:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(300),
        "slow healthy stream completed too quickly to cover multi-chunk runtime read: {elapsed:?}"
    );
    assert!(body.contains("\"response.completed\""));
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_does_not_rotate_after_first_sse_chunk_reset() {
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    let _runtime_log_dir_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_log_dir.to_str().unwrap());
    let backend = RuntimeProxyBackend::start_http_reset_after_first_chunk();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let log_path = fs::read_to_string(runtime_proxy_latest_log_pointer_path())
        .expect("latest runtime pointer should exist");
    let log_path = PathBuf::from(log_path.trim());

    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("client");
    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    assert_eq!(response.status(), 200);
    let _ = response.text();

    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
    let mut tail = Vec::new();
    let mut observed = false;
    for _ in 0..80 {
        tail = read_runtime_log_tail(&log_path, 32 * 1024)
            .expect("runtime log tail should be readable");
        let text = String::from_utf8_lossy(&tail);
        if text.contains("first_upstream_chunk")
            && text.contains("first_local_chunk")
            && text.contains("stream_read_error")
        {
            observed = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(
        observed,
        "runtime log should capture first-chunk reset markers"
    );
    let tail = String::from_utf8_lossy(&tail);
    assert!(tail.contains("first_upstream_chunk"));
    assert!(tail.contains("first_local_chunk"));
    assert!(tail.contains("stream_read_error"));
}

#[test]
fn runtime_proxy_does_not_rotate_after_multi_chunk_sse_stall() {
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    let _runtime_log_dir_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_log_dir.to_str().unwrap());
    let backend = RuntimeProxyBackend::start_http_stall_after_several_chunks();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let log_path = fs::read_to_string(runtime_proxy_latest_log_pointer_path())
        .expect("latest runtime pointer should exist");
    let log_path = PathBuf::from(log_path.trim());

    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("client");
    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    assert_eq!(response.status(), 200);
    let _ = response.text();

    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
    let mut tail = Vec::new();
    let mut observed = false;
    for _ in 0..80 {
        tail = read_runtime_log_tail(&log_path, 32 * 1024)
            .expect("runtime log tail should be readable");
        let text = String::from_utf8_lossy(&tail);
        if text.contains("first_upstream_chunk")
            && text.contains("first_local_chunk")
            && text.contains("stream_read_error")
        {
            observed = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(
        observed,
        "runtime log should capture multi-chunk stall markers"
    );
    let tail = String::from_utf8_lossy(&tail);
    assert!(tail.contains("first_upstream_chunk"));
    assert!(tail.contains("first_local_chunk"));
    assert!(tail.contains("stream_read_error"));
}

#[test]
fn runtime_proxies_bind_distinct_local_ports() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    let paths_one = AppPaths {
        root: temp_dir.path.join("prodex-one"),
        state_file: temp_dir.path.join("prodex-one/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex-one/profiles"),
        shared_codex_root: temp_dir.path.join("shared-one"),
        legacy_shared_codex_root: temp_dir.path.join("prodex-one/shared"),
    };
    let paths_two = AppPaths {
        root: temp_dir.path.join("prodex-two"),
        state_file: temp_dir.path.join("prodex-two/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex-two/profiles"),
        shared_codex_root: temp_dir.path.join("shared-two"),
        legacy_shared_codex_root: temp_dir.path.join("prodex-two/shared"),
    };

    let proxy_one =
        start_runtime_rotation_proxy(&paths_one, &state, "main", backend.base_url(), false)
            .expect("first runtime proxy should start");
    let proxy_two =
        start_runtime_rotation_proxy(&paths_two, &state, "main", backend.base_url(), false)
            .expect("second runtime proxy should start");

    assert_ne!(proxy_one.listen_addr, proxy_two.listen_addr);
}

#[test]
fn runtime_proxy_websocket_rotates_on_upstream_websocket_quota_error() {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"response.created\""))
    );
    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"response.completed\""))
    );
    assert!(
        !payloads
            .iter()
            .any(|payload| payload.contains("main quota exhausted"))
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
}

#[test]
fn runtime_proxy_websocket_rotates_on_upstream_websocket_overload_error() {
    let backend = RuntimeProxyBackend::start_websocket_overloaded();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"response.created\""))
    );
    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"response.completed\""))
    );
    assert!(
        !payloads
            .iter()
            .any(|payload| payload.contains("Selected model is at capacity")),
        "capacity error should be rotated away before commit: {payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
}

#[test]
fn runtime_proxy_websocket_reuse_rotates_on_delayed_quota_before_commit() {
    let backend = RuntimeProxyBackend::start_websocket_delayed_quota_after_prelude();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("first runtime proxy websocket request should be sent");

    let mut first_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                first_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    assert!(
        first_payloads
            .iter()
            .any(|payload| payload.contains("\"resp-main\""))
    );

    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("second runtime proxy websocket request should be sent");

    let mut second_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                second_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        second_payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\"")),
        "unexpected websocket payloads: {second_payloads:?}"
    );
    assert!(
        !second_payloads
            .iter()
            .any(|payload| payload.contains("You've hit your usage limit")),
        "delayed quota error should not be surfaced after pre-commit rotate: {second_payloads:?}"
    );
    assert!(
        !second_payloads.iter().any(|payload| payload.contains("\"msg-main\"")),
        "failed quota pre-commit frames should not leak across rotated retry: {second_payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert_eq!(
        backend.websocket_requests().len(),
        3,
        "expected initial request, delayed quota retry on reused session, and rotated retry"
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
}

#[test]
fn runtime_proxy_websocket_reuse_rotates_on_delayed_overload_before_commit() {
    let backend = RuntimeProxyBackend::start_websocket_delayed_overload_after_prelude();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("first runtime proxy websocket request should be sent");

    let mut first_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                first_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    assert!(
        first_payloads
            .iter()
            .any(|payload| payload.contains("\"resp-main\""))
    );

    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("second runtime proxy websocket request should be sent");

    let mut second_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                second_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        second_payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\"")),
        "unexpected websocket payloads: {second_payloads:?}"
    );
    assert!(
        !second_payloads
            .iter()
            .any(|payload| payload.contains("Selected model is at capacity")),
        "capacity error should not be surfaced after pre-commit rotate: {second_payloads:?}"
    );
    assert!(
        !second_payloads.iter().any(|payload| payload.contains("\"msg-main\"")),
        "failed pre-commit frames should not leak across rotated retry: {second_payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert_eq!(
        backend.websocket_requests().len(),
        3,
        "expected initial request, delayed overload retry on reused session, and rotated retry"
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
}

#[test]
fn runtime_proxy_websocket_session_affinity_rotates_on_delayed_overload_before_commit() {
    let backend = RuntimeProxyBackend::start_websocket_delayed_overload_after_prelude();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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

    let mut request = tungstenite::client::IntoClientRequest::into_client_request(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("client request should build");
    request.headers_mut().insert(
        "session_id",
        "sess-ws-overload".parse().expect("session header"),
    );
    let (mut socket, _response) =
        tungstenite::connect(request).expect("runtime proxy websocket handshake should succeed");

    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("first runtime proxy websocket request should be sent");

    let mut first_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                first_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    assert!(
        first_payloads
            .iter()
            .any(|payload| payload.contains("\"resp-main\""))
    );

    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("second runtime proxy websocket request should be sent");

    let mut second_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                second_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        second_payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\"")),
        "unexpected websocket payloads: {second_payloads:?}"
    );
    assert!(
        !second_payloads
            .iter()
            .any(|payload| payload.contains("Selected model is at capacity")),
        "capacity error should not be surfaced after session-bound pre-commit rotate: {second_payloads:?}"
    );
    assert!(
        !second_payloads.iter().any(|payload| payload.contains("\"msg-main\"")),
        "failed pre-commit frames should not leak across rotated session retry: {second_payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert_eq!(
        backend.websocket_requests().len(),
        3,
        "expected initial request, delayed overload retry on reused session, and rotated retry"
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("main")
            && state
                .session_profile_bindings
                .get("sess-ws-overload")
                .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("main"));
    assert_eq!(
        persisted
            .session_profile_bindings
            .get("sess-ws-overload")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
    assert!(
        persisted.last_run_selected_at.is_empty(),
        "session-bound websocket continuation should not promote the global active profile"
    );
}

#[test]
fn runtime_proxy_websocket_releases_quota_blocked_previous_response_affinity_before_fresh_fallback()
{
    let backend = RuntimeProxyBackend::start_websocket_delayed_quota_after_prelude();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("first runtime proxy websocket request should be sent");

    let mut first_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                first_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    let first_response_id = first_payloads
        .iter()
        .flat_map(|payload| extract_runtime_response_ids_from_payload(payload))
        .last()
        .expect("first websocket response should expose a response id");
    assert_eq!(first_response_id, "resp-main");

    socket
        .send(WsMessage::Text(
            format!("{{\"previous_response_id\":\"{first_response_id}\",\"input\":[]}}").into(),
        ))
        .expect("second runtime proxy websocket request should be sent");

    let mut second_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                second_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        second_payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\"")),
        "unexpected websocket payloads after quota-blocked continuation fallback: {second_payloads:?}"
    );
    assert!(
        !second_payloads
            .iter()
            .any(|payload| payload.contains("You've hit your usage limit")),
        "quota-blocked previous_response should not surface usage limit: {second_payloads:?}"
    );
    assert!(
        !second_payloads
            .iter()
            .any(|payload| payload.contains("\"previous_response_not_found\"")),
        "previous_response fallback should stay pre-commit: {second_payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert_eq!(
        backend.websocket_requests().len(),
        3,
        "expected initial request, quota-blocked continuation, and fresh fallback"
    );
    assert_eq!(
        backend
            .websocket_requests()
            .iter()
            .filter(|request| request.contains("\"previous_response_id\":\"resp-main\""))
            .count(),
        1,
        "fresh fallback should not resend the old previous_response_id to another profile"
    );

    let persisted = wait_for_state(&paths, |state| {
        !state.response_profile_bindings.contains_key("resp-main")
            && state
                .response_profile_bindings
                .get("resp-second")
                .is_some_and(|binding| binding.profile_name == "second")
    });
    assert!(
        !persisted
            .response_profile_bindings
            .contains_key("resp-main")
    );
    assert_eq!(
        persisted
            .response_profile_bindings
            .get("resp-second")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
    let continuations = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .statuses
            .response
            .get("resp-main")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
    });
    assert_eq!(
        continuations
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_proxy_websocket_surfaces_mid_turn_close_without_post_commit_rotate() {
    let backend = RuntimeProxyBackend::start_websocket_close_mid_turn();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket.read() {
            Ok(WsMessage::Text(text)) => payloads.push(text.to_string()),
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed)
            | Err(_) => break,
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"response.created\""))
    );
    assert!(
        !payloads
            .iter()
            .any(|payload| payload.contains("\"response.completed\""))
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
}

#[test]
fn runtime_proxy_retries_after_websocket_reuse_silent_hang() {
    let backend = RuntimeProxyBackend::start_websocket_reuse_silent_hang();
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    let _runtime_log_dir_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_log_dir.to_str().unwrap());
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    let log_path = fs::read_to_string(runtime_proxy_latest_log_pointer_path())
        .expect("latest runtime pointer should exist");
    let log_path = PathBuf::from(log_path.trim());
    let startup_usage_accounts = sorted_backend_usage_accounts(&backend);
    assert_eq!(
        startup_usage_accounts,
        vec![
            "main-account".to_string(),
            "second-account".to_string(),
            "third-account".to_string(),
        ]
    );

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("first runtime proxy websocket request should be sent");

    let mut first_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open for first request")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                first_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    assert!(
        first_payloads
            .iter()
            .any(|payload| payload.contains("\"response.completed\""))
    );

    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("second runtime proxy websocket request should be sent");
    let mut second_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open for second request")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                second_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        second_payloads
            .iter()
            .any(|payload| payload.contains("\"response.completed\""))
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "second-account".to_string(),
            "third-account".to_string(),
        ]
    );
    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("third")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("third"));
    let mut tail = Vec::new();
    let mut observed = false;
    for _ in 0..160 {
        tail = read_runtime_log_tail(&log_path, 32 * 1024)
            .expect("runtime log tail should be readable");
        let text = String::from_utf8_lossy(&tail);
        if text.contains("websocket_reuse_watchdog")
            && text.contains("websocket_precommit_frame_timeout")
        {
            observed = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(
        observed,
        "runtime log should capture websocket reuse watchdog"
    );
    let tail = String::from_utf8_lossy(&tail);
    assert!(tail.contains("websocket_reuse_watchdog"));
    assert!(tail.contains("websocket_precommit_frame_timeout"));
}

#[test]
fn runtime_proxy_retries_same_compact_owner_after_websocket_reuse_watchdog() {
    let backend = RuntimeProxyBackend::start_websocket_reuse_silent_hang();
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    let _runtime_log_dir_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_log_dir.to_str().unwrap());
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");
    let mut request = format!("ws://{}/backend-api/codex/responses", proxy.listen_addr)
        .into_client_request()
        .expect("websocket request should build");
    request.headers_mut().insert(
        "session_id",
        "sess-compact".parse().expect("valid session header"),
    );

    let (mut socket, _response) =
        tungstenite::connect(request).expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("first runtime proxy websocket request should be sent");

    let mut first_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open for first request")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                first_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    assert!(
        first_payloads
            .iter()
            .any(|payload| payload.contains("\"response.completed\""))
    );

    let compact = client
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-compact")
        .body("{\"input\":[],\"instructions\":\"compact\"}")
        .send()
        .expect("compact request should succeed");
    assert!(
        compact.status().is_success(),
        "unexpected compact status: {}",
        compact.status()
    );
    assert_eq!(
        compact
            .text()
            .expect("compact response body should be readable"),
        "{\"output\":[]}"
    );

    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("second runtime proxy websocket request should be sent");

    let mut second_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open for compact owner retry")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                second_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        second_payloads
            .iter()
            .any(|payload| payload.contains("\"response.completed\""))
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "second-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string(),
        ],
        "compact owner should reconnect fresh on the same profile instead of failing or rotating"
    );
    assert_eq!(
        backend.websocket_requests().len(),
        3,
        "proxy should issue one fresh websocket reconnect after the failed reuse attempt"
    );

    let mut tail = Vec::new();
    let mut observed_retry = false;
    for _ in 0..160 {
        let Some(log_path) = prodex_runtime_log_paths_in_dir(&runtime_log_dir)
            .into_iter()
            .next_back()
        else {
            thread::sleep(Duration::from_millis(25));
            continue;
        };
        tail = read_runtime_log_tail(&log_path, 32 * 1024)
            .expect("runtime log tail should be readable");
        let text = String::from_utf8_lossy(&tail);
        if text.contains("websocket_reuse_owner_fresh_retry") {
            observed_retry = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(
        observed_retry,
        "runtime log should capture a fresh reconnect on the compact follow-up owner"
    );
    let tail = String::from_utf8_lossy(&tail);
    assert!(tail.contains("websocket_reuse_owner_fresh_retry"));
    assert!(
        !tail.contains("compact_fresh_fallback_blocked"),
        "compact follow-up owner should not be blocked after a reuse watchdog when a fresh reconnect is possible"
    );
}

#[test]
fn runtime_proxy_preserves_compact_lineage_after_websocket_previous_response_fallback() {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");
    let now = Local::now().timestamp();
    let compact_key = runtime_compact_session_lineage_key("sess-compact");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-stale".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            )]),
            session_id_bindings: BTreeMap::from([(
                compact_key.clone(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-stale".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 2,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("websocket".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                session_id: BTreeMap::from([(
                    compact_key.clone(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 2,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("compact".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to seed runtime continuations");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    set_test_websocket_io_timeout(&mut socket, Duration::from_secs(5));
    socket
        .send(WsMessage::Text(
            "{\"session_id\":\"sess-compact\",\"previous_response_id\":\"resp-stale\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open for fallback request")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-third\"")),
        "previous_response fallback should complete on third-account: {payloads:?}"
    );
    assert!(
        !payloads
            .iter()
            .any(|payload| payload.contains("\"previous_response_not_found\"")),
        "previous_response_not_found should stay pre-commit: {payloads:?}"
    );

    let persisted = wait_for_state(&paths, |state| state.active_profile.as_deref() == Some("third"));
    assert_eq!(persisted.active_profile.as_deref(), Some("third"));

    let continuations = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .session_id_bindings
            .get(&compact_key)
            .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(
        continuations
            .session_id_bindings
            .get(&compact_key)
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
    assert_ne!(
        continuations
            .statuses
            .session_id
            .get(&compact_key)
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );

    let resumed_compact = client
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-compact")
        .body("{\"input\":[],\"instructions\":\"compact-again\"}")
        .send()
        .expect("second compact request should succeed");
    let resumed_turn_state = resumed_compact
        .headers()
        .get("x-codex-turn-state")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .expect("second compact response should expose turn state");
    assert_eq!(resumed_turn_state, "compact-turn-second");
    assert!(resumed_compact.status().is_success());
    assert_eq!(
        resumed_compact
            .text()
            .expect("second compact response body should be readable"),
        "{\"output\":[]}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "second-account".to_string(),
            "third-account".to_string(),
            "third-account".to_string(),
            "second-account".to_string(),
        ]
    );
}

#[test]
fn runtime_proxy_bound_previous_response_without_turn_state_fails_as_transport_after_websocket_reuse_watchdog()
 {
    let backend = RuntimeProxyBackend::start_websocket_reuse_previous_response_needs_turn_state();
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    let _runtime_log_dir_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_log_dir.to_str().unwrap());
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("first runtime proxy websocket request should be sent");

    let mut first_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open for first request")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                first_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    assert!(
        first_payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\""))
    );

    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("bound continuation websocket request should be sent");

    let mut saw_close = false;
    for _ in 0..4 {
        match socket.read() {
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => {
                saw_close = true;
                break;
            }
            Ok(WsMessage::Text(text)) => {
                panic!(
                    "non-replayable websocket continuation should fail as transport instead of returning text payloads: {text}"
                );
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
            Err(_) => {
                saw_close = true;
                break;
            }
        }
    }
    assert!(
        saw_close,
        "websocket continuation without replayable turn_state should close the local transport"
    );

    assert_eq!(
        backend.websocket_requests().len(),
        2,
        "backend should stop after the failed reuse request instead of reconnecting with previous_response_id"
    );
    assert!(
        backend
            .websocket_requests()
            .last()
            .is_some_and(|request| request.contains("\"previous_response_id\":\"resp-second\"")),
        "failed reuse request should still preserve previous_response_id"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()],
        "proxy should not open a fresh owner reconnect after the failed reuse watchdog"
    );

    let mut observed = false;
    for _ in 0..80 {
        let Some(log_path) = prodex_runtime_log_paths_in_dir(&runtime_log_dir)
            .into_iter()
            .next_back()
        else {
            thread::sleep(Duration::from_millis(10));
            continue;
        };
        let tail = read_runtime_log_tail(&log_path, 32 * 1024)
            .expect("runtime log tail should be readable");
        let text = String::from_utf8_lossy(&tail);
        if text.contains("websocket_reuse_previous_response_blocked") {
            observed = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        observed,
        "runtime log should capture blocking of websocket reuse retry without turn_state"
    );
}

#[test]
fn runtime_proxy_stale_websocket_previous_response_reuse_fails_as_transport() {
    let backend = RuntimeProxyBackend::start_websocket_stale_reuse_needs_turn_state();
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    let _runtime_log_dir_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_log_dir.to_str().unwrap());
    let _stale_guard = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS",
        "1",
    );
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("first runtime proxy websocket request should be sent");

    let mut first_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open for first request")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                first_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    assert!(
        first_payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\""))
    );

    thread::sleep(Duration::from_millis(10));
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("bound continuation websocket request should be sent");

    let mut saw_close = false;
    for _ in 0..4 {
        match socket.read() {
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => {
                saw_close = true;
                break;
            }
            Ok(WsMessage::Text(text)) => {
                panic!("stale websocket reuse should not forward text payloads: {text}");
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
            Err(_) => {
                saw_close = true;
                break;
            }
        }
    }
    assert!(
        saw_close,
        "stale websocket reuse should close the local transport"
    );
    assert_eq!(
        backend.websocket_requests().len(),
        2,
        "proxy should stop after the failed reuse request instead of reconnecting with previous_response_id"
    );

    let mut observed = false;
    for _ in 0..80 {
        let Some(log_path) = prodex_runtime_log_paths_in_dir(&runtime_log_dir)
            .into_iter()
            .next_back()
        else {
            thread::sleep(Duration::from_millis(10));
            continue;
        };
        let tail = read_runtime_log_tail(&log_path, 32 * 1024)
            .expect("runtime log tail should be readable");
        let text = String::from_utf8_lossy(&tail);
        if text.contains("websocket_reuse_stale_previous_response_blocked") {
            observed = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        observed,
        "runtime log should capture stale websocket reuse blocking"
    );
}

#[test]
fn runtime_proxy_logs_local_writer_disconnect_after_first_chunk() {
    let temp_dir = TestDir::new();
    let runtime_log_dir = temp_dir.path.join("runtime-logs");
    let _runtime_log_dir_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", runtime_log_dir.to_str().unwrap());
    let log_path = temp_dir.path.join("runtime-proxy.log");
    let shared = RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "second".to_string(),
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
        log_path: log_path.clone(),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
    };
    let body = TwoChunkReader::new(vec![
        b"data: first\n\n".to_vec(),
        b"data: second\n\n".to_vec(),
    ]);
    let response = RuntimeStreamingResponse {
        status: 200,
        headers: vec![("Content-Type".to_string(), "text/event-stream".to_string())],
        body: Box::new(body),
        request_id: 1,
        profile_name: "second".to_string(),
        log_path: log_path.clone(),
        shared,
        _inflight_guard: None,
    };
    let writer: Box<dyn Write + Send + 'static> = Box::new(FailAfterFirstChunkWriter::new());

    let error = write_runtime_streaming_response(writer, response)
        .expect_err("writer disconnect should surface as an error");
    assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);

    let tail =
        read_runtime_log_tail(&log_path, 32 * 1024).expect("runtime log tail should be readable");
    let tail = String::from_utf8_lossy(&tail);
    assert!(tail.contains("first_local_chunk"));
    assert!(tail.contains("local_writer_error"));
}

#[test]
fn runtime_proxy_non_sse_stream_does_not_flush_each_chunk() {
    let temp_dir = TestDir::new();
    let log_path = temp_dir.path.join("runtime-proxy.log");
    let shared = RuntimeRotationProxyShared {
        async_client: reqwest::Client::builder().build().expect("async client"),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("async runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "second".to_string(),
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
        log_path: log_path.clone(),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
    };
    let body = TwoChunkReader::new(vec![b"{\"partial\":".to_vec(), b"1}".to_vec()]);
    let response = RuntimeStreamingResponse {
        status: 200,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: Box::new(body),
        request_id: 1,
        profile_name: "second".to_string(),
        log_path: log_path.clone(),
        shared,
        _inflight_guard: None,
    };
    let writer: Box<dyn Write + Send + 'static> =
        Box::new(FailOnUnexpectedMidStreamFlushWriter::new());

    write_runtime_streaming_response(writer, response)
        .expect("non-SSE stream should not flush each chunk before trailer");

    let tail =
        read_runtime_log_tail(&log_path, 32 * 1024).expect("runtime log tail should be readable");
    let tail = String::from_utf8_lossy(&tail);
    assert!(tail.contains("first_local_chunk"));
    assert!(!tail.contains("local_writer_error"));
}

#[test]
fn runtime_proxy_passes_through_upstream_websocket_error_payload() {
    let backend = RuntimeProxyBackend::start_websocket();
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

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("runtime proxy websocket request should be sent");

    let payload = socket
        .read()
        .expect("runtime proxy websocket should return an upstream error payload");
    let text = payload
        .into_text()
        .expect("upstream error payload should stay text")
        .to_string();

    assert!(text.contains("\"type\":\"error\""));
    assert!(text.contains("\"status\":429"));
    assert!(text.contains("\"code\":\"insufficient_quota\""));
    assert!(text.contains("main quota exhausted"));
}

#[test]
fn runtime_proxy_keeps_previous_response_affinity_for_http_requests() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    let client = Client::builder().build().expect("client");

    let first = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("first runtime proxy request should succeed");
    let first_body = first
        .text()
        .expect("first response body should be readable");
    let first_response_id = runtime_proxy_response_ids_from_http_body(&first_body)
        .into_iter()
        .last()
        .expect("first response should expose a response id");
    let owner_account = runtime_proxy_backend_response_owner_account_id(&first_response_id)
        .expect("first response id should map to a backend account");
    let second_response_id = runtime_proxy_backend_next_response_id(Some(&first_response_id))
        .expect("continuation response id should exist");
    assert!(
        first_body.contains(&format!("\"{first_response_id}\"")),
        "unexpected first buffered-json body: {first_body}"
    );

    let second = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body(format!(
            "{{\"previous_response_id\":\"{first_response_id}\",\"input\":[]}}"
        ))
        .send()
        .expect("second runtime proxy request should succeed");
    let second_body = second
        .text()
        .expect("second response body should be readable");
    assert!(
        second_body.contains(&format!("\"{second_response_id}\"")),
        "unexpected continued HTTP body: {second_body}"
    );
    let accounts = backend.responses_accounts();
    assert!(
        accounts.len() >= 2,
        "expected at least two upstream responses, got {accounts:?}"
    );
    assert_eq!(
        &accounts[accounts.len() - 2..],
        &[owner_account.to_string(), owner_account.to_string()]
    );
    assert!(
        !accounts.iter().any(|account| account == "third-account"),
        "previous-response affinity should not spill into a different profile: {accounts:?}"
    );
}

#[test]
fn runtime_proxy_persists_previous_response_affinity_across_restart() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    initial_state
        .save(&paths)
        .expect("failed to save initial state");

    let client = Client::builder().build().expect("client");
    let (first_response_id, owner_account) = {
        let proxy =
            start_runtime_rotation_proxy(&paths, &initial_state, "main", backend.base_url(), false)
                .expect("runtime proxy should start");

        let first = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send()
            .expect("first runtime proxy request should succeed");
        let first_body = first
            .text()
            .expect("first response body should be readable");
        let first_response_id = runtime_proxy_response_ids_from_http_body(&first_body)
            .into_iter()
            .last()
            .expect("first response should expose a response id");
        let owner_account = runtime_proxy_backend_response_owner_account_id(&first_response_id)
            .expect("first response id should map to a backend account");
        assert!(
            first_body.contains(&format!("\"{first_response_id}\"")),
            "unexpected first response body after quota rotation: {first_body}"
        );
        (first_response_id, owner_account)
    };
    let second_response_id = runtime_proxy_backend_next_response_id(Some(&first_response_id))
        .expect("continuation response id should exist");

    let mut resumed_state = AppState::load(&paths).expect("state should reload");
    resumed_state.active_profile = Some("third".to_string());
    resumed_state
        .save(&paths)
        .expect("failed to save resumed state");

    let resumed_proxy =
        start_runtime_rotation_proxy(&paths, &resumed_state, "third", backend.base_url(), false)
            .expect("resumed runtime proxy should start");

    let second = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            resumed_proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body(format!(
            "{{\"previous_response_id\":\"{first_response_id}\",\"input\":[]}}"
        ))
        .send()
        .expect("second runtime proxy request should succeed");
    let second_body = second
        .text()
        .expect("second response body should be readable");
    assert!(
        second_body.contains(&format!("\"{second_response_id}\"")),
        "unexpected continued HTTP body after restart: {second_body}"
    );
    let accounts = backend.responses_accounts();
    assert!(
        accounts.len() >= 2,
        "expected at least two upstream responses, got {accounts:?}"
    );
    assert_eq!(
        &accounts[accounts.len() - 2..],
        &[owner_account.to_string(), owner_account.to_string()]
    );
    assert!(
        !accounts.iter().any(|account| account == "third-account"),
        "persisted previous-response affinity should not spill into a different profile after restart: {accounts:?}"
    );
}

#[test]
fn runtime_proxy_restores_previous_response_affinity_from_continuation_sidecar() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: Local::now().timestamp(),
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to save continuation sidecar");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");
    assert!(body.contains("\"resp-second-next\""));
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_restores_previous_response_affinity_from_continuation_journal() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");
    let saved_at = Local::now().timestamp();
    save_runtime_continuation_journal(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: saved_at,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-second".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 2,
                        last_touched_at: Some(saved_at),
                        last_verified_at: Some(saved_at),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        saved_at,
    )
    .expect("failed to save continuation journal");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");
    assert!(body.contains("\"resp-second-next\""));
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_persists_turn_state_to_continuation_sidecar() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save state");

    let shared = RuntimeRotationProxyShared {
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
        active_request_limit: usize::MAX,
        lane_admission: runtime_proxy_lane_admission_for_global_limit(usize::MAX),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state,
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
        })),
    };

    remember_runtime_turn_state(
        &shared,
        "second",
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("turn-state should persist");
    let persisted = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .turn_state_bindings
            .get("turn-second")
            .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(
        persisted
            .turn_state_bindings
            .get("turn-second")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
}

#[test]
fn runtime_proxy_restores_turn_state_affinity_from_continuation_sidecar() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            turn_state_bindings: BTreeMap::from([(
                "turn-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: Local::now().timestamp(),
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to save continuation sidecar");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-codex-turn-state", "turn-second")
        .body("{\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");
    assert!(body.contains("\"resp-second\""));
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_persists_previous_response_affinity_for_buffered_json_responses() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    let client = Client::builder().build().expect("client");

    let first = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("first runtime proxy request should succeed");
    let first_body = first
        .text()
        .expect("first response body should be readable");
    let first_response_id = runtime_proxy_response_ids_from_http_body(&first_body)
        .into_iter()
        .last()
        .expect("first response should expose a response id");
    let owner_account = runtime_proxy_backend_response_owner_account_id(&first_response_id)
        .expect("first response id should map to a backend account");
    let second_response_id = runtime_proxy_backend_next_response_id(Some(&first_response_id))
        .expect("continuation response id should exist");
    assert!(
        first_body.contains(&format!("\"{first_response_id}\"")),
        "unexpected first buffered-json body: {first_body}"
    );

    let second = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body(format!(
            "{{\"previous_response_id\":\"{first_response_id}\",\"input\":[]}}"
        ))
        .send()
        .expect("second runtime proxy request should succeed");
    let second_body = second
        .text()
        .expect("second response body should be readable");
    assert!(
        second_body.contains(&format!("\"{second_response_id}\"")),
        "unexpected buffered-json continuation body: {second_body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            owner_account.to_string(),
            owner_account.to_string(),
        ]
    );
}

#[test]
fn runtime_proxy_releases_stale_previous_response_binding_after_not_found_http() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-second".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    assert!(
        body.contains("\"resp-second-next\""),
        "unexpected HTTP retry body: {body}"
    );
    let accounts = backend.responses_accounts();
    assert!(
        accounts.iter().any(|account| account == "main-account"),
        "stale bound profile should be tried first: {accounts:?}"
    );
    assert_eq!(
        accounts.last().map(String::as_str),
        Some("second-account"),
        "owner should be rediscovered after stale binding release: {accounts:?}"
    );

    let persisted = wait_for_state(&paths, |state| {
        state
            .response_profile_bindings
            .get("resp-second")
            .is_none_or(|binding| binding.profile_name != "main")
    });
    assert!(
        persisted
            .response_profile_bindings
            .get("resp-second")
            .is_none_or(|binding| binding.profile_name != "main"),
        "stale previous_response_id should not stay pinned to the wrong owner"
    );
}

#[test]
fn runtime_proxy_releases_stale_previous_response_binding_after_not_found_websocket() {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-second".to_string(),
            ResponseProfileBinding {
                profile_name: "third".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    set_test_websocket_io_timeout(&mut socket, Duration::from_secs(5));
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => {
                break;
            }
            Err(err) => {
                panic!(
                    "runtime proxy websocket failed while waiting for retry payloads: {err}; payloads={payloads:?}"
                );
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second-next\"")),
        "unexpected websocket retry payloads: {payloads:?}"
    );
    let accounts = backend.responses_accounts();
    assert_eq!(
        accounts.first().map(String::as_str),
        Some("third-account"),
        "stale bound profile should be tried first: {accounts:?}"
    );
    assert_eq!(
        accounts.last().map(String::as_str),
        Some("second-account"),
        "owner should be rediscovered after stale binding release: {accounts:?}"
    );

    let first_request_count = backend.responses_accounts().len();
    let (mut retry_socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("second runtime proxy websocket handshake should succeed");
    set_test_websocket_io_timeout(&mut retry_socket, Duration::from_secs(5));
    retry_socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("second runtime proxy websocket request should be sent");

    let mut retry_payloads = Vec::new();
    loop {
        match retry_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                retry_payloads.push(text);
                if done {
                    break;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                retry_socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => break,
            Err(err) => {
                panic!(
                    "runtime proxy websocket failed while waiting for retry-after-release payloads: {err}; payloads={retry_payloads:?}"
                );
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        retry_payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second-next\"")),
        "unexpected websocket retry-after-release payloads: {retry_payloads:?}"
    );
    let retry_accounts = backend.responses_accounts()[first_request_count..].to_vec();
    assert_eq!(
        retry_accounts,
        vec!["second-account".to_string()],
        "follow-up websocket discovery should not retry the stale owner once negative cache is active"
    );
}

#[test]
fn runtime_proxy_persists_session_affinity_across_restart_for_compact() {
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    initial_state
        .save(&paths)
        .expect("failed to save initial state");

    let client = Client::builder().build().expect("client");
    let owner_profile = {
        let proxy =
            start_runtime_rotation_proxy(&paths, &initial_state, "main", backend.base_url(), false)
                .expect("runtime proxy should start");

        let first = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .header("session_id", "sess-second")
            .body("{\"input\":[]}")
            .send()
            .expect("first runtime proxy request should succeed");
        let first_body = first
            .text()
            .expect("first response body should be readable");
        let first_response_id = runtime_proxy_response_ids_from_http_body(&first_body)
            .into_iter()
            .last()
            .expect("first response should expose a response id");
        let owner_profile = runtime_proxy_backend_profile_name_for_response_id(&first_response_id)
            .expect("first response id should map to a backend profile");
        assert!(
            first_body.contains(&format!("\"{first_response_id}\"")),
            "unexpected session-affinity seed body: {first_body}"
        );
        owner_profile
    };
    let owner_account = runtime_proxy_backend_account_id_for_profile_name(owner_profile)
        .expect("owner profile should map to a backend account");
    let seed_accounts = backend.responses_accounts();
    let seed_request_count = seed_accounts.len();
    assert_eq!(
        seed_accounts.last().map(String::as_str),
        Some(owner_account),
        "seed request should commit on the discovered session owner: {seed_accounts:?}"
    );

    let persisted = wait_for_state(&paths, |state| {
        state
            .session_profile_bindings
            .get("sess-second")
            .is_some_and(|binding| binding.profile_name == owner_profile)
    });
    assert_eq!(
        persisted
            .session_profile_bindings
            .get("sess-second")
            .map(|binding| binding.profile_name.as_str()),
        Some(owner_profile)
    );

    let mut resumed_state = AppState::load(&paths).expect("state should reload");
    let resumed_current_profile = if owner_profile == "second" {
        "third"
    } else {
        "second"
    };
    resumed_state.active_profile = Some(resumed_current_profile.to_string());
    resumed_state
        .save(&paths)
        .expect("failed to save resumed state");

    let resumed_proxy = start_runtime_rotation_proxy(
        &paths,
        &resumed_state,
        resumed_current_profile,
        backend.base_url(),
        false,
    )
    .expect("resumed runtime proxy should start");

    let compact = client
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            resumed_proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-second")
        .body("{\"input\":[],\"instructions\":\"compact\"}")
        .send()
        .expect("compact request should succeed");
    assert!(compact.status().is_success());
    assert_eq!(
        compact
            .text()
            .expect("compact response body should be readable"),
        "{\"output\":[]}"
    );
    let resumed_accounts = backend.responses_accounts();
    assert_eq!(
        resumed_accounts[seed_request_count..].to_vec(),
        vec![owner_account.to_string()],
        "compact follow-up should reuse the persisted session owner even after restart on a different current profile: {resumed_accounts:?}"
    );
}

#[test]
fn runtime_proxy_discovers_previous_response_owner_without_saved_binding_http() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    let accounts = backend.responses_accounts();
    assert!(
        body.contains("\"resp-second-next\""),
        "unexpected HTTP discovery body: {body}; accounts: {accounts:?}"
    );
    assert_eq!(
        accounts,
        vec![
            "third-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string(),
        ]
    );
}

#[test]
fn runtime_proxy_discovers_previous_response_owner_without_saved_binding_websocket() {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    set_test_websocket_io_timeout(&mut socket, Duration::from_secs(5));
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => break,
            Err(err) => {
                panic!(
                    "runtime proxy websocket failed while waiting for previous_response turn-state retry payloads: {err}; payloads={payloads:?}"
                );
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second-next\"")),
        "unexpected websocket discovery payloads: {payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "third-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string(),
        ]
    );
}

#[test]
fn runtime_proxy_previous_response_discovery_ignores_compact_followup_http() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            session_id_bindings: BTreeMap::from([(
                runtime_compact_session_lineage_key("sess-main"),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: Local::now().timestamp(),
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to save continuation sidecar");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("session_id", "sess-main")
        .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    let accounts = backend.responses_accounts();
    assert!(
        body.contains("\"resp-second-next\""),
        "unexpected HTTP discovery body: {body}; accounts: {accounts:?}"
    );
    assert_eq!(
        accounts,
        vec![
            "third-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string(),
        ],
        "previous_response discovery should not be hijacked by compact session lineage"
    );
}

#[test]
fn runtime_proxy_previous_response_discovery_ignores_compact_followup_websocket() {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            session_id_bindings: BTreeMap::from([(
                runtime_compact_session_lineage_key("sess-main"),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: Local::now().timestamp(),
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to save continuation sidecar");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    set_test_websocket_io_timeout(&mut socket, Duration::from_secs(5));
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => break,
            Err(err) => {
                panic!(
                    "runtime proxy websocket failed while waiting for upstream turn-state retry payloads: {err}; payloads={payloads:?}"
                );
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second-next\"")),
        "unexpected websocket discovery payloads: {payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "third-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string(),
        ],
        "previous_response discovery should not be hijacked by compact session lineage"
    );
}

#[test]
fn runtime_proxy_falls_back_to_fresh_request_when_previous_response_missing_everywhere_http() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-missing\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    assert!(
        body.contains("\"resp-second\"") || body.contains("\"resp-third\""),
        "proxy should degrade to a fresh request after previous response discovery exhausts: {body}"
    );
    let accounts = backend.responses_accounts();
    assert!(
        accounts.iter().any(|account| account == "second-account"),
        "discovery should still probe alternate owners before falling back fresh: {accounts:?}"
    );
    assert!(
        matches!(
            accounts.last().map(String::as_str),
            Some("second-account" | "third-account")
        ),
        "fresh fallback should complete on a healthy candidate without reusing the exhausted profile: {accounts:?}"
    );
    let bodies = backend.responses_bodies();
    assert!(
        bodies
            .last()
            .is_some_and(|request| !request.contains("\"previous_response_id\":\"resp-missing\"")),
        "fresh fallback should strip previous_response_id before the final attempt: {bodies:?}"
    );
}

#[test]
fn runtime_proxy_falls_back_to_fresh_request_when_previous_response_missing_everywhere_websocket() {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-missing\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads.iter().any(|payload| {
            payload.contains("\"resp-second\"") || payload.contains("\"resp-third\"")
        }),
        "proxy should degrade to a fresh websocket request after previous response discovery exhausts: {payloads:?}"
    );
    let accounts = backend.responses_accounts();
    assert!(
        accounts.iter().any(|account| account == "second-account"),
        "discovery should still probe alternate websocket owners before falling back fresh: {accounts:?}"
    );
    assert!(
        matches!(
            accounts.last().map(String::as_str),
            Some("second-account" | "third-account")
        ),
        "fresh websocket fallback should complete on a healthy candidate without reusing the exhausted profile: {accounts:?}"
    );
    let upstream_requests = backend.websocket_requests();
    assert!(
        upstream_requests
            .last()
            .is_some_and(|request| !request.contains("\"previous_response_id\":\"resp-missing\"")),
        "fresh websocket fallback should strip previous_response_id before the final attempt: {upstream_requests:?}"
    );
}

#[test]
fn runtime_proxy_falls_back_to_fresh_request_when_previous_response_missing_with_stale_turn_state_http()
 {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-codex-turn-state", "turn-stale")
        .body("{\"previous_response_id\":\"resp-missing\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert_eq!(
        status.as_u16(),
        200,
        "unexpected status: {status} body={body}"
    );
    assert!(
        body.contains("\"resp-second\"") || body.contains("\"resp-third\""),
        "proxy should degrade to a fresh request after previous response discovery exhausts: {body}"
    );

    let headers = backend.responses_headers();
    let final_headers = headers.last().expect("final request should be captured");
    assert!(
        final_headers
            .get("chatgpt-account-id")
            .is_some_and(|account_id| matches!(
                account_id.as_str(),
                "second-account" | "third-account"
            )),
        "fresh fallback should complete on a healthy candidate: {headers:?}"
    );
    assert!(
        !final_headers.contains_key("x-codex-turn-state"),
        "fresh fallback should strip stale turn-state before the final attempt: {headers:?}"
    );
}

#[test]
fn runtime_proxy_clears_stale_previous_response_binding_after_safe_http_fresh_fallback() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    let now = Local::now().timestamp();

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("initial state should save");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-main\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let status = response.status();
    let body = response.text().expect("response body should be readable");

    assert_eq!(
        status.as_u16(),
        200,
        "unexpected status: {status} body={body}"
    );
    assert!(
        body.contains("\"resp-second\""),
        "proxy should degrade to a fresh request after clearing stale binding: {body}"
    );

    let persisted = wait_for_state(&paths, |state| {
        !state.response_profile_bindings.contains_key("resp-main")
    });
    assert!(
        !persisted
            .response_profile_bindings
            .contains_key("resp-main"),
        "stale previous_response binding should be cleared after safe fallback: {:?}",
        persisted.response_profile_bindings
    );
}

#[test]
fn runtime_proxy_retries_previous_response_with_upstream_turn_state_http() {
    let backend = RuntimeProxyBackend::start_http_previous_response_needs_turn_state();
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let response = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
        .send()
        .expect("runtime proxy request should succeed");
    let body = response.text().expect("response body should be readable");

    assert!(
        body.contains("\"resp-second-next\""),
        "unexpected HTTP retry body: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string(), "second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_retries_previous_response_with_upstream_turn_state_websocket() {
    let backend = RuntimeProxyBackend::start_websocket_previous_response_needs_turn_state();
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    set_test_websocket_io_timeout(&mut socket, Duration::from_secs(5));
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => break,
            Err(err) => {
                panic!(
                    "runtime proxy websocket failed while waiting for upstream turn-state retry payloads: {err}; payloads={payloads:?}"
                );
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second-next\"")),
        "unexpected websocket retry payloads: {payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string(), "second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_keeps_multi_turn_previous_response_chain_on_websocket_owner() {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");

    let mut previous_response_id: Option<String> = None;
    for expected_response_id in ["resp-second", "resp-second-next", "resp-second-next-next"] {
        let request = previous_response_id
            .as_deref()
            .map(|previous_response_id| {
                format!("{{\"previous_response_id\":\"{previous_response_id}\",\"input\":[]}}")
            })
            .unwrap_or_else(|| "{\"input\":[]}".to_string());
        socket
            .send(WsMessage::Text(request.into()))
            .expect("runtime proxy websocket request should be sent");

        let mut payloads = Vec::new();
        loop {
            match socket
                .read()
                .expect("runtime proxy websocket should stay open across chained turns")
            {
                WsMessage::Text(text) => {
                    let text = text.to_string();
                    let done = is_runtime_terminal_event(&text);
                    payloads.push(text);
                    if done {
                        break;
                    }
                }
                WsMessage::Ping(payload) => {
                    socket
                        .send(WsMessage::Pong(payload))
                        .expect("pong should be sent");
                }
                WsMessage::Pong(_) | WsMessage::Frame(_) => {}
                other => panic!("unexpected websocket message: {other:?}"),
            }
        }

        assert!(
            !payloads
                .iter()
                .any(|payload| payload.contains("\"previous_response_not_found\"")),
            "chained websocket continuation should not degrade into previous_response_not_found: {payloads:?}"
        );
        let seen_response_ids = payloads
            .iter()
            .flat_map(|payload| extract_runtime_response_ids_from_payload(payload))
            .collect::<Vec<_>>();
        assert_eq!(
            seen_response_ids.last().map(String::as_str),
            Some(expected_response_id),
            "unexpected websocket continuation chain payloads: {payloads:?}"
        );
        previous_response_id = Some(expected_response_id.to_string());
    }

    assert_eq!(
        backend.websocket_requests().len(),
        3,
        "backend should see each chained websocket continuation request"
    );
}

#[test]
fn runtime_proxy_keeps_multi_turn_previous_response_chain_on_websocket_owner_with_top_level_response_id()
 {
    let backend = RuntimeProxyBackend::start_websocket_top_level_response_id();
    let temp_dir = TestDir::new();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
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
    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");

    let mut previous_response_id: Option<String> = None;
    for expected_response_id in ["resp-second", "resp-second-next", "resp-second-next-next"] {
        let request = previous_response_id
            .as_deref()
            .map(|previous_response_id| {
                format!("{{\"previous_response_id\":\"{previous_response_id}\",\"input\":[]}}")
            })
            .unwrap_or_else(|| "{\"input\":[]}".to_string());
        socket
            .send(WsMessage::Text(request.into()))
            .expect("runtime proxy websocket request should be sent");

        let mut payloads = Vec::new();
        loop {
            match socket
                .read()
                .expect("runtime proxy websocket should stay open across chained turns")
            {
                WsMessage::Text(text) => {
                    let text = text.to_string();
                    let done = is_runtime_terminal_event(&text);
                    payloads.push(text);
                    if done {
                        break;
                    }
                }
                WsMessage::Ping(payload) => {
                    socket
                        .send(WsMessage::Pong(payload))
                        .expect("pong should be sent");
                }
                WsMessage::Pong(_) | WsMessage::Frame(_) => {}
                other => panic!("unexpected websocket message: {other:?}"),
            }
        }

        assert!(
            !payloads
                .iter()
                .any(|payload| payload.contains("\"previous_response_not_found\"")),
            "chained websocket continuation with top-level response_id should not degrade into previous_response_not_found: {payloads:?}"
        );
        let seen_response_ids = payloads
            .iter()
            .flat_map(|payload| extract_runtime_response_ids_from_payload(payload))
            .collect::<Vec<_>>();
        assert_eq!(
            seen_response_ids.last().map(String::as_str),
            Some(expected_response_id),
            "unexpected websocket continuation chain payloads: {payloads:?}"
        );
        previous_response_id = Some(expected_response_id.to_string());
    }

    assert_eq!(
        backend.websocket_requests().len(),
        3,
        "backend should see each chained websocket continuation request"
    );
}

#[test]
fn runtime_proxy_keeps_previous_response_chain_across_multiple_restarts_http() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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
    initial_state
        .save(&paths)
        .expect("failed to save initial state");

    let client = Client::builder().build().expect("client");
    let (mut previous_response_id, owner_profile, owner_account) = {
        let proxy =
            start_runtime_rotation_proxy(&paths, &initial_state, "main", backend.base_url(), false)
                .expect("runtime proxy should start");

        let first = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body("{\"input\":[]}")
            .send()
            .expect("first runtime proxy request should succeed");
        let first_body = first
            .text()
            .expect("first response body should be readable");
        let first_response_id = runtime_proxy_response_ids_from_http_body(&first_body)
            .into_iter()
            .last()
            .expect("first response should expose a response id");
        let owner_profile = runtime_proxy_backend_profile_name_for_response_id(&first_response_id)
            .expect("first response id should map to a backend profile");
        let owner_account = runtime_proxy_backend_account_id_for_profile_name(owner_profile)
            .expect("owner profile should map to a backend account");
        assert!(
            first_body.contains(&format!("\"{first_response_id}\"")),
            "unexpected first continuation body: {first_body}"
        );
        let _ = wait_for_runtime_continuations(&paths, |continuations| {
            continuations
                .response_profile_bindings
                .get(&first_response_id)
                .is_some_and(|binding| binding.profile_name == owner_profile)
        });
        (first_response_id, owner_profile, owner_account)
    };

    for _ in 0..2 {
        let expected_response_id =
            runtime_proxy_backend_next_response_id(Some(&previous_response_id))
                .expect("continuation response id should exist");
        let mut resumed_state = AppState::load(&paths).expect("state should reload");
        resumed_state.active_profile = Some("third".to_string());
        resumed_state
            .save(&paths)
            .expect("failed to save resumed state");

        let proxy = start_runtime_rotation_proxy(
            &paths,
            &resumed_state,
            "third",
            backend.base_url(),
            false,
        )
        .expect("resumed runtime proxy should start");

        let response = client
            .post(format!(
                "http://{}/backend-api/codex/responses",
                proxy.listen_addr
            ))
            .header("Content-Type", "application/json")
            .body(format!(
                "{{\"previous_response_id\":\"{previous_response_id}\",\"input\":[]}}"
            ))
            .send()
            .expect("continued runtime proxy request should succeed");
        let body = response.text().expect("response body should be readable");
        assert!(
            body.contains(&format!("\"{expected_response_id}\"")),
            "unexpected continued body: {body}"
        );
        let _ = wait_for_runtime_continuations(&paths, |continuations| {
            continuations
                .response_profile_bindings
                .get(&expected_response_id)
                .is_some_and(|binding| binding.profile_name == owner_profile)
        });
        previous_response_id = expected_response_id;
    }

    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            owner_account.to_string(),
            owner_account.to_string(),
            owner_account.to_string(),
        ],
        "restarts should keep the continuation chain pinned to the original owner"
    );
}

#[test]
fn runtime_proxy_keeps_previous_response_affinity_for_websocket_requests() {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                },
            ),
        ]),
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

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("first runtime proxy websocket request should be sent");

    let mut first_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                first_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    let first_response_id = first_payloads
        .iter()
        .flat_map(|payload| extract_runtime_response_ids_from_payload(payload))
        .last()
        .expect("first websocket response should expose a response id");
    let owner_account = runtime_proxy_backend_response_owner_account_id(&first_response_id)
        .expect("first websocket response id should map to a backend account");
    let second_response_id = runtime_proxy_backend_next_response_id(Some(&first_response_id))
        .expect("continuation response id should exist");
    assert!(
        first_payloads
            .iter()
            .any(|payload| payload.contains(&format!("\"{first_response_id}\""))),
        "unexpected initial websocket payloads: {first_payloads:?}"
    );

    socket
        .send(WsMessage::Text(
            format!("{{\"previous_response_id\":\"{first_response_id}\",\"input\":[]}}").into(),
        ))
        .expect("second runtime proxy websocket request should be sent");

    let mut second_payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                second_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
    assert!(
        second_payloads
            .iter()
            .any(|payload| payload.contains(&format!("\"{second_response_id}\""))),
        "unexpected continued websocket payloads: {second_payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), owner_account.to_string()]
    );
}

#[test]
fn runtime_proxy_releases_previous_response_affinity_for_websocket_requests_when_owner_snapshot_is_exhausted()
{
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 1,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("websocket".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to save continuation sidecar");
    save_runtime_usage_snapshots(
        &paths,
        &BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 90,
                weekly_reset_at: now + 86_400,
            },
        )]),
    )
    .expect("failed to save runtime usage snapshots");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    set_test_websocket_io_timeout(&mut socket, Duration::from_secs(5));
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-main\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => break,
            Err(err) => {
                panic!(
                    "runtime proxy websocket failed while waiting for exhausted-owner continuation payloads: {err}; payloads={payloads:?}"
                );
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\"")),
        "unexpected websocket payloads for exhausted owner fresh fallback: {payloads:?}"
    );
    assert!(
        !payloads
            .iter()
            .any(|payload| payload.contains("\"previous_response_not_found\"")),
        "previous_response fallback should stay pre-commit: {payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        !state.response_profile_bindings.contains_key("resp-main")
            && state
                .response_profile_bindings
                .get("resp-second")
                .is_some_and(|binding| binding.profile_name == "second")
    });
    assert!(!persisted.response_profile_bindings.contains_key("resp-main"));
    assert_eq!(
        persisted
            .response_profile_bindings
            .get("resp-second")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );

    let continuations = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .statuses
            .response
            .get("resp-main")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
    });
    assert_eq!(
        continuations
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_proxy_releases_previous_response_affinity_for_websocket_requests_when_owner_snapshot_hits_critical_floor()
{
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-main".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 1,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("websocket".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to save continuation sidecar");
    save_runtime_usage_snapshots(
        &paths,
        &BTreeMap::from([(
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: now,
                five_hour_status: RuntimeQuotaWindowStatus::Critical,
                five_hour_remaining_percent: 1,
                five_hour_reset_at: now + 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 80,
                weekly_reset_at: now + 86_400,
            },
        )]),
    )
    .expect("failed to save runtime usage snapshots");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    set_test_websocket_io_timeout(&mut socket, Duration::from_secs(5));
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-main\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => break,
            Err(err) => {
                panic!(
                    "runtime proxy websocket failed while waiting for critical-floor fallback payloads: {err}; payloads={payloads:?}"
                );
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\"")),
        "unexpected websocket payloads for critical-floor fallback: {payloads:?}"
    );
    assert!(
        !payloads
            .iter()
            .any(|payload| payload.contains("You've hit your usage limit")),
        "critical-floor owner should be blocked before surfacing usage limit: {payloads:?}"
    );
    assert!(
        !payloads
            .iter()
            .any(|payload| payload.contains("\"previous_response_not_found\"")),
        "previous_response fallback should stay pre-commit: {payloads:?}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
}

#[test]
fn runtime_proxy_stale_critical_floor_snapshot_skips_current_profile_on_fresh_websocket_requests(
) {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");
    save_runtime_usage_snapshots(
        &paths,
        &BTreeMap::from([(
            "main".to_string(),
            stale_critical_runtime_usage_snapshot(now),
        )]),
    )
    .expect("failed to seed stale runtime usage snapshots");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    wait_for_runtime_background_queues_idle();
    let before_request = backend.responses_accounts().len();

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("runtime proxy websocket handshake should succeed");
    set_test_websocket_io_timeout(&mut socket, Duration::from_secs(5));
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => break,
            Err(err) => {
                panic!(
                    "runtime proxy websocket failed while waiting for stale-snapshot fallback payloads: {err}; payloads={payloads:?}"
                );
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
        }
    }

    let request_accounts = backend.responses_accounts()[before_request..].to_vec();
    assert!(
        !request_accounts.is_empty(),
        "stale critical-floor snapshot should still make a fresh websocket request"
    );
    assert_eq!(
        request_accounts.last().map(String::as_str),
        Some("second-account"),
        "stale critical-floor snapshot should finish on the fallback websocket profile: {request_accounts:?}"
    );
    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\"")),
        "stale critical-floor snapshot should still complete on the fallback profile: {payloads:?}"
    );
    assert!(
        !payloads
            .iter()
            .any(|payload| payload.contains("You've hit your usage limit")),
        "stale critical-floor snapshot should not surface usage limit: {payloads:?}"
    );
}

#[test]
fn runtime_proxy_quarantines_usage_limited_profile_across_restart_for_fresh_websocket_requests(
) {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                },
            ),
        ]),
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
    state.save(&paths).expect("failed to save initial state");

    {
        let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
            .expect("runtime proxy should start");
        wait_for_runtime_background_queues_idle();
        let before_first_request = backend.responses_accounts().len();

        let (mut socket, _response) = ws_connect(format!(
            "ws://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .expect("runtime proxy websocket handshake should succeed");
        set_test_websocket_io_timeout(&mut socket, Duration::from_secs(5));
        socket
            .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
            .expect("first runtime proxy websocket request should be sent");

        let mut first_payloads = Vec::new();
        loop {
            match socket.read() {
                Ok(WsMessage::Text(text)) => {
                    let text = text.to_string();
                    let done = is_runtime_terminal_event(&text);
                    first_payloads.push(text);
                    if done {
                        break;
                    }
                }
                Ok(WsMessage::Ping(payload)) => {
                    socket
                        .send(WsMessage::Pong(payload))
                        .expect("pong should be sent");
                }
                Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
                Ok(WsMessage::Close(_))
                | Err(WsError::ConnectionClosed)
                | Err(WsError::AlreadyClosed) => break,
                Err(err) => {
                    panic!(
                        "runtime proxy websocket failed while waiting for initial quota-limit payloads: {err}; payloads={first_payloads:?}"
                    );
                }
                Ok(other) => panic!("unexpected websocket message: {other:?}"),
            }
        }

        let first_request_accounts = backend.responses_accounts()[before_first_request..].to_vec();
        assert_eq!(
            first_request_accounts.last().map(String::as_str),
            Some("second-account"),
            "initial quota-limited websocket request should finish on the fallback profile: {first_request_accounts:?}"
        );
        assert!(
            !first_payloads
                .iter()
                .any(|payload| payload.contains("You've hit your usage limit")),
            "quota-limited websocket request should not surface usage limit: {first_payloads:?}"
        );
    }

    state.save(&paths).expect("failed to restore active profile");
    wait_for_runtime_background_queues_idle();
    let before_second_request = backend.responses_accounts().len();

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("replacement runtime proxy should start");
    wait_for_runtime_background_queues_idle();

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("replacement runtime proxy websocket handshake should succeed");
    set_test_websocket_io_timeout(&mut socket, Duration::from_secs(5));
    socket
        .send(WsMessage::Text("{\"input\":[]}".to_string().into()))
        .expect("replacement runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(_))
            | Err(WsError::ConnectionClosed)
            | Err(WsError::AlreadyClosed) => break,
            Err(err) => {
                panic!(
                    "replacement runtime proxy websocket failed while waiting for payloads: {err}; payloads={payloads:?}"
                );
            }
            Ok(other) => panic!("unexpected websocket message: {other:?}"),
        }
    }

    let second_request_accounts = backend.responses_accounts()[before_second_request..].to_vec();
    assert_eq!(
        second_request_accounts,
        vec!["second-account".to_string()],
        "quarantined profile should not be reused for a follow-up websocket request: {second_request_accounts:?}"
    );
    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\"")),
        "quarantined profile should still complete on the fallback websocket profile: {payloads:?}"
    );
    assert!(
        !payloads
            .iter()
            .any(|payload| payload.contains("You've hit your usage limit")),
        "quarantined profile should not surface usage limit after restart: {payloads:?}"
    );
}

#[test]
fn runtime_proxy_allows_only_one_persistence_owner_per_state_root() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let owner = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("first runtime proxy should start");
    let follower = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("second runtime proxy should start");

    assert!(runtime_proxy_persistence_enabled_for_log_path(
        &owner.log_path
    ));
    assert!(!runtime_proxy_persistence_enabled_for_log_path(
        &follower.log_path
    ));

    drop(owner);

    let promoted = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("replacement runtime proxy should start");
    assert!(runtime_proxy_persistence_enabled_for_log_path(
        &promoted.log_path
    ));
}

#[test]
fn runtime_proxy_follower_keeps_persistence_side_effects_in_memory_only() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let _owner = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("owner runtime proxy should start");
    let follower = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("follower runtime proxy should start");
    assert!(!runtime_proxy_persistence_enabled_for_log_path(
        &follower.log_path
    ));

    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            follower.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("follower runtime proxy request should succeed");
    assert!(response.status().is_success());

    let state_after = AppState::load(&paths).expect("state should reload");
    assert!(state_after.response_profile_bindings.is_empty());

    let continuations = load_runtime_continuations_with_recovery(&paths, &state_after.profiles)
        .expect("continuations should reload")
        .value;
    assert!(continuations.response_profile_bindings.is_empty());
}

#[test]
fn runtime_proxy_owner_still_persists_bindings_when_follower_exists() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let owner = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("owner runtime proxy should start");
    let _follower = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("follower runtime proxy should start");

    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            owner.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"input\":[]}")
        .send()
        .expect("owner runtime proxy request should succeed");
    assert!(response.status().is_success());

    let state_after = wait_for_state(&paths, |state| {
        state.response_profile_bindings.contains_key("resp-second")
    });
    assert_eq!(
        state_after
            .response_profile_bindings
            .get("resp-second")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
}

#[test]
fn runtime_proxy_preserves_websocket_headers_and_payload_metadata() {
    let backend = RuntimeProxyBackend::start_websocket();
    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");

    let mut request = tungstenite::client::IntoClientRequest::into_client_request(format!(
        "ws://{}/backend-api/codex/responses",
        proxy.listen_addr
    ))
    .expect("client request should build");
    request
        .headers_mut()
        .insert("session_id", "sess-ws-123".parse().expect("session header"));
    request.headers_mut().insert(
        "x-openai-subagent",
        "compact-remote".parse().expect("subagent header"),
    );
    request.headers_mut().insert(
        "x-codex-turn-metadata",
        r#"{"source":"resume","session_id":"sess-ws-123"}"#
            .parse()
            .expect("turn metadata header"),
    );
    request.headers_mut().insert(
        "x-codex-beta-features",
        "remote-sync,realtime".parse().expect("beta header"),
    );
    request.headers_mut().insert(
        "User-Agent",
        "codex-cli/0.117.0".parse().expect("ua header"),
    );

    let (mut socket, _response) =
        tungstenite::connect(request).expect("runtime proxy websocket handshake should succeed");
    let request_payload = serde_json::json!({
        "input": [],
        "client_metadata": {
            "traceparent": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            "session_id": "sess-ws-123",
            "turn_id": "turn-xyz"
        }
    })
    .to_string();
    socket
        .send(WsMessage::Text(request_payload.clone().into()))
        .expect("runtime proxy websocket request should be sent");

    let mut payloads = Vec::new();
    loop {
        match socket
            .read()
            .expect("runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }

    assert!(
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-second\"")),
        "unexpected websocket payloads: {payloads:?}"
    );

    let headers = backend.responses_headers();
    let first = headers
        .first()
        .expect("backend should capture websocket handshake headers");
    assert_eq!(
        first.get("session_id").map(String::as_str),
        Some("sess-ws-123")
    );
    assert_eq!(
        first.get("x-openai-subagent").map(String::as_str),
        Some("compact-remote")
    );
    assert_eq!(
        first.get("x-codex-turn-metadata").map(String::as_str),
        Some(r#"{"source":"resume","session_id":"sess-ws-123"}"#)
    );
    assert_eq!(
        first.get("x-codex-beta-features").map(String::as_str),
        Some("remote-sync,realtime")
    );
    assert_eq!(
        first.get("user-agent").map(String::as_str),
        Some("codex-cli/0.117.0")
    );
    assert_eq!(
        first.get("chatgpt-account-id").map(String::as_str),
        Some("second-account")
    );

    let upstream_requests = backend.websocket_requests();
    assert_eq!(
        upstream_requests.last().map(String::as_str),
        Some(request_payload.as_str())
    );
}

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
    assert!(metrics.active_request_limit > 0);
    assert!(metrics.traffic.responses.limit > 0);
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

#[test]
fn runtime_broker_openai_mount_path_falls_back_to_running_legacy_broker_version() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TestDir::new();
    let script_path = temp_dir.path.join("legacy-prodex.sh");
    fs::write(
        &script_path,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'prodex 0.2.99'\n  exit 0\nfi\nsleep 30\n",
    )
    .expect("legacy broker script should write");
    let mut permissions = fs::metadata(&script_path)
        .expect("legacy broker script metadata should load")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions)
        .expect("legacy broker script permissions should update");

    let mut child = Command::new(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("legacy broker script should spawn");

    for _ in 0..20 {
        if collect_process_rows()
            .into_iter()
            .any(|row| row.pid == child.id())
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let registry = RuntimeBrokerRegistry {
        pid: child.id(),
        listen_addr: "127.0.0.1:9".to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        instance_token: "instance".to_string(),
        admin_token: "secret".to_string(),
        openai_mount_path: None,
    };

    let mount_path = runtime_broker_openai_mount_path(&registry)
        .expect("legacy broker mount path should resolve from running broker version");

    let _ = child.kill();
    let _ = child.wait();

    assert_eq!(mount_path, "/backend-api/prodex/v0.2.99");
}

#[test]
fn find_compatible_runtime_broker_registry_discovers_other_broker_key() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let server = TinyServer::http("127.0.0.1:0").expect("health server should bind");
    let listen_addr = server
        .server_addr()
        .to_ip()
        .expect("health server should expose a TCP address");
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        listen_addr: listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        instance_token: "legacy-instance".to_string(),
        admin_token: "secret".to_string(),
        openai_mount_path: Some("/backend-api/prodex/v0.2.99".to_string()),
    };
    save_runtime_broker_registry(&paths, "legacy-key", &registry)
        .expect("legacy broker registry should save");

    let health_thread = thread::spawn(move || {
        let request = server.recv().expect("health request should arrive");
        let body = serde_json::to_string(&RuntimeBrokerHealth {
            pid: std::process::id(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            active_requests: 0,
            instance_token: "legacy-instance".to_string(),
            persistence_role: "owner".to_string(),
        })
        .expect("health payload should serialize");
        let response = TinyResponse::from_string(body).with_status_code(200);
        request
            .respond(response)
            .expect("health response should write");
    });
    let client = runtime_broker_client().expect("broker client should build");

    let discovered = find_compatible_runtime_broker_registry(
        &client,
        &paths,
        "current-key",
        "https://chatgpt.com/backend-api",
        false,
    )
    .expect("registry scan should not fail")
    .expect("compatible registry should be discovered");

    health_thread
        .join()
        .expect("health server thread should join");
    assert_eq!(discovered.0, "legacy-key");
    assert_eq!(discovered.1.instance_token, "legacy-instance");
}

#[test]
fn wait_for_existing_runtime_broker_recovery_or_exit_yields_after_live_unhealthy_registry_clears() {
    let _timeout_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "500");
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "wait-test";
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        listen_addr: "127.0.0.1:9".to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "http://127.0.0.1:12345/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        instance_token: "instance".to_string(),
        admin_token: "secret".to_string(),
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };
    save_runtime_broker_registry(&paths, broker_key, &registry)
        .expect("registry should save for wait test");

    let paths_for_clear = paths.clone();
    let instance_token = registry.instance_token.clone();
    let upstream_base_url = registry.upstream_base_url.clone();
    let include_code_review = registry.include_code_review;
    let clear_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(75));
        remove_runtime_broker_registry_if_token_matches(
            &paths_for_clear,
            broker_key,
            &instance_token,
        );
    });
    let client = runtime_broker_client().expect("broker client should build");

    let recovered = wait_for_existing_runtime_broker_recovery_or_exit(
        &client,
        &paths,
        broker_key,
        &upstream_base_url,
        include_code_review,
    )
    .expect("wait should not fail");

    clear_thread
        .join()
        .expect("registry clear thread should join");
    assert!(
        recovered.is_none(),
        "wait should yield once the live unhealthy registry clears"
    );
}

#[test]
fn runtime_broker_startup_grace_covers_ready_timeout() {
    let _timeout_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "15000");
    assert!(runtime_broker_startup_grace_seconds() >= 16);
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
fn runtime_proxy_lane_classifies_anthropic_messages_as_responses() {
    assert_eq!(
        runtime_proxy_request_lane("/v1/messages?beta=true", false),
        RuntimeRouteKind::Responses
    );
}

#[test]
fn runtime_proxy_claude_launch_env_uses_foundry_compat_with_profile_config_dir() {
    let temp_dir = TestDir::new();
    let _model_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_MODEL");
    let config_dir = temp_dir.path.join("claude-config");
    let codex_home = temp_dir.path.join("codex-home");
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43123"
            .parse()
            .expect("listen address should parse"),
        &config_dir,
        &codex_home,
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "CLAUDE_CONFIG_DIR")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some(config_dir.to_string_lossy().into_owned())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_BASE_URL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("http://127.0.0.1:43123".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_AUTH_TOKEN")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some(PRODEX_CLAUDE_PROXY_API_KEY.to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "CLAUDE_CODE_USE_FOUNDRY")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("1".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_FOUNDRY_BASE_URL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("http://127.0.0.1:43123".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_FOUNDRY_API_KEY")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some(PRODEX_CLAUDE_PROXY_API_KEY.to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("gpt-5".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_DEFAULT_OPUS_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("gpt-5.4".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("effort,max_effort,thinking,adaptive_thinking,interleaved_thinking".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_DEFAULT_SONNET_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("gpt-5.3-codex".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_DEFAULT_HAIKU_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("gpt-5.4-mini".to_string())
    );
    assert!(
        env.iter()
            .all(|(key, _)| *key != "ANTHROPIC_CUSTOM_MODEL_OPTION")
    );
    assert!(env.iter().all(|(key, _)| *key != "ANTHROPIC_API_KEY"));
    assert_eq!(
        runtime_proxy_claude_removed_env(),
        [
            "ANTHROPIC_API_KEY",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_USE_VERTEX",
            "CLAUDE_CODE_USE_FOUNDRY",
            "CLAUDE_CODE_USE_ANTHROPIC_AWS",
            "ANTHROPIC_BEDROCK_BASE_URL",
            "ANTHROPIC_VERTEX_BASE_URL",
            "ANTHROPIC_FOUNDRY_BASE_URL",
            "ANTHROPIC_AWS_BASE_URL",
            "ANTHROPIC_FOUNDRY_RESOURCE",
            "ANTHROPIC_VERTEX_PROJECT_ID",
            "ANTHROPIC_AWS_WORKSPACE_ID",
            "CLOUD_ML_REGION",
            "ANTHROPIC_FOUNDRY_API_KEY",
            "ANTHROPIC_AWS_API_KEY",
            "CLAUDE_CODE_SKIP_BEDROCK_AUTH",
            "CLAUDE_CODE_SKIP_VERTEX_AUTH",
            "CLAUDE_CODE_SKIP_FOUNDRY_AUTH",
            "CLAUDE_CODE_SKIP_ANTHROPIC_AWS_AUTH",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES",
            "ANTHROPIC_CUSTOM_MODEL_OPTION",
            "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME",
            "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION"
        ]
    );
}

#[test]
fn runtime_proxy_claude_launch_env_honors_model_override() {
    let _model_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_MODEL", "gpt-5-mini");
    let temp_dir = TestDir::new();
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &temp_dir.path.join("codex-home"),
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("gpt-5-mini".to_string())
    );
    assert!(
        env.iter()
            .all(|(key, _)| *key != "ANTHROPIC_CUSTOM_MODEL_OPTION")
    );
}

#[test]
fn runtime_proxy_claude_launch_env_keeps_custom_picker_entry_for_unknown_override() {
    let _model_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_MODEL", "my-gateway-model");
    let temp_dir = TestDir::new();
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &temp_dir.path.join("codex-home"),
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("my-gateway-model".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_CUSTOM_MODEL_OPTION")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("my-gateway-model".to_string())
    );
}

#[test]
fn runtime_proxy_claude_launch_env_uses_codex_config_model_by_default() {
    let _model_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_MODEL");
    let temp_dir = TestDir::new();
    let codex_home = temp_dir.path.join("codex-home");
    fs::create_dir_all(&codex_home).expect("codex home should exist");
    fs::write(codex_home.join("config.toml"), "model = \"gpt-5.4\"\n")
        .expect("config should write");
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &codex_home,
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("opus".to_string())
    );
    assert!(
        env.iter()
            .all(|(key, _)| *key != "ANTHROPIC_CUSTOM_MODEL_OPTION")
    );
}

#[test]
fn runtime_proxy_claude_launch_env_maps_alias_backed_override_to_builtin_picker_value() {
    let _model_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_MODEL", "gpt-5.4");
    let temp_dir = TestDir::new();
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &temp_dir.path.join("codex-home"),
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("opus".to_string())
    );
}

#[test]
fn runtime_proxy_claude_target_model_maps_builtin_aliases_to_pinned_gpt_models() {
    assert_eq!(
        runtime_proxy_claude_target_model("opus"),
        "gpt-5.4".to_string()
    );
    assert_eq!(
        runtime_proxy_claude_target_model("sonnet"),
        "gpt-5.3-codex".to_string()
    );
    assert_eq!(
        runtime_proxy_claude_target_model("claude-sonnet-4-6"),
        "gpt-5.3-codex".to_string()
    );
    assert_eq!(
        runtime_proxy_claude_target_model("haiku"),
        "gpt-5.4-mini".to_string()
    );
    assert_eq!(
        runtime_proxy_claude_target_model("claude-opus-4-5"),
        "gpt-5.2".to_string()
    );
    assert_eq!(
        runtime_proxy_claude_target_model("claude-haiku-3-5"),
        "gpt-5-mini".to_string()
    );
}

#[test]
fn runtime_proxy_claude_reasoning_effort_override_normalizes_env() {
    let _effort_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_REASONING_EFFORT", "max");
    assert_eq!(
        runtime_proxy_claude_reasoning_effort_override(),
        Some("xhigh".to_string())
    );
}

#[test]
fn runtime_proxy_claude_reasoning_effort_override_ignores_invalid_env() {
    let _effort_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_REASONING_EFFORT", "ultra");
    assert_eq!(runtime_proxy_claude_reasoning_effort_override(), None);
}

#[test]
fn parse_runtime_proxy_claude_version_text_extracts_semver_prefix() {
    assert_eq!(
        parse_runtime_proxy_claude_version_text("2.1.90 (Claude Code)"),
        Some("2.1.90".to_string())
    );
    assert_eq!(
        parse_runtime_proxy_claude_version_text("Claude Code 2.1.90"),
        Some("2.1.90".to_string())
    );
    assert_eq!(parse_runtime_proxy_claude_version_text("Claude Code"), None);
}

#[test]
fn ensure_runtime_proxy_claude_launch_config_seeds_onboarding_and_project_trust() {
    let temp_dir = TestDir::new();
    let config_dir = temp_dir.path.join("claude-config");
    let cwd = temp_dir.path.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace dir should exist");

    ensure_runtime_proxy_claude_launch_config(&config_dir, &cwd, Some("2.1.90"))
        .expect("Claude config seed should succeed");

    let config: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(config_dir.join(".claude.json"))
            .expect("Claude config should be written"),
    )
    .expect("Claude config should be valid JSON");
    assert_eq!(config["numStartups"], serde_json::json!(1));
    assert_eq!(config["hasCompletedOnboarding"], serde_json::json!(true));
    assert_eq!(config["lastOnboardingVersion"], serde_json::json!("2.1.90"));
    let additional_model_options = config["additionalModelOptionsCache"]
        .as_array()
        .expect("additional model options cache should be an array");
    assert_eq!(additional_model_options.len(), 6);
    assert!(!additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("gpt-5.4")
    }));
    assert!(additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("gpt-5.2-codex")
            && entry.get("label").and_then(serde_json::Value::as_str) == Some("gpt-5.2-codex")
    }));

    let project_key = cwd.to_string_lossy().into_owned();
    let project = config["projects"]
        .get(project_key.as_str())
        .expect("seeded project entry should exist");
    assert_eq!(project["hasTrustDialogAccepted"], serde_json::json!(true));
    assert_eq!(project["projectOnboardingSeenCount"], serde_json::json!(1));
    assert!(project["allowedTools"].is_array());
    assert!(project["mcpContextUris"].is_array());
    assert!(project["mcpServers"].is_object());
    assert!(project["enabledMcpjsonServers"].is_array());
    assert!(project["disabledMcpjsonServers"].is_array());
    assert!(project["exampleFiles"].is_array());
}

#[test]
fn ensure_runtime_proxy_claude_launch_config_preserves_existing_entries() {
    let temp_dir = TestDir::new();
    let config_dir = temp_dir.path.join("claude-config");
    let cwd = temp_dir.path.join("workspace");
    let other_project = temp_dir.path.join("other");
    fs::create_dir_all(&cwd).expect("workspace dir should exist");
    fs::create_dir_all(&config_dir).expect("config dir should exist");
    fs::write(
        config_dir.join(".claude.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "numStartups": 7,
            "customField": "keep-me",
            "additionalModelOptionsCache": [
                {
                    "value": "claude-opus-4-6",
                    "label": "gpt-5.4",
                    "description": "Old managed model entry"
                },
                {
                    "value": "custom-provider/model",
                    "label": "custom-provider/model",
                    "description": "Existing custom model"
                }
            ],
            "projects": {
                other_project.to_string_lossy().to_string(): {
                    "hasTrustDialogAccepted": false,
                    "allowedTools": ["Bash"]
                }
            }
        }))
        .expect("existing config should serialize"),
    )
    .expect("existing config should write");

    ensure_runtime_proxy_claude_launch_config(&config_dir, &cwd, Some("2.1.90"))
        .expect("Claude config merge should succeed");

    let config: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(config_dir.join(".claude.json"))
            .expect("Claude config should still exist"),
    )
    .expect("merged Claude config should be valid JSON");
    assert_eq!(config["numStartups"], serde_json::json!(7));
    assert_eq!(config["customField"], serde_json::json!("keep-me"));
    let additional_model_options = config["additionalModelOptionsCache"]
        .as_array()
        .expect("additional model options cache should be preserved as an array");
    assert!(additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("custom-provider/model")
    }));
    assert!(additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("gpt-5.2")
            && entry.get("label").and_then(serde_json::Value::as_str) == Some("gpt-5.2")
            && entry.get("supportedEffortLevels")
                == Some(&serde_json::json!(["low", "medium", "high", "max"]))
    }));
    assert!(!additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("gpt-5.4")
    }));
    assert!(!additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("claude-opus-4-6")
    }));
    let other_project_key = other_project.to_string_lossy().to_string();
    let cwd_key = cwd.to_string_lossy().to_string();
    assert_eq!(
        config["projects"][other_project_key.as_str()]["allowedTools"],
        serde_json::json!(["Bash"])
    );
    assert_eq!(
        config["projects"][cwd_key.as_str()]["hasTrustDialogAccepted"],
        serde_json::json!(true)
    );
}

#[test]
fn runtime_proxy_serves_local_anthropic_compat_metadata_routes() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "compat-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let root: serde_json::Value = client
        .get(format!("http://{}/", proxy.listen_addr))
        .send()
        .expect("root request should succeed")
        .json()
        .expect("root response should parse");
    assert_eq!(
        root.get("service").and_then(serde_json::Value::as_str),
        Some("prodex")
    );
    assert_eq!(
        root.get("status").and_then(serde_json::Value::as_str),
        Some("ok")
    );

    let health: serde_json::Value = client
        .get(format!("http://{}/health", proxy.listen_addr))
        .send()
        .expect("health request should succeed")
        .json()
        .expect("health response should parse");
    assert_eq!(
        health.get("status").and_then(serde_json::Value::as_str),
        Some("ok")
    );
    let head = client
        .head(format!("http://{}/", proxy.listen_addr))
        .send()
        .expect("root HEAD request should succeed");
    assert!(
        head.status().is_success(),
        "unexpected HEAD status: {}",
        head.status()
    );

    let models: serde_json::Value = client
        .get(format!("http://{}/v1/models?beta=true", proxy.listen_addr))
        .send()
        .expect("models request should succeed")
        .json()
        .expect("models response should parse");
    let data = models
        .get("data")
        .and_then(serde_json::Value::as_array)
        .expect("models data should be an array");
    assert_eq!(
        data.len(),
        9,
        "metadata route should expose the prodex Claude OpenAI model catalog"
    );
    assert!(data.iter().all(|model| {
        !model
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .starts_with("claude-")
    }));
    assert!(
        data.iter().any(|model| {
            model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5.4")
        })
    );
    assert!(
        data.iter().any(|model| {
            model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5.2")
        })
    );
    assert!(
        data.iter()
            .any(|model| { model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5") })
    );
    assert!(data.iter().any(|model| {
        model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5-mini")
    }));

    let model: serde_json::Value = client
        .get(format!(
            "http://{}/v1/models/gpt-5?beta=true",
            proxy.listen_addr
        ))
        .send()
        .expect("model request should succeed")
        .json()
        .expect("model response should parse");
    assert_eq!(
        model.get("id").and_then(serde_json::Value::as_str),
        Some("gpt-5")
    );
    assert_eq!(
        model
            .get("display_name")
            .and_then(serde_json::Value::as_str),
        Some("GPT-5")
    );

    assert!(
        backend.responses_headers().is_empty(),
        "local compatibility routes should not hit the upstream backend"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_tools_and_tool_results() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![
            ("User-Agent".to_string(), "claude-cli/test".to_string()),
            (
                "X-Claude-Code-Session-Id".to_string(),
                "claude-session-123".to_string(),
            ),
        ],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "stream": true,
            "thinking": {
                "type": "adaptive"
            },
            "system": [
                {
                    "type": "text",
                    "text": "System instructions",
                }
            ],
            "output_config": {
                "effort": "medium",
            },
            "tools": [
                {
                    "name": "shell",
                    "description": "Run a shell command",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "tool_choice": {
                "type": "any"
            },
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "Show the logs",
                        },
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "YWJj",
                            }
                        }
                    ]
                },
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "text",
                            "text": "Calling shell",
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_1",
                            "name": "shell",
                            "input": {
                                "cmd": "ls"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_1",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "file1",
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    assert_eq!(
        translated.translated_request.path_and_query,
        "/backend-api/codex/responses"
    );
    assert_eq!(translated.requested_model, "claude-sonnet-4-6");
    assert!(translated.stream);
    assert!(translated.want_thinking);
    assert_eq!(
        runtime_proxy_request_header_value(&translated.translated_request.headers, "session_id"),
        Some("claude-session-123")
    );

    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("model").and_then(serde_json::Value::as_str),
        Some("gpt-5.3-codex")
    );
    assert_eq!(
        body.get("instructions").and_then(serde_json::Value::as_str),
        Some("System instructions")
    );
    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );
    assert!(
        body.get("max_tokens").is_none(),
        "translated request should not send unsupported max_tokens"
    );
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("medium")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("parameters"))
            .and_then(|parameters| parameters.get("properties"))
            .and_then(serde_json::Value::as_object)
            .map(serde_json::Map::len),
        Some(0)
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(input.len(), 4);
    assert_eq!(
        input[0].get("role").and_then(serde_json::Value::as_str),
        Some("user")
    );
    assert_eq!(
        input[0]
            .get("content")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        input[1].get("role").and_then(serde_json::Value::as_str),
        Some("assistant")
    );
    assert_eq!(
        input[1].get("content").and_then(serde_json::Value::as_str),
        Some("Calling shell")
    );
    assert_eq!(
        input[2].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[2].get("call_id").and_then(serde_json::Value::as_str),
        Some("toolu_1")
    );
    assert_eq!(
        input[3].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );
    assert_eq!(
        input[3].get("output").and_then(serde_json::Value::as_str),
        Some("file1")
    );
}

#[test]
fn runtime_proxy_anthropic_reasoning_effort_normalizes_output_config_levels() {
    let cases = [
        ("gpt-5.4", "low", Some("low")),
        ("gpt-5.4", "medium", Some("medium")),
        ("gpt-5.4", "high", Some("high")),
        ("gpt-5.4", "max", Some("xhigh")),
        ("gpt-5", "max", Some("high")),
        ("gpt-5.4", "HIGH", Some("high")),
        ("gpt-5.4", "unknown", None),
    ];

    for (target_model, input_effort, expected) in cases {
        let value = serde_json::json!({
            "output_config": {
                "effort": input_effort,
            }
        });
        assert_eq!(
            runtime_proxy_anthropic_reasoning_effort(&value, target_model).as_deref(),
            expected,
            "input effort {input_effort:?} normalized incorrectly for target model {target_model}"
        );
    }
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_max_effort_to_xhigh_for_supported_model() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.4",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "max",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("xhigh")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_keeps_max_effort_at_high_for_legacy_model() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "max",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("high")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_honors_reasoning_override_env() {
    let _effort_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_REASONING_EFFORT", "xhigh");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.2",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "low",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("xhigh")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_tool_use_and_usage() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7,
                "input_tokens_details": {
                    "cached_tokens": 3
                }
            },
            "output": [
                {
                    "type": "reasoning",
                    "summary": [
                        {
                            "type": "summary_text",
                            "text": "Plan first",
                        }
                    ]
                },
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hello from prodex",
                        }
                    ]
                },
                {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "shell",
                    "arguments": "{\"cmd\":\"ls\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        true,
    );

    assert_eq!(
        response.get("type").and_then(serde_json::Value::as_str),
        Some("message")
    );
    assert_eq!(
        response.get("model").and_then(serde_json::Value::as_str),
        Some("claude-sonnet-4-6")
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("thinking")
    );
    assert_eq!(
        content[1].get("text").and_then(serde_json::Value::as_str),
        Some("Hello from prodex")
    );
    assert_eq!(
        content[2].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[2].get("id").and_then(serde_json::Value::as_str),
        Some("call_123")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("cache_read_input_tokens"))
            .and_then(serde_json::Value::as_u64),
        Some(3)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_collects_deltas_and_tool_use() {
    let body = concat!(
        "event: response.reasoning_summary_text.delta\r\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"Plan.\"}\r\n",
        "\r\n",
        "event: response.output_text.delta\r\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\r\n",
        "\r\n",
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"shell\"}}\r\n",
        "\r\n",
        "event: response.function_call_arguments.delta\r\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"call_id\":\"call_1\",\"delta\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"shell\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4}}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", true)
        .expect("SSE translation should succeed");
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("thinking")
    );
    assert_eq!(
        content[1].get("text").and_then(serde_json::Value::as_str),
        Some("Hello")
    );
    assert_eq!(
        content[2].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(serde_json::Value::as_u64),
        Some(9)
    );
}

#[test]
fn runtime_proxy_translates_anthropic_messages_to_responses_and_back() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/v1/messages?beta=true",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .header("x-claude-code-session-id", "claude-session-42")
        .header("User-Agent", "claude-cli/test")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "max_tokens": 256,
                "messages": [
                    {
                        "role": "user",
                        "content": "hello from Claude"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("anthropic proxy request should succeed");

    assert!(
        response.status().is_success(),
        "unexpected status: {}",
        response.status()
    );
    let body: serde_json::Value = response.json().expect("anthropic response should parse");
    assert_eq!(
        body.get("type").and_then(serde_json::Value::as_str),
        Some("message")
    );
    assert_eq!(
        body.get("model").and_then(serde_json::Value::as_str),
        Some("claude-sonnet-4-6")
    );

    let headers = backend.responses_headers();
    let request_headers = headers
        .first()
        .expect("backend should capture translated request headers");
    assert_eq!(
        request_headers.get("session_id").map(String::as_str),
        Some("claude-session-42")
    );
    assert_eq!(
        request_headers.get("user-agent").map(String::as_str),
        Some("claude-cli/test")
    );
    assert_eq!(
        request_headers
            .get("chatgpt-account-id")
            .map(String::as_str),
        Some("second-account")
    );

    let translated_body = backend
        .responses_bodies()
        .into_iter()
        .next()
        .expect("backend should capture translated request body");
    let translated_json: serde_json::Value =
        serde_json::from_str(&translated_body).expect("translated request body should parse");
    assert_eq!(
        translated_json
            .get("model")
            .and_then(serde_json::Value::as_str),
        Some("gpt-5.3-codex")
    );
    assert_eq!(
        translated_json
            .get("input")
            .and_then(serde_json::Value::as_array)
            .and_then(|input| input.first())
            .and_then(|item| item.get("content"))
            .and_then(serde_json::Value::as_str),
        Some("hello from Claude")
    );
}

#[test]
fn runtime_proxy_streams_anthropic_messages_from_buffered_responses() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "stream-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/v1/messages?beta=true",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .header("User-Agent", "claude-cli/test")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "stream": true,
                "messages": [
                    {
                        "role": "user",
                        "content": "hello from Claude"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("anthropic proxy request should succeed");

    assert!(
        response.status().is_success(),
        "unexpected status: {}",
        response.status()
    );
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let body = response.text().expect("stream body should decode");
    assert!(body.contains("event: message_start"));
    assert!(body.contains("event: message_stop"));
}

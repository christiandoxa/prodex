use super::*;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
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

fn runtime_proxy_backend_next_second_response_id(
    previous_response_id: Option<&str>,
) -> Option<String> {
    match previous_response_id {
        Some("resp-second") => Some("resp-second-next".to_string()),
        Some(previous_response_id) if previous_response_id.starts_with("resp-second-next") => {
            Some(format!("{previous_response_id}-next"))
        }
        _ => None,
    }
}

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
        wait_for_runtime_background_queues_idle();
        let unique = format!(
            "prodex-runtime-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("failed to create test temp dir");
        Self { path }
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
            return;
        }
        thread::sleep(Duration::from_millis(10));
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

fn wait_for_state<F>(paths: &AppPaths, predicate: F) -> AppState
where
    F: Fn(&AppState) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(state) = AppState::load(paths)
            && predicate(&state)
        {
            return state;
        }
        if Instant::now() >= deadline {
            return AppState::load(paths).expect("state should reload");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_runtime_continuations<F>(paths: &AppPaths, predicate: F) -> RuntimeContinuationStore
where
    F: Fn(&RuntimeContinuationStore) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(continuations) = load_runtime_continuations_with_recovery(
            paths,
            &AppState::load(paths).unwrap_or_default().profiles,
        )
        .map(|loaded| loaded.value)
            && predicate(&continuations)
        {
            return continuations;
        }
        if Instant::now() >= deadline {
            return load_runtime_continuations_with_recovery(
                paths,
                &AppState::load(paths).unwrap_or_default().profiles,
            )
            .map(|loaded| loaded.value)
            .expect("runtime continuations should reload");
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
    websocket_requests: Arc<Mutex<Vec<String>>>,
    usage_accounts: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Clone, Copy)]
enum RuntimeProxyBackendMode {
    HttpOnly,
    HttpOnlyBufferedJson,
    HttpOnlyInitialBodyStall,
    HttpOnlySlowStream,
    HttpOnlyStallAfterSeveralChunks,
    HttpOnlyResetBeforeFirstByte,
    HttpOnlyResetAfterFirstChunk,
    HttpOnlyPreviousResponseNeedsTurnState,
    HttpOnlyCompactOverloaded,
    HttpOnlyUsageLimitMessage,
    HttpOnlyPlain429,
    Websocket,
    WebsocketDelayedQuotaAfterPrelude,
    WebsocketReuseSilentHang,
    WebsocketReusePreviousResponseNeedsTurnState,
    WebsocketCloseMidTurn,
    WebsocketPreviousResponseNeedsTurnState,
    WebsocketStaleReuseNeedsTurnState,
}

impl RuntimeProxyBackend {
    fn start() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnly)
    }

    fn start_http_initial_body_stall() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
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

    fn start_http_plain_429() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyPlain429)
    }

    fn start_websocket() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::Websocket)
    }

    fn start_websocket_delayed_quota_after_prelude() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude)
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
        let websocket_requests = Arc::new(Mutex::new(Vec::new()));
        let usage_accounts = Arc::new(Mutex::new(Vec::new()));
        let shutdown_flag = Arc::clone(&shutdown);
        let responses_accounts_flag = Arc::clone(&responses_accounts);
        let responses_headers_flag = Arc::clone(&responses_headers);
        let websocket_requests_flag = Arc::clone(&websocket_requests);
        let usage_accounts_flag = Arc::clone(&usage_accounts);
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let responses_accounts_flag = Arc::clone(&responses_accounts_flag);
                        let responses_headers_flag = Arc::clone(&responses_headers_flag);
                        let websocket_requests_flag = Arc::clone(&websocket_requests_flag);
                        let usage_accounts_flag = Arc::clone(&usage_accounts_flag);
                        let websocket_enabled = matches!(
                            mode,
                            RuntimeProxyBackendMode::Websocket
                                | RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude
                                | RuntimeProxyBackendMode::WebsocketReuseSilentHang
                                | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
                                | RuntimeProxyBackendMode::WebsocketCloseMidTurn
                                | RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                                | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
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
        } else if path.ends_with("/backend-api/codex/responses") {
            responses_accounts
                .lock()
                .expect("responses_accounts poisoned")
                .push(account_id.clone());
            responses_headers
                .lock()
                .expect("responses_headers poisoned")
                .push(captured_headers);
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
                "main-account" => {
                    let body = if matches!(mode, RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage)
                    {
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
                    ) && runtime_proxy_backend_next_second_response_id(
                        previous_response_id.as_deref()
                    )
                    .is_some()
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
                        && runtime_proxy_backend_next_second_response_id(
                            previous_response_id.as_deref()
                        )
                        .is_some() =>
                {
                    let next_response_id = runtime_proxy_backend_next_second_response_id(
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
                    if runtime_proxy_backend_next_second_response_id(
                        previous_response_id.as_deref()
                    )
                    .is_some() =>
                {
                    let next_response_id = runtime_proxy_backend_next_second_response_id(
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
                    if matches!(mode, RuntimeProxyBackendMode::HttpOnlyBufferedJson) {
                        (
                            "HTTP/1.1 200 OK",
                            "application/json",
                            serde_json::json!({
                                "id": "resp-second",
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
                            concat!(
                                "event: response.created\r\n",
                                "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second\"}}\r\n",
                                "\r\n",
                                "event: response.completed\r\n",
                                "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-second\"}}\r\n",
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
                        )
                    }
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
                "third-account" => (
                    "HTTP/1.1 200 OK",
                    "text/event-stream",
                    concat!(
                        "event: response.created\r\n",
                        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-third\"}}\r\n",
                        "\r\n",
                        "event: response.completed\r\n",
                        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-third\"}}\r\n",
                        "\r\n"
                        )
                        .to_string(),
                        None,
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                            .then_some(Duration::from_millis(100)),
                ),
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
                    Some(match account_id.as_str() {
                        "main-account" => "compact-turn-main",
                        "second-account" => "compact-turn-second",
                        "third-account" => "compact-turn-third",
                        _ => "compact-turn-unknown",
                    }
                    .to_string()),
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
            if matches!(mode, RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks)
                && account_id == "second-account"
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
                if matches!(mode, RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude)
                    && request_count == 1
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
                if matches!(mode, RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude)
                    && request_count >= 2
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
                                "type": "response.failed",
                                "status": 429,
                                "error": {
                                    "type": "usage_limit_reached",
                                    "message": "The usage limit has been reached"
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .expect("delayed quota error should be sent");
                    continue;
                }
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "error",
                            "status": 429,
                            "error": {
                                "code": "insufficient_quota",
                                "message": "main quota exhausted",
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("quota error should be sent");
            }
            "second-account"
                if matches!(
                    mode,
                    RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                        | RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState
                        | RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState
                ) && runtime_proxy_backend_next_second_response_id(previous_response_id.as_deref())
                    .is_some()
                    && effective_turn_state.as_deref() != Some("turn-second") =>
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
                if runtime_proxy_backend_next_second_response_id(previous_response_id.as_deref())
                    .is_some() =>
            {
                let next_response_id =
                    runtime_proxy_backend_next_second_response_id(previous_response_id.as_deref())
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
                                "id": "resp-second"
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
                                "id": "resp-second"
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
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": "resp-third"
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
                                "id": "resp-third"
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
fn runtime_state_save_debounce_only_applies_to_binding_updates() {
    assert_eq!(
        runtime_state_save_debounce("profile_commit:main"),
        Duration::ZERO
    );
    assert!(
        runtime_state_save_debounce("session_id:main") > Duration::ZERO,
        "session id saves should be debounced"
    );
    assert_eq!(
        runtime_state_save_debounce("response_ids:main"),
        Duration::ZERO
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
    assert_eq!(ranked[0].quota_source, RuntimeQuotaSource::PersistedSnapshot);
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
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 7_200, 95, 172_800)),
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("boom".to_string()),
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 1_800, 95, 259_200)),
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
    let cli = Cli::try_parse_from(["prodex", "quota", "--once"]).expect("quota command");
    let Commands::Quota(args) = cli.command else {
        panic!("expected quota command");
    };
    assert!(args.once);
    assert!(!quota_watch_enabled(&args));
}

#[test]
fn profile_quota_watch_output_contains_header_and_snapshot_body() {
    let output = render_profile_quota_watch_output(
        "main",
        "2026-03-22 10:00:00 WIB",
        Ok(usage_with_main_windows(63, 18_000, 12, 604_800)),
    );

    assert!(output.contains("Quota Watch"));
    assert!(output.contains("Profile"));
    assert!(output.contains("main"));
    assert!(output.contains("Updated"));
    assert!(output.contains("2026-03-22 10:00:00 WIB"));
    assert!(output.contains("Quota main"));
    assert!(!output.ends_with('\n'));
}

#[test]
fn all_quota_watch_output_preserves_updated_on_load_error() {
    let output = render_all_quota_watch_output(
        "2026-03-22 10:00:00 WIB",
        Err("load failed".to_string()),
        None,
        false,
    );

    assert!(output.contains("Quota Watch"));
    assert!(output.contains("Updated"));
    assert!(output.contains("2026-03-22 10:00:00 WIB"));
    assert!(output.contains("load failed"));
    assert!(!output.ends_with('\n'));
}

#[test]
fn quota_reports_include_pool_summary_lines() {
    let alpha = usage_with_main_windows(90, 7_200, 95, 172_800);
    let beta = usage_with_main_windows(45, 1_800, 40, 86_400);
    let reports = vec![
        QuotaReport {
            name: "alpha".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(alpha.clone()),
        },
        QuotaReport {
            name: "beta".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(beta.clone()),
        },
        QuotaReport {
            name: "api".to_string(),
            active: false,
            auth: AuthSummary {
                label: "api-key".to_string(),
                quota_compatible: false,
            },
            result: Err("auth mode is not quota-compatible".to_string()),
        },
    ];

    let output = render_quota_reports_with_layout(&reports, false, None, 160);
    let five_hour_reset = required_main_window_snapshot(&beta, "5h")
        .expect("5h snapshot")
        .reset_at;
    let weekly_reset = required_main_window_snapshot(&beta, "weekly")
        .expect("weekly snapshot")
        .reset_at;

    assert!(output.contains("5h remaining pool:"));
    assert!(output.contains("Weekly remaining pool:"));
    assert!(output.contains(&format_info_pool_remaining(135, 2, Some(five_hour_reset))));
    assert!(output.contains(&format_info_pool_remaining(135, 2, Some(weekly_reset))));
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
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 7_200, 95, 172_800)),
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("boom".to_string()),
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 1_800, 95, 259_200)),
        },
    ];

    let output = render_quota_reports_with_line_limit(&reports, false, Some(13));

    assert!(output.contains("ready-early"));
    assert!(output.contains("ready-late"));
    assert!(!output.contains("blocked"));
    assert!(!output.contains("error"));
    assert!(output.contains("showing top 2 of 4 profiles"));
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
        },
        QuotaReport {
            name: "ready-late".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 7_200, 95, 172_800)),
        },
        QuotaReport {
            name: "error".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Err("boom".to_string()),
        },
        QuotaReport {
            name: "ready-early".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 1_800, 95, 259_200)),
        },
    ];

    let window =
        render_quota_reports_window_with_layout(&reports, false, Some(13), 100, 1, true);

    assert_eq!(window.start_profile, 1);
    assert_eq!(window.total_profiles, 4);
    assert_eq!(window.shown_profiles, 2);
    assert_eq!(window.hidden_before, 1);
    assert_eq!(window.hidden_after, 1);
    assert!(window.output.contains("ready-late"));
    assert!(window.output.contains("blocked"));
    assert!(!window.output.contains("ready-early"));
    assert!(window
        .output
        .contains("press Up/Down to scroll profiles (2-3 of 4; 1 above, 1 below)"));
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
        },
        QuotaReport {
            name: "blocked".to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(0, 3_600, 80, 86_400)),
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

    mark_runtime_profile_transport_backoff(&shared, "main", "test")
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
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::from([("main".to_string(), now + 60)]),
        profile_transport_backoff_until: BTreeMap::from([("main".to_string(), now + 60)]),
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

    mark_runtime_profile_transport_backoff(&shared, "main", "first")
        .expect("first transport backoff should succeed");
    let first_until = shared
        .runtime
        .lock()
        .expect("runtime should lock")
        .profile_transport_backoff_until
        .get("main")
        .copied()
        .expect("first transport backoff should exist");

    mark_runtime_profile_transport_backoff(&shared, "main", "second")
        .expect("second transport backoff should succeed");
    let second_until = shared
        .runtime
        .lock()
        .expect("runtime should lock")
        .profile_transport_backoff_until
        .get("main")
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
            "main".to_string(),
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
            ("main".to_string(), now.saturating_add(90)),
            ("second".to_string(), now.saturating_add(30)),
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

    let switched = commit_runtime_proxy_profile_selection(&shared, "second", RuntimeRouteKind::Compact)
        .expect("compact profile commit should succeed");

    assert!(switched, "compact commit should switch the runtime current profile");
    let runtime = shared.runtime.lock().expect("runtime should lock");
    assert_eq!(runtime.current_profile, "second");
    assert_eq!(runtime.state.active_profile.as_deref(), Some("main"));
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
    summary
        .marker_counts
        .insert("compact_followup_owner", 1);
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
    summary.previous_response_not_found_by_route = BTreeMap::from([
        ("responses".to_string(), 1),
        ("websocket".to_string(), 1),
    ]);
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
    assert_eq!(
        value["previous_response_not_found_by_transport"]["http"],
        1
    );
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
    assert_eq!(value["persisted_continuation_journal_turn_state_bindings"], 1);
    assert_eq!(value["persisted_continuation_journal_session_id_bindings"], 4);
    assert_eq!(value["state_save_queue_backlog"], 2);
    assert_eq!(value["state_save_lag_ms"], 17);
    assert_eq!(value["continuation_journal_save_backlog"], 1);
    assert_eq!(value["continuation_journal_save_lag_ms"], 9);
    assert_eq!(value["profile_probe_refresh_backlog"], 3);
    assert_eq!(value["profile_probe_refresh_lag_ms"], 5);
    assert_eq!(value["continuation_journal_saved_at"], 123);
    assert_eq!(value["suspect_continuation_bindings"][0], "turn-second:suspect");
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
    assert_eq!(value["degraded_routes"][0], "main/responses circuit=open until=123");
    assert_eq!(value["orphan_managed_dirs"][0], "ghost_profile");
    assert_eq!(value["profiles"][0]["profile"], "main");
    assert_eq!(value["profiles"][0]["quota_freshness"], "stale");
    assert_eq!(value["profiles"][0]["routes"][0]["route"], "responses");
    assert_eq!(value["profiles"][0]["routes"][0]["performance_score"], 3);
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

    assert_eq!(fields.get("State save backlog").map(String::as_str), Some("4"));
    assert_eq!(fields.get("State save lag").map(String::as_str), Some("21"));
    assert_eq!(
        fields.get("Cont journal backlog").map(String::as_str),
        Some("2")
    );
    assert_eq!(fields.get("Cont journal lag").map(String::as_str), Some("11"));
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

    let future_now = SystemTime::now() + Duration::from_secs(ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS as u64 + 5);
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
            retry_backoff_until: BTreeMap::from([("main".to_string(), Local::now().timestamp() + 30)]),
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
    assert_eq!(summary.continuation_journal_saved_at, Some(journal_saved_at));
    assert_eq!(
        summary.suspect_continuation_bindings,
        vec!["resp-main:suspect".to_string()]
    );
    assert!(summary.degraded_routes.iter().any(|line| line.contains("main/responses")));
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
        merged.get("main").expect("main snapshot should exist").checked_at,
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
        runtime_profile_transport_health_penalty("responses_upstream_request"),
        RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY
    );
    assert_eq!(
        runtime_profile_transport_health_penalty("websocket_upstream_connect"),
        RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY
    );
    assert_eq!(
        runtime_profile_transport_health_penalty("responses_forward_response"),
        RUNTIME_PROFILE_FORWARD_FAILURE_HEALTH_PENALTY
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
    assert_eq!(loaded.last_run_selected_at.get("main").copied(), Some(now - 20));
    assert_eq!(loaded.last_run_selected_at.get("second").copied(), Some(now - 10));
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
        paths.state_file
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
    assert!(
        compacted
            .response_profile_bindings
            .contains_key(&format!("resp-{:06}-{}", 0, "x".repeat(64)))
    );
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
            transport_backoff_until: BTreeMap::from([("main".to_string(), now - 1)]),
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
    assert!(backoffs.route_circuit_open_until.contains_key(
        &runtime_profile_route_circuit_key("main", RuntimeRouteKind::Responses)
    ));
    assert!(!backoffs.route_circuit_open_until.contains_key(
        &runtime_profile_route_circuit_key("ghost", RuntimeRouteKind::Responses)
    ));
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
    fs::write(&pointer, format!("{}\n", temp_dir.path.join("missing.log").display()))
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
    assert_eq!(loaded.last_run_selected_at.get("second").copied(), Some(now - 20));
    assert_eq!(loaded.last_run_selected_at.get("main").copied(), Some(now - 10));
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
            transport_backoff_until: BTreeMap::from([("second".to_string(), now + 120)]),
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
    assert!(persisted_backoffs
        .transport_backoff_until
        .get("second")
        .is_some_and(|until| *until > Local::now().timestamp()));
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
    assert!(err.to_string().contains("injected runtime state save failure"));
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
            response: BTreeMap::from([(
                "resp-stale".to_string(),
                dead_continuation_status(now),
            )]),
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
    state.save(&paths).expect("initial state save should succeed");

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
        pruned.statuses.response.get("resp-main").map(|status| status.state),
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
        merged.statuses.response.get("resp-main").map(|status| status.state),
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
fn runtime_proxy_accepts_legacy_openai_prefix() {
    assert!(is_runtime_responses_path("/backend-api/codex/responses"));
    assert!(is_runtime_compact_path("/backend-api/codex/responses/compact"));
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/codex/responses"
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
}

#[test]
fn runtime_proxy_broker_key_changes_with_versioned_mount_path() {
    let current_key = runtime_broker_key("https://chatgpt.com/backend-api", false);

    let legacy_key = {
        let mut hasher = DefaultHasher::new();
        "https://chatgpt.com/backend-api".hash(&mut hasher);
        false.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };

    assert_ne!(current_key, legacy_key);
}

#[test]
fn runtime_doctor_summary_counts_recent_runtime_markers() {
    let summary = summarize_runtime_log_tail(
            br#"[2026-03-20 12:00:00.000 +07:00] request=1 transport=http first_upstream_chunk bytes=128
[2026-03-20 12:00:00.010 +07:00] request=1 transport=http first_local_chunk profile=main bytes=128 elapsed_ms=10
[2026-03-20 12:00:00.015 +07:00] runtime_proxy_admission_wait_started transport=http path=/backend-api/codex/responses budget_ms=120 poll_ms=10 reason=responses
[2026-03-20 12:00:00.020 +07:00] profile_transport_backoff profile=main until=123 reason=stream_read_error
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
    assert_eq!(runtime_doctor_marker_count(&summary, "state_save_skipped"), 1);
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_error"),
        1
    );
    assert_eq!(runtime_doctor_marker_count(&summary, "profile_circuit_open"), 1);
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
    assert_eq!(runtime_doctor_marker_count(&summary, "local_writer_error"), 1);
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
        summary.previous_response_not_found_by_route.get("websocket"),
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
        RuntimeResponsesAttempt::LocalSelectionBlocked { profile_name } => {
            assert_eq!(profile_name, "main");
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

    match attempt_runtime_standard_request(1, &request, &shared, "main")
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

    let second = acquire_runtime_proxy_active_request_slot_with_wait(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("second slot should recover after a short wait");
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
    request
        .headers_mut()
        .insert("session_id", "sess-123".parse().expect("valid header value"));
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
    request
        .headers_mut()
        .insert("User-Agent", "codex-cli/0.117.0".parse().expect("valid header value"));

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
    request
        .headers_mut()
        .insert("session_id", "sess-main".parse().expect("valid header value"));

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
fn runtime_proxy_websocket_preserves_function_call_output_affinity_when_previous_response_missing(
) {
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
fn exhausted_usage_snapshot_releases_persisted_affinity_bindings() {
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
        turn_state_bindings: BTreeMap::new(),
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

    let runtime = shared
        .runtime
        .lock()
        .expect("runtime lock should succeed");
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
    assert!(!runtime.state.response_profile_bindings.contains_key("resp-1"));
    assert!(!runtime.state.session_profile_bindings.contains_key("sess-123"));
    assert!(!runtime.turn_state_bindings.values().any(|binding| binding.profile_name == "main"));
    assert!(!runtime.session_id_bindings.contains_key("sess-123"));
}

#[test]
fn runtime_doctor_detects_upstream_without_local_chunk_in_sampled_tail() {
    let tail = concat!(
        "[2026-03-25 00:00:00.000 +07:00] request=1 route=responses transport=http first_upstream_chunk profile=main\n",
        "[2026-03-25 00:00:01.000 +07:00] request=1 route=responses transport=http stream_read_error profile=main reason=connection_reset\n",
    );

    let summary = summarize_runtime_log_tail(tail.as_bytes());
    assert_eq!(runtime_doctor_marker_count(&summary, "first_upstream_chunk"), 1);
    assert_eq!(runtime_doctor_marker_count(&summary, "first_local_chunk"), 0);
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
    let pointer = runtime_proxy_latest_log_pointer_path();
    let log_path = temp_dir.path.join("runtime-doctor.log");
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
    assert_eq!(runtime_doctor_marker_count(&summary, "profile_circuit_open"), 1);
    assert_eq!(summary.transport_pressure, "elevated");
    assert!(
        summary
            .diagnosis
            .contains("circuit breaker")
            || summary.diagnosis.contains("writer stall")
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
    assert!(!runtime.state.response_profile_bindings.contains_key("resp-missing"));
    assert!(!runtime.state.session_profile_bindings.contains_key("sess-missing"));
    assert!(!runtime.turn_state_bindings.contains_key("turn-missing"));
    assert!(!runtime.session_id_bindings.contains_key("sess-missing"));
    assert!(!runtime.profile_probe_cache.contains_key("missing"));
    assert!(!runtime.profile_usage_snapshots.contains_key("missing"));
    assert!(!runtime.profile_retry_backoff_until.contains_key("missing"));
    assert!(!runtime.profile_transport_backoff_until.contains_key("missing"));
    assert!(
        !runtime.profile_route_circuit_open_until.contains_key(
            &runtime_profile_route_circuit_key("missing", RuntimeRouteKind::Responses)
        )
    );
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

    assert!(status.is_success(), "bound compact should rotate pre-commit after owner overload: {status}");
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
    assert!(compact.status().is_success());
    assert_eq!(
        compact.text().expect("compact response body should be readable"),
        "{\"output\":[]}"
    );

    let continuations = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .session_id_bindings
            .get(&runtime_compact_session_lineage_key("sess-compact"))
            .is_some_and(|binding| binding.profile_name == "second")
            && continuations
                .turn_state_bindings
                .get(&runtime_compact_turn_state_lineage_key("compact-turn-second"))
                .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(
        continuations
            .session_id_bindings
            .get(&runtime_compact_session_lineage_key("sess-compact"))
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
    assert_eq!(
        continuations
            .turn_state_bindings
            .get(&runtime_compact_turn_state_lineage_key(&compact_turn_state))
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string(),
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
    assert!(compact.status().is_success());
    assert_eq!(
        compact.text().expect("compact response body should be readable"),
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
    assert!(followup_body.contains("\"resp-second\""));
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string(),
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
    let compact_turn_state = {
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
        assert!(compact.status().is_success());
        assert_eq!(
            compact.text().expect("compact response body should be readable"),
            "{\"output\":[]}"
        );
        compact_turn_state
    };

    let _ = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .session_id_bindings
            .get(&runtime_compact_session_lineage_key("sess-compact"))
            .is_some_and(|binding| binding.profile_name == "second")
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
    assert!(followup_body.contains("\"resp-second\""));
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string(),
        ]
    );
}

#[test]
fn runtime_proxy_uses_current_profile_without_runtime_quota_probe() {
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
    assert!(backend.usage_accounts().is_empty());
}

#[test]
fn runtime_proxy_sheds_long_lived_queue_overload_fast() {
    let _wait_budget_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS", "20");
    let _wait_poll_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_POLL_MS", "5");

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
    let _wait_poll_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_POLL_MS", "5");

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
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        let drained = release_receiver
            .lock()
            .expect("receiver mutex should not be poisoned")
            .recv_timeout(Duration::from_secs(1))
            .expect("queue should drain after short burst");
        assert_eq!(drained, 1);
    });

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

    release.join().expect("release thread should join");
    let queued = receiver
        .lock()
        .expect("receiver mutex should not be poisoned")
        .recv_timeout(Duration::from_secs(1))
        .expect("recovered item should reach the queue");
    assert_eq!(queued, 2);
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
    assert_eq!(backend.usage_accounts(), vec!["second-account".to_string()]);

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
    assert_eq!(backend.usage_accounts(), vec!["second-account".to_string()]);
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

    let message = extract_runtime_proxy_quota_message(body)
        .expect("text quota message should be detected");

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

    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("client");
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
            match response.text() {
                Ok(body) => assert!(
                    body.is_empty(),
                    "aborted runtime stream should terminate without a synthetic payload"
                ),
                Err(_) => {}
            }
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
        response_ready < Duration::from_millis(100),
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

    let client = Client::builder().build().expect("client");
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
    assert!(observed, "runtime log should capture first-chunk reset markers");
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

    let client = Client::builder().build().expect("client");
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
    assert!(observed, "runtime log should capture multi-chunk stall markers");
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
    assert!(observed, "runtime log should capture websocket reuse watchdog");
    let tail = String::from_utf8_lossy(&tail);
    assert!(tail.contains("websocket_reuse_watchdog"));
    assert!(tail.contains("websocket_precommit_frame_timeout"));
}

#[test]
fn runtime_proxy_bound_previous_response_without_turn_state_fails_as_transport_after_websocket_reuse_watchdog() {
    let backend = RuntimeProxyBackend::start_websocket_reuse_previous_response_needs_turn_state();
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
        3,
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
        vec!["main-account".to_string(), "second-account".to_string()],
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
    let _stale_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS", "1");
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
    assert!(saw_close, "stale websocket reuse should close the local transport");
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
    assert!(observed, "runtime log should capture stale websocket reuse blocking");
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
    let body = TwoChunkReader::new(vec![b"data: first\n\n".to_vec(), b"data: second\n\n".to_vec()]);
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

    let tail = read_runtime_log_tail(&log_path, 32 * 1024)
        .expect("runtime log tail should be readable");
    let tail = String::from_utf8_lossy(&tail);
    assert!(tail.contains("first_local_chunk"));
    assert!(tail.contains("local_writer_error"));
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
    assert!(first_body.contains("\"resp-second\""));

    let second = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
        .send()
        .expect("second runtime proxy request should succeed");
    let second_body = second
        .text()
        .expect("second response body should be readable");
    assert!(second_body.contains("\"resp-second-next\""));
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string(),
        ]
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
    {
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
        assert!(first_body.contains("\"resp-second\""));
    }

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
        .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
        .send()
        .expect("second runtime proxy request should succeed");
    let second_body = second
        .text()
        .expect("second response body should be readable");
    assert!(second_body.contains("\"resp-second-next\""));
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string(),
        ]
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

    let proxy =
        start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
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
    assert_eq!(backend.responses_accounts(), vec!["second-account".to_string()]);
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

    let proxy =
        start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
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
    assert_eq!(backend.responses_accounts(), vec!["second-account".to_string()]);
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

    let proxy =
        start_runtime_rotation_proxy(&paths, &state, "third", backend.base_url(), false)
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
    assert_eq!(backend.responses_accounts(), vec!["second-account".to_string()]);
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
    assert!(first_body.contains("\"resp-second\""));

    let second = client
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .body("{\"previous_response_id\":\"resp-second\",\"input\":[]}")
        .send()
        .expect("second runtime proxy request should succeed");
    let second_body = second
        .text()
        .expect("second response body should be readable");
    assert!(second_body.contains("\"resp-second-next\""));
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string(),
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
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
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
    retry_socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
                .to_string()
                .into(),
        ))
        .expect("second runtime proxy websocket request should be sent");

    let mut retry_payloads = Vec::new();
    loop {
        match retry_socket
            .read()
            .expect("second runtime proxy websocket should stay open")
        {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = is_runtime_terminal_event(&text);
                retry_payloads.push(text);
                if done {
                    break;
                }
            }
            WsMessage::Ping(payload) => {
                retry_socket
                    .send(WsMessage::Pong(payload))
                    .expect("pong should be sent");
            }
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket message: {other:?}"),
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
    {
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
        assert!(first_body.contains("\"resp-second\""));
    }

    let persisted = wait_for_state(&paths, |state| {
        state
            .session_profile_bindings
            .get("sess-second")
            .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(
        persisted
            .session_profile_bindings
            .get("sess-second")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );

    let mut resumed_state = AppState::load(&paths).expect("state should reload");
    resumed_state.active_profile = Some("third".to_string());
    resumed_state
        .save(&paths)
        .expect("failed to save resumed state");

    let resumed_proxy =
        start_runtime_rotation_proxy(&paths, &resumed_state, "third", backend.base_url(), false)
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
    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string(),
        ]
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
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
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
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
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
        body.contains("\"resp-third\""),
        "proxy should degrade to a fresh request after previous response discovery exhausts: {body}"
    );
    let accounts = backend.responses_accounts();
    assert!(
        accounts.iter().any(|account| account == "second-account"),
        "discovery should still probe alternate owners before falling back fresh: {accounts:?}"
    );
    assert_eq!(
        accounts.last().map(String::as_str),
        Some("third-account"),
        "fresh fallback should complete on a healthy candidate: {accounts:?}"
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
        payloads
            .iter()
            .any(|payload| payload.contains("\"resp-third\"")),
        "proxy should degrade to a fresh websocket request after previous response discovery exhausts: {payloads:?}"
    );
    let accounts = backend.responses_accounts();
    assert!(
        accounts.iter().any(|account| account == "second-account"),
        "discovery should still probe alternate websocket owners before falling back fresh: {accounts:?}"
    );
    assert_eq!(
        accounts.last().map(String::as_str),
        Some("third-account"),
        "fresh websocket fallback should complete on a healthy candidate: {accounts:?}"
    );
}

#[test]
fn runtime_proxy_falls_back_to_fresh_request_when_previous_response_missing_with_stale_turn_state_http(
) {
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

    assert_eq!(status.as_u16(), 200, "unexpected status: {status} body={body}");
    assert!(
        body.contains("\"resp-third\""),
        "proxy should degrade to a fresh request after previous response discovery exhausts: {body}"
    );

    let headers = backend.responses_headers();
    let final_headers = headers.last().expect("final request should be captured");
    assert_eq!(
        final_headers.get("chatgpt-account-id").map(String::as_str),
        Some("third-account"),
        "fresh fallback should complete on a healthy candidate: {headers:?}"
    );
    assert!(
        !final_headers.contains_key("x-codex-turn-state"),
        "fresh fallback should strip stale turn-state before the final attempt: {headers:?}"
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
    socket
        .send(WsMessage::Text(
            "{\"previous_response_id\":\"resp-second\",\"input\":[]}"
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
    let mut previous_response_id = {
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
        assert!(
            first_body.contains("\"resp-second\""),
            "unexpected first continuation body: {first_body}"
        );
        let _ = wait_for_runtime_continuations(&paths, |continuations| {
            continuations
                .response_profile_bindings
                .get("resp-second")
                .is_some_and(|binding| binding.profile_name == "second")
        });
        "resp-second".to_string()
    };

    for expected_response_id in ["resp-second-next", "resp-second-next-next"] {
        let mut resumed_state = AppState::load(&paths).expect("state should reload");
        resumed_state.active_profile = Some("third".to_string());
        resumed_state
            .save(&paths)
            .expect("failed to save resumed state");

        let proxy =
            start_runtime_rotation_proxy(&paths, &resumed_state, "third", backend.base_url(), false)
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
        let expected_response_id_owned = expected_response_id.to_string();
        let _ = wait_for_runtime_continuations(&paths, |continuations| {
            continuations
                .response_profile_bindings
                .get(expected_response_id)
                .is_some_and(|binding| binding.profile_name == "second")
        });
        previous_response_id = expected_response_id_owned;
    }

    assert_eq!(
        backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string(),
            "second-account".to_string(),
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
            .any(|payload| payload.contains("\"resp-second-next\""))
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
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

    assert!(runtime_proxy_persistence_enabled_for_log_path(&owner.log_path));
    assert!(!runtime_proxy_persistence_enabled_for_log_path(&follower.log_path));

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
    let follower =
        start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
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

    let mut request =
        tungstenite::client::IntoClientRequest::into_client_request(format!(
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
    request
        .headers_mut()
        .insert("User-Agent", "codex-cli/0.117.0".parse().expect("ua header"));

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
fn update_notice_is_suppressed_for_machine_output_modes() {
    assert!(!should_emit_update_notice(&Commands::Doctor(DoctorArgs {
        quota: false,
        runtime: true,
        json: true,
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
        .get(format!("http://{}/__prodex/runtime/health", proxy.listen_addr))
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
        .get(format!("http://{}/__prodex/runtime/health", proxy.listen_addr))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker health request should succeed")
        .json::<RuntimeBrokerHealth>()
        .expect("runtime broker health should decode");
    assert_eq!(health.current_profile, "second");
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
        !with_review.iter().any(|value| value == "true" || value == "false"),
        "review flag must be encoded as a clap boolean switch"
    );
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

    let recovered = wait_for_existing_runtime_broker_recovery_or_exit(
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

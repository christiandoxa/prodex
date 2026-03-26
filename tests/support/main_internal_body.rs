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

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
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

struct RuntimeProxyBackend {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    responses_accounts: Arc<Mutex<Vec<String>>>,
    responses_headers: Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    usage_accounts: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Clone, Copy)]
enum RuntimeProxyBackendMode {
    HttpOnly,
    HttpOnlyInitialBodyStall,
    HttpOnlySlowStream,
    HttpOnlyResetBeforeFirstByte,
    HttpOnlyResetAfterFirstChunk,
    HttpOnlyPreviousResponseNeedsTurnState,
    HttpOnlyCompactOverloaded,
    HttpOnlyUsageLimitMessage,
    HttpOnlyPlain429,
    Websocket,
    WebsocketCloseMidTurn,
    WebsocketPreviousResponseNeedsTurnState,
}

impl RuntimeProxyBackend {
    fn start() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnly)
    }

    fn start_http_initial_body_stall() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
    }

    fn start_http_slow_stream() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlySlowStream)
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

    fn start_websocket_close_mid_turn() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketCloseMidTurn)
    }

    fn start_websocket_previous_response_needs_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState)
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
        let usage_accounts = Arc::new(Mutex::new(Vec::new()));
        let shutdown_flag = Arc::clone(&shutdown);
        let responses_accounts_flag = Arc::clone(&responses_accounts);
        let responses_headers_flag = Arc::clone(&responses_headers);
        let usage_accounts_flag = Arc::clone(&usage_accounts);
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let responses_accounts_flag = Arc::clone(&responses_accounts_flag);
                        let responses_headers_flag = Arc::clone(&responses_headers_flag);
                        let usage_accounts_flag = Arc::clone(&usage_accounts_flag);
                        let websocket_enabled = matches!(
                            mode,
                            RuntimeProxyBackendMode::Websocket
                                | RuntimeProxyBackendMode::WebsocketCloseMidTurn
                                | RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState
                        );
                        thread::spawn(move || {
                            if websocket_enabled
                                && runtime_proxy_backend_is_websocket_upgrade(&stream)
                            {
                                handle_runtime_proxy_backend_websocket(
                                    stream,
                                    &responses_accounts_flag,
                                    &responses_headers_flag,
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
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                            .then_some(Duration::from_millis(100)),
                    )
                }
                "second-account"
                    if matches!(
                        mode,
                        RuntimeProxyBackendMode::HttpOnlyPreviousResponseNeedsTurnState
                    ) && previous_response_id.as_deref() == Some("resp-second")
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
                "second-account" if previous_response_id.as_deref() == Some("resp-second") => {
                    (
                        "HTTP/1.1 200 OK",
                        "text/event-stream",
                        concat!(
                            "event: response.created\r\n",
                            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-second-next\"}}\r\n",
                            "\r\n",
                            "event: response.completed\r\n",
                            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-second-next\"}}\r\n",
                            "\r\n"
                        )
                        .to_string(),
                        None,
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
                            .then_some(Duration::from_millis(750)),
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
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
                "second-account" => (
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
                        matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
                            .then_some(Duration::from_millis(100)),
                ),
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
                    matches!(mode, RuntimeProxyBackendMode::HttpOnlySlowStream)
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
                    None,
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
                    None,
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
                    None,
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
    ) && account_id == "second-account")
        .then(|| "turn-second".to_string());

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
        let previous_response_id = runtime_request_previous_response_id_from_text(&request);

        match account_id.as_str() {
            "main-account" => {
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
                ) && previous_response_id.as_deref() == Some("resp-second")
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
            "second-account" if previous_response_id.as_deref() == Some("resp-second") => {
                websocket
                    .send(WsMessage::Text(
                        serde_json::json!({
                            "type": "response.created",
                            "response": {
                                "id": "resp-second-next"
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
                                "id": "resp-second-next"
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
        session_profile_bindings: BTreeMap::new(),
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

    let output = render_quota_reports_with_line_limit(&reports, false, Some(10));

    assert!(output.contains("ready-early"));
    assert!(output.contains("ready-late"));
    assert!(!output.contains("blocked"));
    assert!(!output.contains("error"));
    assert!(output.contains("showing top 2 of 4 profiles"));
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
        session_profile_bindings: BTreeMap::new(),
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
        next_runtime_previous_response_candidate(&shared, &excluded, RuntimeRouteKind::Responses)
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
        false
    ));
    assert!(runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
        false
    ));

    let started_at = Instant::now()
        .checked_sub(Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_BUDGET_MS + 1))
        .expect("elapsed start should be constructible");
    assert!(runtime_proxy_precommit_budget_exhausted(
        started_at, 0, false
    ));
    assert!(!runtime_proxy_precommit_budget_exhausted(
        Instant::now(),
        RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
        true
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
    summary.marker_last_fields.insert(
        "selection_pick",
        BTreeMap::from([
            ("profile".to_string(), "second".to_string()),
            ("route".to_string(), "responses".to_string()),
            ("quota_source".to_string(), "persisted_snapshot".to_string()),
        ]),
    );
    summary.diagnosis = "Recent selection decisions were logged.".to_string();

    let value = runtime_doctor_json_value(&summary);
    assert_eq!(value["line_count"], 3);
    assert_eq!(value["first_timestamp"], "2026-03-25 00:00:00.000 +07:00");
    assert_eq!(value["last_timestamp"], "2026-03-25 00:00:05.000 +07:00");
    assert_eq!(value["marker_counts"]["selection_pick"], 2);
    assert_eq!(value["marker_counts"]["selection_skip_current"], 1);
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
    assert_eq!(
        value["diagnosis"],
        "Recent selection decisions were logged."
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
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
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
            Some("main"),
            false,
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
            true,
            RuntimeRouteKind::Responses,
        )
        .expect("candidate lookup should succeed"),
        Some("second".to_string())
    );
}

#[test]
fn merge_runtime_usage_snapshots_keeps_newer_entries() {
    let existing = BTreeMap::from([(
        "main".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: 10,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 70,
            five_hour_reset_at: 100,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 80,
            weekly_reset_at: 200,
        },
    )]);
    let incoming = BTreeMap::from([
        (
            "main".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: 20,
                five_hour_status: RuntimeQuotaWindowStatus::Exhausted,
                five_hour_remaining_percent: 0,
                five_hour_reset_at: 300,
                weekly_status: RuntimeQuotaWindowStatus::Critical,
                weekly_remaining_percent: 5,
                weekly_reset_at: 400,
            },
        ),
        (
            "stale".to_string(),
            RuntimeProfileUsageSnapshot {
                checked_at: 20,
                five_hour_status: RuntimeQuotaWindowStatus::Ready,
                five_hour_remaining_percent: 100,
                five_hour_reset_at: 300,
                weekly_status: RuntimeQuotaWindowStatus::Ready,
                weekly_remaining_percent: 100,
                weekly_reset_at: 400,
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
        20
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
    let stale_binding = now - APP_STATE_RESPONSE_BINDING_RETENTION_SECONDS - 5;
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
                    bound_at: stale_binding,
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
                    bound_at: stale_binding,
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
    assert!(!loaded.response_profile_bindings.contains_key("resp-stale"));
    assert!(loaded.session_profile_bindings.contains_key("sess-fresh"));
    assert!(!loaded.session_profile_bindings.contains_key("sess-stale"));
    assert!(raw.contains("resp-fresh"));
    assert!(!raw.contains("resp-stale"));
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
    let stale_binding = now - APP_STATE_SESSION_BINDING_RETENTION_SECONDS - 5;
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
                "bound_at": stale_binding
            }
        },
        "session_profile_bindings": {
            "sess-stale": {
                "profile_name": "main",
                "bound_at": stale_binding
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
    assert!(loaded.response_profile_bindings.is_empty());
    assert!(loaded.session_profile_bindings.is_empty());
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
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
    };

    schedule_runtime_state_save(
        &shared,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: profiles.clone(),
            last_run_selected_at: BTreeMap::from([("main".to_string(), now - 20)]),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        },
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
    schedule_runtime_state_save(
        &shared,
        AppState {
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
        },
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
            .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(
        persisted
            .response_profile_bindings
            .get("resp-second")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
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
        "openai_base_url=\"http://127.0.0.1:4455/backend-api/codex\""
    );
    assert_eq!(&rendered[4..], ["exec", "hello"]);
}

#[test]
fn runtime_proxy_maps_openai_prefix_to_upstream_backend_api() {
    assert_eq!(
        runtime_proxy_upstream_url(
            "https://chatgpt.com/backend-api",
            "/backend-api/codex/responses"
        ),
        "https://chatgpt.com/backend-api/codex/responses"
    );
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
[2026-03-20 12:00:00.075 +07:00] state_save_skipped revision=2 reason=session_id:main
[2026-03-20 12:00:00.080 +07:00] profile_probe_refresh_start profile=second
[2026-03-20 12:00:00.085 +07:00] profile_probe_refresh_ok profile=second
[2026-03-20 12:00:00.090 +07:00] profile_probe_refresh_error profile=third error=timeout
[2026-03-20 12:00:00.095 +07:00] profile_circuit_open profile=main route=responses until=123 reason=stream_read_error score=4
[2026-03-20 12:00:00.096 +07:00] profile_circuit_half_open_probe profile=main route=responses until=128 health=3
[2026-03-20 12:00:00.097 +07:00] websocket_reuse_watchdog profile=main event=read_error elapsed_ms=33 committed=true
[2026-03-20 12:00:00.099 +07:00] local_writer_error request=1 transport=http profile=main stage=chunk_flush chunks=1 bytes=128 elapsed_ms=20 error=broken_pipe
[2026-03-20 12:00:00.100 +07:00] runtime_proxy_startup_audit missing_managed_dirs=1 stale_response_bindings=2 stale_session_bindings=1 active_profile_missing_dir=false
"#,
        );

    assert_eq!(summary.line_count, 22);
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
    assert_eq!(runtime_doctor_marker_count(&summary, "local_writer_error"), 1);
    assert_eq!(
        runtime_doctor_marker_count(&summary, "runtime_proxy_startup_audit"),
        1
    );
    assert_eq!(
        runtime_doctor_top_facet(&summary, "quota_source").as_deref(),
        Some("persisted_snapshot (1)")
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
        .header("x-codex-turn-metadata", r#"{"source":"resume"}"#)
        .header("x-codex-beta-features", "remote-sync,realtime")
        .header("User-Agent", "codex-cli/0.116.0")
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
        Some(r#"{"source":"resume"}"#)
    );
    assert_eq!(
        first.get("x-codex-beta-features").map(String::as_str),
        Some("remote-sync,realtime")
    );
    assert_eq!(
        first.get("user-agent").map(String::as_str),
        Some("codex-cli/0.116.0")
    );
    assert_eq!(
        first.get("chatgpt-account-id").map(String::as_str),
        Some("second-account")
    );
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
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
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
    let log_path = fs::read_to_string(runtime_proxy_latest_log_pointer_path())
        .expect("latest runtime pointer should exist");
    let tail = read_runtime_log_tail(Path::new(log_path.trim()), 32 * 1024)
        .expect("runtime log tail should be readable");
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
            "third-account".to_string(),
            "third-account".to_string(),
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
            "third-account".to_string(),
            "third-account".to_string(),
            "third-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string(),
        ]
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

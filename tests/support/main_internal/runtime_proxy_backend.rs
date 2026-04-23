use super::*;

pub(super) fn runtime_proxy_backend_profile_name_for_account_id(account_id: &str) -> Option<&'static str> {
    match account_id {
        "main-account" => Some("main"),
        "second-account" => Some("second"),
        "third-account" => Some("third"),
        "fourth-account" => Some("fourth"),
        "fifth-account" => Some("fifth"),
        _ => None,
    }
}

pub(super) fn runtime_proxy_backend_account_id_for_profile_name(profile_name: &str) -> Option<&'static str> {
    match profile_name {
        "main" => Some("main-account"),
        "second" => Some("second-account"),
        "third" => Some("third-account"),
        "fourth" => Some("fourth-account"),
        "fifth" => Some("fifth-account"),
        _ => None,
    }
}

pub(super) fn runtime_proxy_backend_initial_response_id_for_account(account_id: &str) -> Option<&'static str> {
    match account_id {
        "second-account" => Some("resp-second"),
        "third-account" => Some("resp-third"),
        "fourth-account" => Some("resp-fourth"),
        "fifth-account" => Some("resp-fifth"),
        _ => None,
    }
}

pub(super) fn runtime_proxy_backend_response_owner_account_id(response_id: &str) -> Option<&'static str> {
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

pub(super) fn runtime_proxy_backend_profile_name_for_response_id(response_id: &str) -> Option<&'static str> {
    runtime_proxy_backend_response_owner_account_id(response_id)
        .and_then(runtime_proxy_backend_profile_name_for_account_id)
}

pub(super) fn runtime_proxy_backend_initial_response_id_for_profile_name(
    profile_name: &str,
) -> Option<&'static str> {
    runtime_proxy_backend_account_id_for_profile_name(profile_name)
        .and_then(runtime_proxy_backend_initial_response_id_for_account)
}

pub(super) fn runtime_proxy_backend_next_response_id(previous_response_id: Option<&str>) -> Option<String> {
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

pub(super) fn runtime_proxy_backend_is_owned_continuation(
    account_id: &str,
    previous_response_id: Option<&str>,
) -> bool {
    previous_response_id.is_some_and(|previous_response_id| {
        runtime_proxy_backend_response_owner_account_id(previous_response_id) == Some(account_id)
            && runtime_proxy_backend_next_response_id(Some(previous_response_id)).is_some()
    })
}

pub(super) fn runtime_proxy_backend_profile_name_for_compact_turn_state(
    turn_state: &str,
) -> Option<&'static str> {
    match turn_state {
        "compact-turn-main" => Some("main"),
        "compact-turn-second" => Some("second"),
        "compact-turn-third" => Some("third"),
        _ => None,
    }
}

pub(super) fn runtime_proxy_response_ids_from_http_body(body: &str) -> Vec<String> {
    let response_ids = extract_runtime_response_ids_from_body_bytes(body.as_bytes());
    if !response_ids.is_empty() {
        return response_ids;
    }

    body.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .flat_map(extract_runtime_response_ids_from_payload)
        .collect()
}

pub(super) fn wait_for_backend_usage_accounts(
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

pub(super) fn sorted_backend_usage_accounts(backend: &RuntimeProxyBackend) -> Vec<String> {
    let mut usage_accounts = backend.usage_accounts();
    usage_accounts.sort();
    usage_accounts
}

pub(super) fn closed_loopback_backend_base_url() -> String {
    let listener =
        TcpListener::bind("127.0.0.1:0").expect("closed loopback helper should bind a port");
    let addr = listener
        .local_addr()
        .expect("closed loopback helper should read local address");
    drop(listener);
    format!("http://{addr}/backend-api")
}

pub(super) fn unresponsive_loopback_backend_listener() -> (TcpListener, String) {
    let listener =
        TcpListener::bind("127.0.0.1:0").expect("unresponsive loopback helper should bind");
    let addr = listener
        .local_addr()
        .expect("unresponsive loopback helper should read local address");
    (listener, format!("http://{addr}/backend-api"))
}


pub(super) struct RuntimeProxyBackend {
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
pub(super) enum RuntimeProxyBackendMode {
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
    WebsocketPreviousResponseNotFoundAfterPrelude,
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
    pub(super) fn start() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnly)
    }

    pub(super) fn start_http_initial_body_stall() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyInitialBodyStall)
    }

    pub(super) fn start_http_unauthorized_main() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyUnauthorizedMain)
    }

    pub(super) fn start_http_buffered_json() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyBufferedJson)
    }

    pub(super) fn start_http_anthropic_web_search_followup() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyAnthropicWebSearchFollowup)
    }

    pub(super) fn start_http_anthropic_mcp_stream() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyAnthropicMcpStream)
    }

    pub(super) fn start_http_slow_stream() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlySlowStream)
    }

    pub(super) fn start_http_stall_after_several_chunks() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks)
    }

    pub(super) fn start_http_reset_before_first_byte() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyResetBeforeFirstByte)
    }

    pub(super) fn start_http_reset_after_first_chunk() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyResetAfterFirstChunk)
    }

    pub(super) fn start_http_previous_response_needs_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyPreviousResponseNeedsTurnState)
    }

    pub(super) fn start_http_previous_response_not_found_after_commit() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyPreviousResponseNotFoundAfterCommit)
    }

    pub(super) fn start_http_sse_headers_array_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlySseHeadersArrayTurnState)
    }

    pub(super) fn start_http_compact_overloaded() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyCompactOverloaded)
    }

    pub(super) fn start_http_compact_previous_response_not_found() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyCompactPreviousResponseNotFound)
    }

    pub(super) fn start_http_large_compact_response() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyLargeCompactResponse)
    }

    pub(super) fn start_http_usage_limit_message() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyUsageLimitMessage)
    }

    pub(super) fn start_http_usage_limit_message_late_ready_fifth() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth)
    }

    pub(super) fn start_http_delayed_quota_after_output_item_added() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyDelayedQuotaAfterOutputItemAdded)
    }

    pub(super) fn start_http_quota_then_tool_output_fresh_fallback_error() -> Self {
        Self::start_with_mode(
            RuntimeProxyBackendMode::HttpOnlyQuotaThenToolOutputFreshFallbackError,
        )
    }

    pub(super) fn start_http_previous_response_tool_context_missing() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyPreviousResponseToolContextMissing)
    }

    pub(super) fn start_http_plain_429() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyPlain429)
    }

    pub(super) fn start_websocket() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::Websocket)
    }

    pub(super) fn start_websocket_overloaded() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketOverloaded)
    }

    pub(super) fn start_websocket_delayed_quota_after_prelude() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketDelayedQuotaAfterPrelude)
    }

    pub(super) fn start_websocket_delayed_overload_after_prelude() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketDelayedOverloadAfterPrelude)
    }

    pub(super) fn start_websocket_reuse_silent_hang() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketReuseSilentHang)
    }

    pub(super) fn start_websocket_reuse_precommit_hold_stall() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketReusePrecommitHoldStall)
    }

    pub(super) fn start_websocket_reuse_owned_previous_response_silent_hang() -> Self {
        Self::start_with_mode(
            RuntimeProxyBackendMode::WebsocketReuseOwnedPreviousResponseSilentHang,
        )
    }

    pub(super) fn start_websocket_reuse_previous_response_needs_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketReusePreviousResponseNeedsTurnState)
    }

    pub(super) fn start_websocket_previous_response_not_found_after_prelude() -> Self {
        Self::start_with_mode(
            RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterPrelude,
        )
    }

    pub(super) fn start_websocket_previous_response_not_found_after_commit() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterCommit)
    }

    pub(super) fn start_websocket_previous_response_missing_without_turn_state() -> Self {
        Self::start_with_mode(
            RuntimeProxyBackendMode::WebsocketPreviousResponseMissingWithoutTurnState,
        )
    }

    pub(super) fn start_websocket_owned_tool_output_needs_session_replay() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketOwnedToolOutputNeedsSessionReplay)
    }

    pub(super) fn start_websocket_close_mid_turn() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketCloseMidTurn)
    }

    pub(super) fn start_websocket_previous_response_needs_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketPreviousResponseNeedsTurnState)
    }

    pub(super) fn start_websocket_stale_reuse_needs_turn_state() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketStaleReuseNeedsTurnState)
    }

    pub(super) fn start_websocket_top_level_response_id() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketTopLevelResponseId)
    }

    pub(super) fn start_websocket_realtime_sideband() -> Self {
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
                                | RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterPrelude
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

    pub(super) fn base_url(&self) -> String {
        format!("http://{}/backend-api", self.addr)
    }

    pub(super) fn responses_accounts(&self) -> Vec<String> {
        self.responses_accounts
            .lock()
            .expect("responses_accounts poisoned")
            .clone()
    }

    pub(super) fn responses_headers(&self) -> Vec<BTreeMap<String, String>> {
        self.responses_headers
            .lock()
            .expect("responses_headers poisoned")
            .clone()
    }

    pub(super) fn responses_bodies(&self) -> Vec<String> {
        self.responses_bodies
            .lock()
            .expect("responses_bodies poisoned")
            .clone()
    }

    pub(super) fn websocket_requests(&self) -> Vec<String> {
        self.websocket_requests
            .lock()
            .expect("websocket_requests poisoned")
            .clone()
    }

    pub(super) fn usage_accounts(&self) -> Vec<String> {
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

pub(super) fn handle_runtime_proxy_backend_request(
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
                        Some("turn-second".to_string()),
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

pub(super) fn runtime_proxy_backend_is_websocket_upgrade(stream: &TcpStream) -> bool {
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
pub(super) fn handle_runtime_proxy_backend_websocket(
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
                | RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterCommit
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
                    RuntimeProxyBackendMode::WebsocketPreviousResponseNotFoundAfterPrelude
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
                                "id": next_response_id
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .expect("response.created should be sent");
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
                    .expect("precommit previous_response_not_found should be sent");
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

pub(super) fn read_http_request(stream: &mut TcpStream) -> Option<String> {
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

pub(super) fn request_header(request: &str, header_name: &str) -> Option<String> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case(header_name) {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

pub(super) fn request_headers_map(request: &str) -> BTreeMap<String, String> {
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

pub(super) fn request_previous_response_id(request: &str) -> Option<String> {
    let body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default();
    runtime_request_previous_response_id_from_text(body)
}

pub(super) fn request_body_contains_only_function_call_output(request_body: &str) -> bool {
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

pub(super) fn request_body_contains_session_id(request_body: &str) -> bool {
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

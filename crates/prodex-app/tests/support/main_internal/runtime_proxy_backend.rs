use super::*;

#[path = "runtime_proxy_backend/request_parsing.rs"]
mod request_parsing;
#[path = "runtime_proxy_backend/shared.rs"]
mod shared;
#[path = "runtime_proxy_backend/websocket.rs"]
mod websocket;
#[path = "runtime_proxy_backend/http.rs"]
mod http;
#[path = "runtime_proxy_backend/fault_script.rs"]
mod fault_script;

pub(crate) use self::fault_script::*;
pub(super) use self::http::*;
pub(super) use self::request_parsing::*;
pub(super) use self::shared::*;
pub(super) use self::websocket::*;

pub(super) struct RuntimeProxyBackend {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    responses_accounts: Arc<Mutex<Vec<String>>>,
    responses_headers: Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    responses_bodies: Arc<Mutex<Vec<String>>>,
    websocket_requests: Arc<Mutex<Vec<String>>>,
    usage_accounts: Arc<Mutex<Vec<String>>>,
    reset_credit_consume_accounts: Arc<Mutex<Vec<String>>>,
    reset_credit_consume_bodies: Arc<Mutex<Vec<String>>>,
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
    HttpOnlyUsageLimitAutoRedeem,
    HttpOnlyUsageLimitMessageLateReadyFifth,
    HttpOnlyUsageLimitUntilThird,
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
    WebsocketKeepaliveBeforeContent,
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

    pub(super) fn start_http_usage_limit_auto_redeem() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyUsageLimitAutoRedeem)
    }

    pub(super) fn start_http_usage_limit_message_late_ready_fifth() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyUsageLimitMessageLateReadyFifth)
    }

    pub(super) fn start_http_usage_limit_until_third() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::HttpOnlyUsageLimitUntilThird)
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

    pub(super) fn start_websocket_keepalive_before_content() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketKeepaliveBeforeContent)
    }

    pub(super) fn start_websocket_realtime_sideband() -> Self {
        Self::start_with_mode(RuntimeProxyBackendMode::WebsocketRealtimeSideband)
    }

    pub(crate) fn start_with_fault_script(script: RuntimeProxyBackendFaultScript) -> Self {
        Self::start_with_mode_and_fault_script(
            RuntimeProxyBackendMode::HttpOnly,
            Some(Arc::new(Mutex::new(script))),
        )
    }

    fn start_with_mode(mode: RuntimeProxyBackendMode) -> Self {
        Self::start_with_mode_and_fault_script(mode, None)
    }

    fn start_with_mode_and_fault_script(
        mode: RuntimeProxyBackendMode,
        fault_script: Option<Arc<Mutex<RuntimeProxyBackendFaultScript>>>,
    ) -> Self {
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
        let reset_credit_consume_accounts = Arc::new(Mutex::new(Vec::new()));
        let reset_credit_consume_bodies = Arc::new(Mutex::new(Vec::new()));
        let connection_threads = Arc::new(Mutex::new(Vec::new()));
        let shutdown_flag = Arc::clone(&shutdown);
        let responses_accounts_flag = Arc::clone(&responses_accounts);
        let responses_headers_flag = Arc::clone(&responses_headers);
        let responses_bodies_flag = Arc::clone(&responses_bodies);
        let websocket_requests_flag = Arc::clone(&websocket_requests);
        let usage_accounts_flag = Arc::clone(&usage_accounts);
        let reset_credit_consume_accounts_flag = Arc::clone(&reset_credit_consume_accounts);
        let reset_credit_consume_bodies_flag = Arc::clone(&reset_credit_consume_bodies);
        let connection_threads_flag = Arc::clone(&connection_threads);
        let fault_script_flag = fault_script.as_ref().map(Arc::clone);
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let responses_accounts_flag = Arc::clone(&responses_accounts_flag);
                        let responses_headers_flag = Arc::clone(&responses_headers_flag);
                        let responses_bodies_flag = Arc::clone(&responses_bodies_flag);
                        let websocket_requests_flag = Arc::clone(&websocket_requests_flag);
                        let usage_accounts_flag = Arc::clone(&usage_accounts_flag);
                        let reset_credit_consume_accounts_flag =
                            Arc::clone(&reset_credit_consume_accounts_flag);
                        let reset_credit_consume_bodies_flag =
                            Arc::clone(&reset_credit_consume_bodies_flag);
                        let fault_script_flag = fault_script_flag.as_ref().map(Arc::clone);
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
                                | RuntimeProxyBackendMode::WebsocketKeepaliveBeforeContent
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
                                    RuntimeProxyBackendHttpContext {
                                        responses_accounts: &responses_accounts_flag,
                                        responses_headers: &responses_headers_flag,
                                        responses_bodies: &responses_bodies_flag,
                                        usage_accounts: &usage_accounts_flag,
                                        reset_credit_consume_accounts:
                                            &reset_credit_consume_accounts_flag,
                                        reset_credit_consume_bodies: &reset_credit_consume_bodies_flag,
                                        fault_script: fault_script_flag.as_ref(),
                                        mode,
                                    },
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
            reset_credit_consume_accounts,
            reset_credit_consume_bodies,
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

    pub(super) fn reset_credit_consume_bodies(&self) -> Vec<String> {
        self.reset_credit_consume_bodies
            .lock()
            .expect("reset_credit_consume_bodies poisoned")
            .clone()
    }

    pub(super) fn reset_credit_consume_accounts(&self) -> Vec<String> {
        self.reset_credit_consume_accounts
            .lock()
            .expect("reset_credit_consume_accounts poisoned")
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

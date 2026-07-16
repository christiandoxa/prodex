use super::*;

pub(super) enum RuntimeResponsesAttempt {
    Success {
        profile_name: String,
        response: RuntimeResponsesReply,
    },
    QuotaBlocked {
        profile_name: String,
        response: RuntimeResponsesReply,
    },
    Overloaded {
        profile_name: String,
        response: RuntimeResponsesReply,
    },
    AuthFailed {
        profile_name: String,
        response: RuntimeResponsesReply,
    },
    PreviousResponseNotFound {
        profile_name: String,
        response: RuntimeResponsesReply,
        turn_state: Option<String>,
    },
    LocalSelectionBlocked {
        profile_name: String,
        reason: &'static str,
    },
}

pub(super) enum RuntimeStandardAttempt {
    Success {
        profile_name: String,
        response: tiny_http::ResponseBox,
    },
    StaleContinuation {
        response: tiny_http::ResponseBox,
    },
    RetryableFailure {
        profile_name: String,
        response: tiny_http::ResponseBox,
        overload: bool,
    },
    AuthFailed {
        profile_name: String,
        response: tiny_http::ResponseBox,
    },
    LocalSelectionBlocked {
        profile_name: String,
    },
    TransportFailed {
        profile_name: String,
        stage: &'static str,
    },
}

#[derive(Debug)]
pub(super) enum RuntimeSseInspection {
    Commit {
        prelude: Vec<u8>,
        response_ids: Vec<String>,
        turn_state: Option<String>,
    },
    QuotaBlocked(Vec<u8>),
    Overloaded(Vec<u8>),
    PreviousResponseNotFound(Vec<u8>),
}

pub(super) type RuntimeSseInspectionProgress = runtime_proxy_crate::RuntimeSseInspectionProgress;

pub(super) type RuntimeSseTapEffect = runtime_proxy_crate::RuntimeSseTapEffect;
pub(super) type RuntimeSseTapState = runtime_proxy_crate::RuntimeSseTapState;
pub(super) type RuntimeSseTapStateInit<'a> = runtime_proxy_crate::RuntimeSseTapStateInit<'a>;

pub(super) type RuntimeTokenUsage = runtime_proxy_crate::RuntimeTokenUsage;

#[allow(clippy::large_enum_variant)]
pub(super) enum RuntimeResponsesReply {
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
    Streaming(RuntimeStreamingResponse),
}

pub(super) struct RuntimeStreamingResponse {
    pub(super) status: u16,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Box<dyn Read + Send>,
    pub(super) request_id: u64,
    pub(super) profile_name: String,
    pub(super) log_path: PathBuf,
    pub(super) shared: RuntimeRotationProxyShared,
    pub(super) _inflight_guard: Option<RuntimeProfileInFlightGuard>,
}

pub(super) struct RuntimeProfileInFlightGuard {
    pub(super) shared: RuntimeRotationProxyShared,
    pub(super) profile_name: String,
    pub(super) context: &'static str,
    pub(super) weight: usize,
}

pub(super) type RuntimeProxyActiveRequestGuard = prodex_runtime_state::RuntimeProxyAdmissionPermit;

impl Drop for RuntimeProfileInFlightGuard {
    fn drop(&mut self) {
        let _ = release_runtime_profile_inflight_guard(
            &self.shared,
            &self.profile_name,
            self.context,
            self.weight,
        );
    }
}

pub(super) enum RuntimePrefetchChunk {
    Data(Vec<u8>),
    End,
    Error(io::ErrorKind, String),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RuntimePrefetchSendOutcome {
    Sent { wait_ms: u128, retries: usize },
    Disconnected,
    TimedOut { message: String },
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RuntimePrefetchConfig {
    pub(super) retry_delay_ms: u64,
    pub(super) timeout_ms: u64,
    pub(super) max_buffered_bytes: usize,
    pub(super) lookahead_timeout_ms: u64,
    pub(super) stream_idle_timeout_ms: u64,
}

impl Default for RuntimePrefetchConfig {
    fn default() -> Self {
        Self {
            retry_delay_ms: RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS,
            timeout_ms: RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS,
            max_buffered_bytes: RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES,
            lookahead_timeout_ms: RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
            stream_idle_timeout_ms: RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
        }
    }
}

#[derive(Default)]
pub(super) struct RuntimePrefetchSharedState {
    pub(super) terminal_error: Mutex<Option<(io::ErrorKind, String)>>,
    pub(super) queued_bytes: AtomicUsize,
    pub(super) config: RuntimePrefetchConfig,
}

pub(super) struct RuntimePrefetchStream {
    pub(super) receiver: Option<Receiver<RuntimePrefetchChunk>>,
    pub(super) shared: Arc<RuntimePrefetchSharedState>,
    pub(super) backlog: VecDeque<RuntimePrefetchChunk>,
    pub(super) worker_abort: Option<tokio::task::AbortHandle>,
}

pub(super) struct RuntimePrefetchReader {
    pub(super) receiver: Receiver<RuntimePrefetchChunk>,
    pub(super) shared: Arc<RuntimePrefetchSharedState>,
    pub(super) backlog: VecDeque<RuntimePrefetchChunk>,
    pub(super) pending: Cursor<Vec<u8>>,
    pub(super) finished: bool,
    pub(super) worker_abort: tokio::task::AbortHandle,
}

#[derive(Debug)]
pub(super) enum RuntimeWebsocketAttempt {
    Delivered,
    QuotaBlocked {
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    },
    Overloaded {
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    },
    Rejected {
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
    },
    LocalSelectionBlocked {
        profile_name: String,
        reason: &'static str,
    },
    PreviousResponseNotFound {
        profile_name: String,
        payload: RuntimeWebsocketErrorPayload,
        turn_state: Option<String>,
    },
    TransportFailed {
        profile_name: String,
        stage: &'static str,
    },
    ReuseWatchdogTripped {
        profile_name: String,
        event: &'static str,
    },
}

#[allow(clippy::large_enum_variant)]
pub(super) enum RuntimeUpstreamFailureResponse {
    Http(RuntimeResponsesReply),
    Websocket(RuntimeWebsocketErrorPayload),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(super) enum RuntimeWebsocketConnectResult {
    Connected {
        socket: RuntimeUpstreamWebSocket,
        turn_state: Option<String>,
    },
    QuotaBlocked(RuntimeWebsocketErrorPayload),
    Overloaded(RuntimeWebsocketErrorPayload),
    Rejected(RuntimeWebsocketErrorPayload),
}

#[derive(Debug)]
pub(super) struct RuntimeWebsocketTcpConnectSuccess {
    pub(super) stream: TcpStream,
    pub(super) selected_addr: SocketAddr,
    pub(super) resolved_addrs: usize,
    pub(super) attempted_addrs: usize,
}

#[derive(Debug)]
pub(super) struct RuntimeWebsocketTcpAttemptResult {
    pub(super) addr: SocketAddr,
    pub(super) result: io::Result<TcpStream>,
}

pub(super) type RuntimeWebsocketErrorPayload = runtime_proxy_crate::RuntimeWebsocketErrorPayload;
pub(super) type RuntimeWebsocketRetryInspectionKind =
    runtime_proxy_crate::RuntimeWebsocketRetryInspectionKind;
pub(super) type RuntimeBufferedWebsocketTextFrame =
    runtime_proxy_crate::RuntimeBufferedWebsocketTextFrame;

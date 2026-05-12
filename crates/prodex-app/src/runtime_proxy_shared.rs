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
}

#[derive(Debug)]
pub(super) enum RuntimeSseInspection {
    Commit {
        prelude: Vec<u8>,
        response_ids: Vec<String>,
        turn_state: Option<String>,
    },
    QuotaBlocked(Vec<u8>),
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

pub(super) struct RuntimeProxyActiveRequestGuard {
    pub(super) active_request_count: Arc<AtomicUsize>,
    pub(super) lane_active_count: Arc<AtomicUsize>,
    pub(super) lane_releases_total: Arc<AtomicU64>,
    pub(super) active_request_release_underflows_total: Arc<AtomicU64>,
    pub(super) lane_release_underflows_total: Arc<AtomicU64>,
    pub(super) wait: Arc<(Mutex<()>, Condvar)>,
}

impl Drop for RuntimeProxyActiveRequestGuard {
    fn drop(&mut self) {
        let _ = release_runtime_proxy_active_request_guard(
            &self.active_request_count,
            &self.lane_active_count,
            &self.lane_releases_total,
            &self.active_request_release_underflows_total,
            &self.lane_release_underflows_total,
            &self.wait,
        );
    }
}

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

#[derive(Default)]
pub(super) struct RuntimePrefetchSharedState {
    pub(super) terminal_error: Mutex<Option<(io::ErrorKind, String)>>,
    pub(super) queued_bytes: AtomicUsize,
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

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

#[derive(Debug)]
pub(super) enum RuntimeSseInspectionProgress {
    Hold {
        response_ids: Vec<String>,
        turn_state: Option<String>,
    },
    Commit {
        response_ids: Vec<String>,
        turn_state: Option<String>,
    },
    QuotaBlocked,
    PreviousResponseNotFound,
}

#[derive(Default)]
pub(super) struct RuntimeParsedSseEvent {
    pub(super) quota_blocked: bool,
    pub(super) previous_response_not_found: bool,
    pub(super) response_ids: Vec<String>,
    pub(super) event_type: Option<String>,
    pub(super) turn_state: Option<String>,
}

#[derive(Default)]
pub(super) struct RuntimeSseTapState {
    pub(super) line: Vec<u8>,
    pub(super) data_lines: Vec<String>,
    pub(super) remembered_response_ids: BTreeSet<String>,
    pub(super) response_ids_with_turn_state: BTreeSet<String>,
    pub(super) turn_state: Option<String>,
}

#[allow(clippy::large_enum_variant)]
pub(super) enum RuntimeResponsesReply {
    Buffered(RuntimeBufferedResponseParts),
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
    pub(super) wait: Arc<(Mutex<()>, Condvar)>,
}

impl Drop for RuntimeProxyActiveRequestGuard {
    fn drop(&mut self) {
        let (mutex, condvar) = &*self.wait;
        let _guard = mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.active_request_count.fetch_sub(1, Ordering::SeqCst);
        self.lane_active_count.fetch_sub(1, Ordering::SeqCst);
        condvar.notify_all();
    }
}

impl Drop for RuntimeProfileInFlightGuard {
    fn drop(&mut self) {
        if let Ok(mut runtime) = self.shared.runtime.lock() {
            let remaining =
                if let Some(count) = runtime.profile_inflight.get_mut(&self.profile_name) {
                    *count = count.saturating_sub(self.weight);
                    let remaining = *count;
                    if remaining == 0 {
                        runtime.profile_inflight.remove(&self.profile_name);
                    }
                    remaining
                } else {
                    0
                };
            drop(runtime);
            runtime_proxy_log(
                &self.shared,
                format!(
                    "profile_inflight profile={} count={} weight={} context={} event=release",
                    self.profile_name, remaining, self.weight, self.context
                ),
            );
            self.shared
                .lane_admission
                .inflight_release_revision
                .fetch_add(1, Ordering::SeqCst);
            let (mutex, condvar) = &*self.shared.lane_admission.wait;
            let _guard = mutex
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            condvar.notify_all();
        }
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

#[derive(Debug, Clone)]
pub(super) enum RuntimeWebsocketErrorPayload {
    Text(String),
    Binary(Vec<u8>),
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeWebsocketRetryInspectionKind {
    QuotaBlocked,
    Overloaded,
    PreviousResponseNotFound,
}

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeInspectedWebsocketTextFrame {
    pub(super) event_type: Option<String>,
    pub(super) turn_state: Option<String>,
    pub(super) response_ids: Vec<String>,
    pub(super) retry_kind: Option<RuntimeWebsocketRetryInspectionKind>,
    pub(super) precommit_hold: bool,
    pub(super) terminal_event: bool,
}

#[derive(Debug)]
pub(super) struct RuntimeBufferedWebsocketTextFrame {
    pub(super) text: String,
    pub(super) response_ids: Vec<String>,
}

use super::*;

mod auth;
mod continuation_handling;
mod failure_handling;
mod loop_control;
mod setup;

pub(super) use self::auth::*;

fn runtime_websocket_should_promote_committed_profile(
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    compact_followup_profile: Option<&(String, &'static str)>,
    request_session_id_header_present: bool,
    bound_session_profile: Option<&str>,
) -> bool {
    previous_response_id.is_none()
        && bound_profile.is_none()
        && request_turn_state.is_none()
        && turn_state_profile.is_none()
        && compact_followup_profile.is_none()
        && !(request_session_id_header_present || bound_session_profile.is_some())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn proxy_runtime_websocket_text_message(
    session_id: u64,
    request_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    request_text: &str,
    request_metadata: &RuntimeWebsocketRequestMetadata,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
) -> Result<()> {
    RuntimeWebsocketTextMessageFlow::new(
        session_id,
        request_id,
        local_socket,
        handshake_request,
        request_text,
        request_metadata,
        shared,
        websocket_session,
    )?
    .run()
}

struct RuntimeWebsocketTextMessageFlow<'a> {
    session_id: u64,
    request_id: u64,
    local_socket: &'a mut RuntimeLocalWebSocket,
    handshake_request: RuntimeProxyRequest,
    request_text: String,
    shared: &'a RuntimeRotationProxyShared,
    websocket_session: &'a mut RuntimeWebsocketSessionState,
    request_requires_previous_response_affinity: bool,
    previous_response_id: Option<String>,
    request_turn_state: Option<String>,
    explicit_request_session_id: Option<RuntimeExplicitSessionId>,
    request_session_id_header_present: bool,
    previous_response_fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    request_session_id: Option<String>,
    bound_profile: Option<String>,
    trusted_previous_response_affinity: bool,
    turn_state_profile: Option<String>,
    bound_session_profile: Option<String>,
    compact_followup_profile: Option<(String, &'static str)>,
    compact_session_profile: Option<String>,
    session_profile: Option<String>,
    pinned_profile: Option<String>,
    excluded_profiles: BTreeSet<String>,
    last_failure: Option<(RuntimeUpstreamFailureResponse, bool)>,
    previous_response_retry_candidate: Option<String>,
    previous_response_retry_index: usize,
    candidate_turn_state_retry_profile: Option<String>,
    candidate_turn_state_retry_value: Option<String>,
    saw_inflight_saturation: bool,
    saw_previous_response_not_found: bool,
    websocket_reuse_fresh_retry_profiles: BTreeSet<String>,
}

#[derive(Clone, Copy)]
enum RuntimeWebsocketMessageLoopAction {
    Continue,
    Finished,
}

#[derive(Clone, Copy)]
enum RuntimeWebsocketDirectCurrentFallbackReason {
    PrecommitBudgetExhausted,
    CandidateExhausted,
}

impl RuntimeWebsocketDirectCurrentFallbackReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::PrecommitBudgetExhausted => "precommit_budget_exhausted",
            Self::CandidateExhausted => "candidate_exhausted",
        }
    }

    fn previous_response_not_found_policy(self) -> RuntimePreviousResponseNotFoundPolicy {
        RuntimePreviousResponseNotFoundPolicy::websocket(
            matches!(self, Self::PrecommitBudgetExhausted),
            false,
        )
    }

    fn reset_previous_response_retry_index_on_local_block(self) -> bool {
        matches!(self, Self::PrecommitBudgetExhausted)
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;
    use std::net::TcpListener;
    use tungstenite::connect;

    pub(super) fn test_runtime_shared(name: &str) -> RuntimeRotationProxyShared {
        let root = env::temp_dir().join(format!(
            "prodex-websocket-message-test-{name}-{}",
            std::process::id()
        ));
        let paths = AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared-codex"),
            legacy_shared_codex_root: root.join("shared"),
            root,
        };

        RuntimeRotationProxyShared {
            upstream_no_proxy: false,
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
                profile_inflight: BTreeMap::new(),
                profile_health: BTreeMap::new(),
            })),
            log_path: env::temp_dir().join(format!(
                "prodex-websocket-message-test-{name}-{}.log",
                std::process::id()
            )),
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

    pub(super) fn test_runtime_local_websocket_pair()
    -> (RuntimeLocalWebSocket, RuntimeUpstreamWebSocket) {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("test websocket listener should bind");
        let addr = listener
            .local_addr()
            .expect("test websocket listener should expose an address");
        let client = thread::spawn(move || {
            let (socket, _response) =
                connect(format!("ws://{addr}")).expect("test websocket client should connect");
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

    pub(super) fn read_runtime_websocket_text(socket: &mut RuntimeUpstreamWebSocket) -> String {
        match socket
            .read()
            .expect("test websocket client should read a frame")
        {
            WsMessage::Text(text) => text.to_string(),
            other => panic!("expected websocket text frame, got {other:?}"),
        }
    }

    pub(super) fn test_runtime_websocket_flow<'a>(
        local_socket: &'a mut RuntimeLocalWebSocket,
        shared: &'a RuntimeRotationProxyShared,
        websocket_session: &'a mut RuntimeWebsocketSessionState,
    ) -> RuntimeWebsocketTextMessageFlow<'a> {
        RuntimeWebsocketTextMessageFlow {
            session_id: 7,
            request_id: 11,
            local_socket,
            handshake_request: RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/backend-api/realtime".to_string(),
                headers: Vec::new(),
                body: br#"{"input":[]}"#.to_vec(),
            },
            request_text: r#"{"type":"response.create"}"#.to_string(),
            shared,
            websocket_session,
            request_requires_previous_response_affinity: false,
            previous_response_id: None,
            request_turn_state: None,
            explicit_request_session_id: None,
            request_session_id_header_present: false,
            previous_response_fresh_fallback_shape: None,
            request_session_id: None,
            bound_profile: None,
            trusted_previous_response_affinity: false,
            turn_state_profile: None,
            bound_session_profile: None,
            compact_followup_profile: None,
            compact_session_profile: None,
            session_profile: None,
            pinned_profile: None,
            excluded_profiles: BTreeSet::new(),
            last_failure: None,
            previous_response_retry_candidate: None,
            previous_response_retry_index: 0,
            candidate_turn_state_retry_profile: None,
            candidate_turn_state_retry_value: None,
            saw_inflight_saturation: false,
            saw_previous_response_not_found: false,
            websocket_reuse_fresh_retry_profiles: BTreeSet::new(),
        }
    }
}

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

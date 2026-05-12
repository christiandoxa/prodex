use super::*;

pub(super) struct RuntimeWebsocketSessionStartRequest<'a> {
    pub(super) request_id: u64,
    pub(super) handshake_request: &'a RuntimeProxyRequest,
    pub(super) request_previous_response_id: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) websocket_session: &'a mut RuntimeWebsocketSessionState,
    pub(super) profile_name: &'a str,
    pub(super) turn_state_override: Option<&'a str>,
    pub(super) promote_committed_profile: bool,
}

pub(super) struct RuntimeWebsocketSessionStart {
    pub(super) upstream_socket: RuntimeUpstreamWebSocket,
    pub(super) upstream_turn_state: Option<String>,
    pub(super) inflight_guard: Option<RuntimeProfileInFlightGuard>,
    pub(super) reuse_existing_session: bool,
    pub(super) precommit_hold_promotion_allowed: bool,
    pub(super) precommit_transport_retry_allowed: bool,
    pub(super) reuse_started_at: Option<Instant>,
    pub(super) precommit_started_at: Instant,
}

pub(super) enum RuntimeWebsocketSessionStartDecision {
    Started(Box<RuntimeWebsocketSessionStart>),
    Attempt(RuntimeWebsocketAttempt),
}

pub(super) fn start_runtime_websocket_upstream_session(
    request: RuntimeWebsocketSessionStartRequest<'_>,
) -> Result<RuntimeWebsocketSessionStartDecision> {
    let RuntimeWebsocketSessionStartRequest {
        request_id,
        handshake_request,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        shared,
        websocket_session,
        profile_name,
        turn_state_override,
        promote_committed_profile,
    } = request;

    let reuse_existing_session = websocket_session.can_reuse(profile_name, turn_state_override);
    let precommit_hold_promotion_allowed = runtime_websocket_precommit_hold_promotion_allowed(
        reuse_existing_session,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        turn_state_override,
        promote_committed_profile,
    );
    let precommit_transport_retry_allowed = runtime_websocket_precommit_transport_retry_allowed(
        reuse_existing_session,
        request_previous_response_id,
        request_turn_state,
        turn_state_override,
        promote_committed_profile,
    );
    let reuse_started_at = reuse_existing_session.then(Instant::now);
    let precommit_started_at = Instant::now();
    let (mut upstream_socket, upstream_turn_state, inflight_guard) = if reuse_existing_session {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "websocket_reuse_start",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field(
                        "turn_state_override",
                        format!("{turn_state_override:?}"),
                    ),
                ],
            ),
        );
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket upstream_session=reuse profile={profile_name} turn_state_override={:?}",
                turn_state_override
            ),
        );
        let Some(socket) = websocket_session.take_socket() else {
            websocket_session.reset();
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "websocket_reuse_missing_socket",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "websocket"),
                        runtime_proxy_log_field("profile", profile_name),
                    ],
                ),
            );
            return Ok(RuntimeWebsocketSessionStartDecision::Attempt(
                RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                    profile_name: profile_name.to_string(),
                    event: "reuse_missing_socket",
                },
            ));
        };
        (socket, websocket_session.turn_state.clone(), None)
    } else {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket upstream_session=connect profile={profile_name} turn_state_override={:?}",
                turn_state_override
            ),
        );
        match connect_runtime_proxy_upstream_websocket(
            request_id,
            handshake_request,
            shared,
            profile_name,
            turn_state_override,
        ) {
            Ok(RuntimeWebsocketConnectResult::Connected { socket, turn_state }) => (
                socket,
                turn_state,
                Some(acquire_runtime_profile_inflight_guard(
                    shared,
                    profile_name,
                    "websocket_session",
                )?),
            ),
            Ok(RuntimeWebsocketConnectResult::QuotaBlocked(payload)) => {
                return Ok(RuntimeWebsocketSessionStartDecision::Attempt(
                    RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name: profile_name.to_string(),
                        payload,
                    },
                ));
            }
            Ok(RuntimeWebsocketConnectResult::Overloaded(payload)) => {
                return Ok(RuntimeWebsocketSessionStartDecision::Attempt(
                    RuntimeWebsocketAttempt::Overloaded {
                        profile_name: profile_name.to_string(),
                        payload,
                    },
                ));
            }
            Err(_err) if precommit_transport_retry_allowed => {
                return Ok(RuntimeWebsocketSessionStartDecision::Attempt(
                    RuntimeWebsocketAttempt::TransportFailed {
                        profile_name: profile_name.to_string(),
                        stage: "connect",
                    },
                ));
            }
            Err(err) => return Err(err),
        }
    };
    runtime_set_upstream_websocket_io_timeout(
        &mut upstream_socket,
        Some(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms(),
        )),
    )
    .context("failed to configure runtime websocket pre-commit timeout")?;

    Ok(RuntimeWebsocketSessionStartDecision::Started(Box::new(
        RuntimeWebsocketSessionStart {
            upstream_socket,
            upstream_turn_state,
            inflight_guard,
            reuse_existing_session,
            precommit_hold_promotion_allowed,
            precommit_transport_retry_allowed,
            reuse_started_at,
            precommit_started_at,
        },
    )))
}

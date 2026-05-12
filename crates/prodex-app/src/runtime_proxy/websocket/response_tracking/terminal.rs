use super::*;

pub(super) struct RuntimeWebsocketTerminalEventRequest<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) websocket_session: &'a mut RuntimeWebsocketSessionState,
    pub(super) profile_name: &'a str,
    pub(super) event_type: Option<&'a str>,
    pub(super) precommit_hold_count: usize,
    pub(super) committed_previous_response_not_found: bool,
    pub(super) upstream_socket: RuntimeUpstreamWebSocket,
    pub(super) upstream_turn_state: Option<String>,
    pub(super) inflight_guard: Option<RuntimeProfileInFlightGuard>,
}

pub(super) fn finish_runtime_websocket_terminal_event(
    request: RuntimeWebsocketTerminalEventRequest<'_>,
) -> RuntimeWebsocketAttempt {
    let RuntimeWebsocketTerminalEventRequest {
        request_id,
        shared,
        websocket_session,
        profile_name,
        event_type,
        precommit_hold_count,
        committed_previous_response_not_found,
        mut upstream_socket,
        upstream_turn_state,
        inflight_guard,
    } = request;

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=websocket terminal_event profile={profile_name} event_type={} precommit_hold_count={precommit_hold_count}",
            event_type.unwrap_or("-"),
        ),
    );
    if committed_previous_response_not_found {
        let _ = upstream_socket.close(None);
        websocket_session.reset();
    } else {
        websocket_session.store(
            upstream_socket,
            profile_name,
            upstream_turn_state,
            inflight_guard,
        );
    }
    RuntimeWebsocketAttempt::Delivered
}

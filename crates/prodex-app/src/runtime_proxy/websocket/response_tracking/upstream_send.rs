use super::*;

pub(super) struct RuntimeWebsocketUpstreamSendRequest<'a, 'socket> {
    pub(super) request_id: u64,
    pub(super) request_text: &'a str,
    pub(super) handshake_request: &'a RuntimeProxyRequest,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) websocket_session: &'a mut RuntimeWebsocketSessionState,
    pub(super) profile_name: &'a str,
    pub(super) reuse_existing_session: bool,
    pub(super) precommit_transport_retry_allowed: bool,
    pub(super) upstream_socket: &'socket mut RuntimeUpstreamWebSocket,
}

pub(super) fn send_runtime_websocket_upstream_request(
    request: RuntimeWebsocketUpstreamSendRequest<'_, '_>,
) -> Result<Option<RuntimeWebsocketAttempt>> {
    let RuntimeWebsocketUpstreamSendRequest {
        request_id,
        request_text,
        handshake_request,
        shared,
        websocket_session,
        profile_name,
        reuse_existing_session,
        precommit_transport_retry_allowed,
        upstream_socket,
    } = request;

    let upstream_request_text = prepare_runtime_smart_context_websocket_text(
        request_id,
        request_text,
        handshake_request,
        shared,
        profile_name,
    );
    if let Err(err) =
        upstream_socket.send(WsMessage::Text(upstream_request_text.into_owned().into()))
    {
        let _ = upstream_socket.close(None);
        websocket_session.reset();
        let transport_error =
            anyhow::anyhow!("failed to send runtime websocket request upstream: {err}");
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            "websocket_upstream_send",
            &transport_error,
        );
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "upstream_send_error",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("error", err.to_string()),
                ],
            ),
        );
        if reuse_existing_session {
            return Ok(Some(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name: profile_name.to_string(),
                event: "upstream_send_error",
            }));
        }
        if precommit_transport_retry_allowed {
            return Ok(Some(RuntimeWebsocketAttempt::TransportFailed {
                profile_name: profile_name.to_string(),
                stage: "send",
            }));
        }
        return Err(transport_error);
    }

    Ok(None)
}

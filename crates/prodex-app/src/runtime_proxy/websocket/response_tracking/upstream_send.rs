use anyhow::Result;
use tungstenite::Message as WsMessage;

use super::{
    RuntimeProxyRequest, RuntimeRotationProxyShared, RuntimeRouteKind, RuntimeUpstreamWebSocket,
    RuntimeWebsocketAttempt, RuntimeWebsocketSessionState,
    apply_runtime_presidio_redaction_to_websocket_text, note_runtime_profile_transport_failure,
    prepare_runtime_smart_context_websocket_text, runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_structured_log_message, runtime_websocket_error_log_value,
};
use crate::runtime_proxy::log_runtime_upstream_payload_snapshot;

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

    let inspected =
        apply_runtime_presidio_redaction_to_websocket_text(request_id, request_text, shared)?;
    let upstream_request_text = prepare_runtime_smart_context_websocket_text(
        request_id,
        inspected.text.as_ref(),
        handshake_request,
        shared,
        profile_name,
    );
    log_runtime_upstream_payload_snapshot(
        shared,
        request_id,
        "websocket",
        RuntimeRouteKind::Websocket,
        profile_name,
        upstream_request_text.as_bytes(),
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
                    runtime_proxy_log_field(
                        "error",
                        runtime_websocket_error_log_value(&err.to_string()),
                    ),
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

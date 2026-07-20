use super::{
    RuntimeWebsocketSessionState, RuntimeWebsocketUpstreamSendRequest,
    send_runtime_websocket_upstream_request,
};
use crate::{
    RuntimeLocalWebSocket, RuntimeProxyRequest, RuntimeRotationProxyShared,
    runtime_proxy_next_request_id, runtime_set_upstream_websocket_read_timeout,
    runtime_websocket_timeout_error,
};
use anyhow::{Context, Result};
use std::thread;
use std::time::Duration;
use tungstenite::{Error as WsError, Message as WsMessage};

const RUNTIME_REALTIME_PUMP_TIMEOUT: Duration = Duration::from_millis(20);
const RUNTIME_REALTIME_IDLE_SLEEP: Duration = Duration::from_millis(5);

pub(in crate::runtime_proxy) fn run_runtime_realtime_websocket_duplex_session(
    _session_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
) -> Result<()> {
    let profile_name = websocket_session
        .profile_name
        .clone()
        .context("realtime websocket committed without a profile")?;
    let mut upstream_socket = websocket_session
        .take_socket()
        .context("realtime websocket committed without an upstream socket")?;
    runtime_set_upstream_websocket_read_timeout(
        &mut upstream_socket,
        Some(RUNTIME_REALTIME_PUMP_TIMEOUT),
    )
    .context("failed to configure realtime upstream pump timeout")?;

    loop {
        let mut progressed = false;
        match local_socket.read() {
            Ok(WsMessage::Text(text)) => {
                progressed = true;
                let request_id = runtime_proxy_next_request_id(shared);
                if send_runtime_websocket_upstream_request(RuntimeWebsocketUpstreamSendRequest {
                    request_id,
                    request_text: text.as_ref(),
                    handshake_request,
                    shared,
                    websocket_session,
                    profile_name: &profile_name,
                    reuse_existing_session: true,
                    precommit_transport_retry_allowed: false,
                    upstream_socket: &mut upstream_socket,
                })?
                .is_some()
                {
                    return Ok(());
                }
            }
            Ok(WsMessage::Binary(payload)) => {
                progressed = true;
                upstream_socket
                    .send(WsMessage::Binary(payload))
                    .context("failed to forward realtime websocket binary frame upstream")?;
            }
            Ok(WsMessage::Ping(payload)) => {
                progressed = true;
                local_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to realtime websocket ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = upstream_socket.close(frame.clone());
                let _ = local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                progressed = true;
            }
            Err(error) if runtime_websocket_timeout_error(&error) => {}
            Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => return Ok(()),
            Err(error) => {
                return Err(anyhow::anyhow!("realtime local websocket failed: {error}"));
            }
        }

        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                progressed = true;
                local_socket
                    .send(WsMessage::Text(text))
                    .context("failed to forward realtime websocket text frame")?;
            }
            Ok(WsMessage::Binary(payload)) => {
                progressed = true;
                local_socket
                    .send(WsMessage::Binary(payload))
                    .context("failed to forward realtime websocket binary frame")?;
            }
            Ok(WsMessage::Ping(payload)) => {
                progressed = true;
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to realtime upstream ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                progressed = true;
            }
            Err(error) if runtime_websocket_timeout_error(&error) => {}
            Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => return Ok(()),
            Err(error) => {
                return Err(anyhow::anyhow!(
                    "realtime upstream websocket failed: {error}"
                ));
            }
        }

        if !progressed {
            thread::sleep(RUNTIME_REALTIME_IDLE_SLEEP);
        }
    }
}

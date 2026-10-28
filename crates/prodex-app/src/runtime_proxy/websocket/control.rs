//! Responses interrupts belong to one already-created response on one transport.
//! They are never model requests, rotation candidates, or replayable retries.
use super::response_tracking::readiness::read_response_frame;
use super::{
    RuntimeLocalWebSocket, RuntimeRotationProxyShared, RuntimeUpstreamWebSocket,
    RuntimeWebsocketSessionState, WsError, WsMessage, runtime_proxy_log,
    runtime_set_upstream_websocket_io_timeout,
};
use anyhow::{Context, Result};
use std::{
    io,
    time::{Duration, Instant},
};

#[derive(serde::Deserialize)]
struct ControlEnvelope {
    #[serde(rename = "type")]
    kind: String,
    response_id: Option<serde_json::Value>,
}

pub(super) fn interrupt_response_id(text: &str) -> Result<Option<String>> {
    let Ok(envelope) = serde_json::from_str::<ControlEnvelope>(text) else {
        return Ok(None);
    };
    if envelope.kind != "response.interrupt" {
        return Ok(None);
    }
    let id = envelope
        .response_id
        .and_then(|id| id.as_str().map(str::to_owned));
    let Some(id) = id.filter(|id| !id.is_empty() && id.trim() == id) else {
        anyhow::bail!("runtime websocket interrupt requires its original response identifier");
    };
    Ok(Some(id))
}

pub(super) fn is_response_interrupt(text: &str) -> bool {
    serde_json::from_str::<ControlEnvelope>(text)
        .is_ok_and(|envelope| envelope.kind == "response.interrupt")
}

pub(super) fn interrupt_acknowledgement(text: &str, expected: &str) -> Result<bool> {
    let Ok(envelope) = serde_json::from_str::<ControlEnvelope>(text) else {
        return Ok(false);
    };
    if !matches!(
        envelope.kind.as_str(),
        "response.interrupt.accepted" | "response.interrupt.failed"
    ) {
        return Ok(false);
    }
    anyhow::ensure!(
        envelope
            .response_id
            .as_ref()
            .and_then(serde_json::Value::as_str)
            == Some(expected),
        "runtime websocket interruption acknowledgement belongs to another response"
    );
    Ok(true)
}

pub(super) fn poll_local_frame(
    local: &mut RuntimeLocalWebSocket,
    session: &RuntimeWebsocketSessionState,
) -> Result<Option<WsMessage>> {
    if session.pending_local_frame.is_some() {
        return Ok(None);
    }
    let Some(control) = session.local_control.as_ref() else {
        return Ok(None);
    };
    control.set_nonblocking(true)?;
    let frame = local.read();
    control.set_nonblocking(false)?;
    match frame {
        Err(WsError::Io(error))
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(None)
        }
        other => other.map(Some).map_err(Into::into),
    }
}

pub(in crate::runtime_proxy) fn forward_runtime_websocket_interrupt(
    text: &str,
    local: &mut RuntimeLocalWebSocket,
    session: &mut RuntimeWebsocketSessionState,
    shared: &RuntimeRotationProxyShared,
) -> Result<bool> {
    let Some(id) = interrupt_response_id(text)? else {
        return Ok(false);
    };
    // A late interrupt may arrive after completion. Only the exact cached socket
    // can accept it; no new connection or account selection is ever attempted.
    anyhow::ensure!(
        session.response_transport_generation(&id) == Some(session.transport_generation()),
        "runtime websocket interrupt original response transport is unavailable"
    );
    let Some(mut socket) = session.take_socket() else {
        anyhow::bail!("runtime websocket interrupt original connection is unavailable");
    };
    let timeout = Duration::from_millis(
        shared
            .runtime_config
            .tuning
            .websocket_precommit_progress_timeout_ms
            .max(1),
    );
    let result = forward_cached_interrupt(text, &id, local, &mut socket, timeout);
    if result.is_ok() {
        runtime_set_upstream_websocket_io_timeout(
            &mut socket,
            Some(Duration::from_millis(
                shared.runtime_config.tuning.stream_idle_timeout_ms.max(1),
            )),
        )?;
        session.restore_socket(socket);
        runtime_proxy_log(
            shared,
            "websocket_interrupt_forwarded transport=cached rotated=false",
        );
    } else {
        session.reset();
    }
    result.map(|()| true)
}

fn forward_cached_interrupt(
    text: &str,
    id: &str,
    local: &mut RuntimeLocalWebSocket,
    socket: &mut RuntimeUpstreamWebSocket,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    runtime_set_upstream_websocket_io_timeout(socket, Some(timeout))?;
    socket.send(WsMessage::Text(text.to_owned().into()))?;
    loop {
        anyhow::ensure!(
            Instant::now() < deadline,
            "runtime websocket interruption acknowledgement timed out"
        );
        match read_response_frame(
            socket,
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50)),
        )? {
            Some(WsMessage::Text(text)) => {
                let acknowledgement = interrupt_acknowledgement(&text, id)?;
                let failed = serde_json::from_str::<ControlEnvelope>(&text).is_ok_and(|event| {
                    matches!(event.kind.as_str(), "error" | "response.interrupt.failed")
                });
                local.send(WsMessage::Text(text))?;
                anyhow::ensure!(
                    !failed,
                    "runtime websocket interruption was rejected on its original connection"
                );
                if acknowledgement {
                    return Ok(());
                }
            }
            Some(WsMessage::Ping(payload)) => socket.send(WsMessage::Pong(payload))?,
            Some(WsMessage::Close(_)) => {
                anyhow::bail!("runtime websocket interruption owner closed")
            }
            Some(WsMessage::Binary(_)) => {
                anyhow::bail!("unexpected binary interruption acknowledgement")
            }
            Some(WsMessage::Pong(_) | WsMessage::Frame(_)) | None => {}
        }
    }
}

pub(super) fn send_interrupt_on_owned_socket(
    socket: &mut RuntimeUpstreamWebSocket,
    text: tungstenite::Utf8Bytes,
    timeout: Duration,
) -> Result<()> {
    fn tcp(socket: &RuntimeUpstreamWebSocket) -> Result<&std::net::TcpStream> {
        match socket.get_ref() {
            tungstenite::stream::MaybeTlsStream::Plain(stream) => Ok(stream),
            tungstenite::stream::MaybeTlsStream::Rustls(stream) => Ok(&stream.sock),
            _ => anyhow::bail!("unsupported interruption transport"),
        }
    }
    let original = tcp(socket)?.write_timeout()?;
    tcp(socket)?.set_write_timeout(Some(original.map_or(timeout, |limit| limit.min(timeout))))?;
    let sent = socket.send(WsMessage::Text(text));
    tcp(socket)?.set_write_timeout(original)?;
    sent.context("failed to forward interruption on its original connection")
}

use super::{RuntimeWebsocketAttempt, RuntimeWebsocketResponseLoop, RuntimeWebsocketTextResult};
use tungstenite::{Error as WsError, Message as WsMessage};

use crate::{RuntimeUpstreamWebSocket, runtime_set_upstream_websocket_io_timeout};
use anyhow::{Context, Result};
use std::time::Duration;

pub(super) fn mark_runtime_websocket_upstream_frame_seen(
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    first_upstream_frame_seen: &mut bool,
    timeout_ms: u64,
) -> Result<()> {
    if *first_upstream_frame_seen {
        return Ok(());
    }
    *first_upstream_frame_seen = true;
    runtime_set_upstream_websocket_io_timeout(
        upstream_socket,
        Some(Duration::from_millis(timeout_ms)),
    )
    .context("failed to restore runtime websocket upstream timeout")
}

// The frame reader is synchronous; selection-loop checks cannot interrupt its
// socket wait. Cap the existing I/O timeout by the same request deadline rather
// than granting another idle budget after hidden metadata or keepalive frames.
pub(super) fn bound_runtime_websocket_precommit_io(
    socket: &mut RuntimeUpstreamWebSocket,
    shared: &crate::runtime_state_shared::RuntimeRotationProxyShared,
) -> Result<()> {
    shared.check_precommit_deadline()?;
    if shared.precommit_deadline.is_none() {
        return Ok(());
    }
    let stream = match socket.get_mut() {
        tungstenite::stream::MaybeTlsStream::Plain(stream) => stream,
        tungstenite::stream::MaybeTlsStream::Rustls(stream) => &mut stream.sock,
        _ => anyhow::bail!("runtime websocket transport has no supported deadline binding"),
    };
    let read = shared.precommit_wait_slice(stream.read_timeout()?.unwrap_or(Duration::MAX))?;
    let write = shared.precommit_wait_slice(stream.write_timeout()?.unwrap_or(Duration::MAX))?;
    if read.is_zero() || write.is_zero() {
        return Err(crate::runtime_proxy::RuntimeProxyRecoveryTimeout.into());
    }
    stream.set_read_timeout(Some(read))?;
    stream.set_write_timeout(Some(write))?;
    Ok(())
}

pub(super) fn run_runtime_websocket_response_loop(
    mut flow: RuntimeWebsocketResponseLoop<'_>,
) -> Result<RuntimeWebsocketAttempt> {
    loop {
        if !flow.committed
            && let Err(error) =
                bound_runtime_websocket_precommit_io(&mut flow.upstream_socket, flow.shared)
        {
            flow.websocket_session.reset();
            return Err(error);
        }
        let frame = flow.upstream_socket.read();
        if !flow.committed
            && let Err(error) = flow.shared.check_precommit_deadline()
        {
            // Expiry is local: do not quarantine the profile or classify it
            // as a retryable transport error. Drop this attempt's socket.
            flow.websocket_session.reset();
            return Err(error);
        }
        match frame {
            Ok(WsMessage::Text(text)) => match flow.handle_text(text.to_string())? {
                RuntimeWebsocketTextResult::Continue => {}
                RuntimeWebsocketTextResult::Terminal(terminal) => {
                    return Ok(flow.finish_terminal(terminal));
                }
                RuntimeWebsocketTextResult::Attempt(attempt) => return Ok(attempt),
            },
            Ok(WsMessage::Binary(payload)) => flow.handle_binary(payload)?,
            Ok(WsMessage::Ping(payload)) => flow.handle_ping(payload)?,
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => flow.handle_keepalive()?,
            Ok(WsMessage::Close(frame)) => {
                let _ = frame;
                return flow.handle_close();
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                return flow.handle_connection_closed();
            }
            Err(err) => return flow.handle_read_error(err),
        }
    }
}

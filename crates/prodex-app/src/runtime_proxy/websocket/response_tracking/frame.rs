use super::{RuntimeWebsocketAttempt, RuntimeWebsocketResponseLoop, RuntimeWebsocketTextResult};
use tungstenite::{Error as WsError, Message as WsMessage};

use crate::{RuntimeUpstreamWebSocket, runtime_set_upstream_websocket_io_timeout};
use anyhow::{Context, Result};
use std::time::{Duration, Instant};

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
    let mut last_upstream_frame = Instant::now();
    let mut last_client_heartbeat = Instant::now();
    loop {
        if let Some(attempt) =
            prepare_response_read(&mut flow, last_upstream_frame, &mut last_client_heartbeat)?
        {
            return Ok(attempt);
        }
        let frame = flow.upstream_socket.read();
        check_response_deadline(&mut flow)?;
        // A poll timeout is not an upstream failure until its independent
        // progress/idle clock expires. Keep partial TLS/WebSocket reads intact.
        if matches!(&frame, Err(error) if crate::runtime_websocket_timeout_error(error)) {
            continue;
        }
        if frame.is_ok() {
            last_upstream_frame = Instant::now();
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

fn check_response_deadline(flow: &mut RuntimeWebsocketResponseLoop<'_>) -> Result<()> {
    if !flow.committed
        && let Err(error) = flow.shared.check_precommit_deadline()
    {
        // Expiry is local, not evidence of depleted quota or a broken profile.
        flow.websocket_session.reset();
        return Err(error);
    }
    Ok(())
}

fn prepare_response_read(
    flow: &mut RuntimeWebsocketResponseLoop<'_>,
    last_upstream_frame: Instant,
    last_client_heartbeat: &mut Instant,
) -> Result<Option<RuntimeWebsocketAttempt>> {
    check_response_deadline(flow)?;
    let timeout_ms = if flow.first_upstream_text_seen {
        flow.shared.runtime_config.tuning.stream_idle_timeout_ms
    } else {
        flow.shared
            .runtime_config
            .tuning
            .websocket_precommit_progress_timeout_ms
    };
    let plan = prodex_mojo_core::websocket_response_tracking::response_wait_plan(
        u64::try_from(last_upstream_frame.elapsed().as_millis()).unwrap_or(u64::MAX),
        timeout_ms,
        u64::try_from(last_client_heartbeat.elapsed().as_millis()).unwrap_or(u64::MAX),
    )
    .map_err(|error| anyhow::anyhow!("Mojo WebSocket liveness plan failed: {error:?}"))?;
    if plan.expired {
        return flow
            .handle_read_error(WsError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "upstream WebSocket progress deadline expired",
            )))
            .map(Some);
    }
    if plan.heartbeat {
        // Protocol control frames are visible to the client idle clock but
        // never commit output, change affinity, or renew upstream progress.
        flow.local_socket
            .send(WsMessage::Ping(Default::default()))
            .context("failed to send runtime WebSocket liveness heartbeat")?;
        *last_client_heartbeat = Instant::now();
    }
    crate::runtime_set_upstream_websocket_read_timeout(
        &mut flow.upstream_socket,
        Some(Duration::from_millis(plan.wait_ms)),
    )?;
    if !flow.committed
        && let Err(error) =
            bound_runtime_websocket_precommit_io(&mut flow.upstream_socket, flow.shared)
    {
        flow.websocket_session.reset();
        return Err(error);
    }
    Ok(None)
}

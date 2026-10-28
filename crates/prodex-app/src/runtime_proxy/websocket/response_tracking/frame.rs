use super::readiness::{read_response_frame, wait_response_readiness};
use super::{
    RuntimeWebsocketAttempt, RuntimeWebsocketResponseLoop, RuntimeWebsocketTextResult,
    RuntimeWebsocketTextTerminal,
};
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
    let mut interrupt = super::control::ResponseInterrupt::default();
    loop {
        check_response_deadline(&mut flow)?;
        if let Err(error) = interrupt.pump(&mut flow) {
            flow.websocket_session.reset();
            return Err(error);
        }
        // Drain an already-buffered frame before declaring upstream idle.
        // Host commit/persistence or downstream writes may take longer than
        // the idle budget while the terminal frame is already waiting. The
        // absolute precommit watchdog still wins before and after this read.
        let frame = read_response_frame(&mut flow.upstream_socket, Duration::ZERO);
        check_response_deadline(&mut flow)?;
        if matches!(&frame, Ok(Some(_))) {
            last_upstream_frame = Instant::now();
        }
        let wait = match prepare_response_read(
            &mut flow,
            last_upstream_frame,
            &mut last_client_heartbeat,
        )? {
            ResponseReadPreparation::Wait(wait) => wait,
            ResponseReadPreparation::Attempt(attempt) => return Ok(attempt),
        };
        let frame = match frame {
            Ok(None) => {
                if let Err(error) = wait_for_response_or_control(&flow, wait) {
                    return flow.handle_read_error(WsError::Io(error));
                }
                continue;
            }
            Ok(Some(frame)) => Ok(frame),
            Err(error) => Err(error),
        };
        match process_response_frame(&mut flow, &mut interrupt, frame)? {
            ResponseFrameAction::Continue => {}
            ResponseFrameAction::Terminal(terminal) => return Ok(flow.finish_terminal(terminal)),
            ResponseFrameAction::Attempt(attempt) => return Ok(attempt),
            ResponseFrameAction::Close => return flow.handle_close(),
            ResponseFrameAction::ConnectionClosed => return flow.handle_connection_closed(),
            ResponseFrameAction::ReadError(error) => return flow.handle_read_error(error),
        }
    }
}

enum ResponseFrameAction {
    Continue,
    Terminal(RuntimeWebsocketTextTerminal),
    Attempt(RuntimeWebsocketAttempt),
    Close,
    ConnectionClosed,
    ReadError(WsError),
}

fn wait_for_response_or_control(
    flow: &RuntimeWebsocketResponseLoop<'_>,
    wait: Duration,
) -> std::io::Result<()> {
    let wait = if flow.websocket_session.local_control.is_some() {
        wait.min(Duration::from_millis(50))
    } else {
        wait
    };
    wait_response_readiness(&flow.upstream_socket, wait)
}

fn process_response_frame(
    flow: &mut RuntimeWebsocketResponseLoop<'_>,
    interrupt: &mut super::control::ResponseInterrupt,
    frame: std::result::Result<WsMessage, WsError>,
) -> Result<ResponseFrameAction> {
    match frame {
        Ok(WsMessage::Text(text)) => return process_response_text(flow, interrupt, &text),
        Ok(WsMessage::Binary(payload)) => flow.handle_binary(payload)?,
        Ok(WsMessage::Ping(payload)) => flow.handle_ping(payload)?,
        Ok(WsMessage::Pong(_) | WsMessage::Frame(_)) => flow.handle_keepalive()?,
        Ok(WsMessage::Close(_)) => return Ok(ResponseFrameAction::Close),
        Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => {
            return Ok(ResponseFrameAction::ConnectionClosed);
        }
        Err(error) => return Ok(ResponseFrameAction::ReadError(error)),
    }
    Ok(ResponseFrameAction::Continue)
}

fn process_response_text(
    flow: &mut RuntimeWebsocketResponseLoop<'_>,
    interrupt: &mut super::control::ResponseInterrupt,
    text: &str,
) -> Result<ResponseFrameAction> {
    if interrupt.observe_ack(flow, text)? {
        return Ok(interrupt
            .take_terminal()
            .map(ResponseFrameAction::Terminal)
            .unwrap_or(ResponseFrameAction::Continue));
    }
    Ok(match flow.handle_text(text.to_string())? {
        RuntimeWebsocketTextResult::Continue => ResponseFrameAction::Continue,
        RuntimeWebsocketTextResult::Terminal(terminal) => interrupt
            .defer_terminal(terminal)
            .map(ResponseFrameAction::Terminal)
            .unwrap_or(ResponseFrameAction::Continue),
        RuntimeWebsocketTextResult::Attempt(attempt) => ResponseFrameAction::Attempt(attempt),
    })
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

enum ResponseReadPreparation {
    Wait(Duration),
    Attempt(RuntimeWebsocketAttempt),
}

fn prepare_response_read(
    flow: &mut RuntimeWebsocketResponseLoop<'_>,
    last_upstream_frame: Instant,
    last_client_heartbeat: &mut Instant,
) -> Result<ResponseReadPreparation> {
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
            .map(ResponseReadPreparation::Attempt);
    }
    if plan.heartbeat {
        // Protocol control frames are visible to the client idle clock but
        // never commit output, change affinity, or renew upstream progress.
        flow.local_socket
            .send(WsMessage::Ping(Default::default()))
            .context("failed to send runtime WebSocket liveness heartbeat")?;
        *last_client_heartbeat = Instant::now();
    }
    if !flow.committed
        && let Err(error) =
            bound_runtime_websocket_precommit_io(&mut flow.upstream_socket, flow.shared)
    {
        flow.websocket_session.reset();
        return Err(error);
    }
    let wait = Duration::from_millis(plan.wait_ms);
    let wait = if flow.committed {
        wait
    } else {
        flow.shared.precommit_wait_slice(wait)?
    };
    Ok(ResponseReadPreparation::Wait(wait))
}

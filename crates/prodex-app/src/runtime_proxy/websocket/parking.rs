//! Bounded parking for local Responses sockets between messages. Only a worker
//! that owns a complete message holds admission or enters upstream I/O. Socket,
//! parser fragments and upstream continuation state move together, never clone.
use super::read_loop::{RuntimeWebsocketSessionReadAction, handle_runtime_proxy_websocket_frame};
use super::{
    RuntimeLocalWebSocket, RuntimeProxyRequest, RuntimeRotationProxyShared,
    RuntimeWebsocketSessionState, WsError, WsMessage,
    acquire_runtime_proxy_active_request_slot_with_wait_for_request, runtime_proxy_log,
    runtime_websocket_error_log_value, runtime_websocket_local_disconnect_error,
    send_runtime_proxy_websocket_error,
};
use anyhow::Result;
use std::collections::VecDeque;
use std::{io, sync::Mutex, time::Duration};

pub(crate) struct RuntimeWebsocketSessionPool {
    idle: Mutex<VecDeque<ParkedSession>>,
    capacity: usize,
}

struct ParkedSession {
    id: u64,
    socket: RuntimeLocalWebSocket,
    handshake: RuntimeProxyRequest,
    control: tiny_http::ConnectionControl,
    state: RuntimeWebsocketSessionState,
}

impl RuntimeWebsocketSessionPool {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            idle: Mutex::new(VecDeque::new()),
            capacity: capacity.max(1),
        }
    }

    pub(crate) fn park_new(
        &self,
        id: u64,
        socket: RuntimeLocalWebSocket,
        handshake: RuntimeProxyRequest,
        control: tiny_http::ConnectionControl,
        shared: &RuntimeRotationProxyShared,
    ) {
        // This bounds a stalled downstream writer, not model reasoning time.
        if let Err(error) = control.set_write_timeout(Some(Duration::from_millis(
            shared.runtime_config.tuning.stream_idle_timeout_ms.max(1),
        ))) {
            runtime_proxy_log(
                shared,
                format!("websocket_park_failed session={id} error={error}"),
            );
            let _ = control.shutdown();
            return;
        }
        let mut state = RuntimeWebsocketSessionState::default();
        state.local_control = Some(control.clone());
        self.park(
            ParkedSession {
                id,
                socket,
                handshake,
                state,
                control,
            },
            shared,
        );
    }

    fn park(&self, session: ParkedSession, shared: &RuntimeRotationProxyShared) {
        let evicted = {
            let mut idle = self.idle.lock().unwrap_or_else(|p| p.into_inner());
            let evicted = (idle.len() >= self.capacity)
                .then(|| idle.pop_front())
                .flatten();
            idle.push_back(session);
            evicted
        };
        // Only parked, non-executing connections are reclaimable. No committed
        // stream can be here. Shared affinity/quotas are not modified by eviction.
        if let Some(session) = evicted {
            runtime_proxy_log(
                shared,
                format!(
                    "websocket_idle_evicted session={} reason=connection_cache_capacity",
                    session.id
                ),
            );
            let _ = session.control.shutdown();
        }
    }

    /// Poll one parked socket without blocking on reads. The caller owns a fixed
    /// executor thread; no connection creates a thread or detached inference task.
    pub(crate) fn poll_one(&self, shared: &RuntimeRotationProxyShared) -> bool {
        let Some(mut session) = self
            .idle
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pop_front()
        else {
            return false;
        };
        let outcome = crate::runtime_panic::catch_runtime_unwind_silently(|| session.poll(shared));
        let outcome =
            outcome.unwrap_or_else(|_| Err(anyhow::anyhow!("runtime websocket worker panicked")));
        match outcome {
            Ok(Some(progress)) => {
                self.park(session, shared);
                progress
            }
            Ok(None) => true,
            Err(error) => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "websocket_session_finished session={} error={}",
                        session.id,
                        runtime_websocket_error_log_value(&format!("{error:#}"))
                    ),
                );
                let _ = session.control.shutdown();
                true
            }
        }
    }

    pub(crate) fn close_idle(&self) {
        let idle = std::mem::take(&mut *self.idle.lock().unwrap_or_else(|p| p.into_inner()));
        for session in idle {
            let _ = session.control.shutdown();
        }
    }
}

impl ParkedSession {
    fn poll(&mut self, shared: &RuntimeRotationProxyShared) -> Result<Option<bool>> {
        // Only this worker owns the socket. Restore blocking writes BEFORE any
        // response forwarding. Tungstenite retains partial frames on WouldBlock.
        let frame = if let Some(frame) = self.state.pending_local_frame.take() {
            Ok(frame)
        } else {
            self.control.set_nonblocking(true)?;
            let frame = self.socket.read();
            self.control.set_nonblocking(false)?;
            frame
        };
        let frame = match frame {
            Err(WsError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {
                return Ok(Some(false));
            }
            Err(error) if runtime_websocket_local_disconnect_error(&error) => return Ok(None),
            Err(error) => return Err(error.into()),
            Ok(frame) => frame,
        };
        let scoped = shared.for_precommit_request();
        let _active = if let WsMessage::Text(text) = &frame
            && !super::control::is_response_interrupt(text)
        {
            let mut request = self.handshake.clone();
            request.body = text.as_bytes().to_vec();
            match acquire_runtime_proxy_active_request_slot_with_wait_for_request(
                &scoped,
                "websocket",
                &request.path_and_query,
                Some(&request),
            ) {
                Ok(permit) => Some(permit),
                Err(_) => {
                    send_runtime_proxy_websocket_error(
                        &mut self.socket,
                        504,
                        "request_timeout",
                        "request timed out",
                    )?;
                    return Ok(Some(true));
                }
            }
        } else {
            None
        };
        match handle_runtime_proxy_websocket_frame(
            self.id,
            &mut self.socket,
            &self.handshake,
            &scoped,
            &mut self.state,
            frame,
        )? {
            RuntimeWebsocketSessionReadAction::Continue => Ok(Some(true)),
            RuntimeWebsocketSessionReadAction::Break => Ok(None),
            RuntimeWebsocketSessionReadAction::Return(result) => {
                result?;
                Ok(None)
            }
        }
    }
}

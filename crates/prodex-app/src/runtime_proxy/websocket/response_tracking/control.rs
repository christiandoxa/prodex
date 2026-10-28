//! Pump connection-bound cancellation alongside upstream response frames.
//! A control frame cannot enter profile selection or the retry/rotation loop.
use super::super::control::{
    interrupt_acknowledgement, interrupt_response_id, poll_local_frame,
    send_interrupt_on_owned_socket,
};
use super::{RuntimeWebsocketResponseLoop, RuntimeWebsocketTextTerminal};
use anyhow::Result;
use std::time::{Duration, Instant};
use tungstenite::Message;

#[derive(Default)]
pub(super) struct ResponseInterrupt {
    pending: Option<(String, Instant)>,
    forwarded: bool,
    terminal: Option<RuntimeWebsocketTextTerminal>,
}

impl ResponseInterrupt {
    pub(super) fn pump(&mut self, flow: &mut RuntimeWebsocketResponseLoop<'_>) -> Result<()> {
        if let Some((_, deadline)) = &self.pending {
            anyhow::ensure!(
                Instant::now() < *deadline,
                "runtime websocket interruption acknowledgement timed out"
            );
        }
        let Some(frame) = poll_local_frame(flow.local_socket, flow.websocket_session)? else {
            return Ok(());
        };
        match frame {
            Message::Text(text) => {
                let Some(id) = interrupt_response_id(&text)? else {
                    // Preserve the next complete request in wire order. Do not
                    // read another until this response (and its ack) completes.
                    flow.websocket_session.pending_local_frame = Some(Message::Text(text));
                    return Ok(());
                };
                anyhow::ensure!(!self.forwarded, "duplicate response interruption");
                anyhow::ensure!(
                    flow.committed_response_ids.contains(&id),
                    "runtime websocket interrupt does not own the active response"
                );
                let timeout = Duration::from_millis(
                    flow.shared
                        .runtime_config
                        .tuning
                        .websocket_precommit_progress_timeout_ms
                        .max(1),
                );
                let deadline = Instant::now() + timeout;
                send_interrupt_on_owned_socket(&mut flow.upstream_socket, text, timeout)?;
                self.forwarded = true;
                self.pending = Some((id, deadline));
                super::runtime_proxy_log(
                    flow.shared,
                    format!(
                        "request={} websocket_interrupt_forwarded transport=active rotated=false",
                        flow.request_id
                    ),
                );
            }
            Message::Ping(payload) => flow.local_socket.send(Message::Pong(payload))?,
            Message::Pong(_) | Message::Frame(_) => {}
            Message::Close(frame) => {
                let _ = flow.upstream_socket.close(frame);
                anyhow::bail!("runtime websocket client disconnected during its response");
            }
            Message::Binary(_) => anyhow::bail!("unexpected binary Responses control message"),
        }
        Ok(())
    }

    pub(super) fn observe_ack(
        &mut self,
        flow: &mut RuntimeWebsocketResponseLoop<'_>,
        text: &str,
    ) -> Result<bool> {
        let Some((id, _)) = &self.pending else {
            return Ok(false);
        };
        if !interrupt_acknowledgement(text, id)? {
            return Ok(false);
        }
        // Forward the actual acknowledgement; never manufacture success.
        flow.forward_text(text)?;
        self.pending = None;
        Ok(true)
    }

    pub(super) fn defer_terminal(
        &mut self,
        terminal: RuntimeWebsocketTextTerminal,
    ) -> Option<RuntimeWebsocketTextTerminal> {
        if self.pending.is_some()
            && !terminal.reset_upstream_socket
            && !terminal.committed_previous_response_not_found
        {
            self.terminal = Some(terminal);
            None
        } else {
            Some(terminal)
        }
    }

    pub(super) fn take_terminal(&mut self) -> Option<RuntimeWebsocketTextTerminal> {
        self.terminal.take()
    }
}

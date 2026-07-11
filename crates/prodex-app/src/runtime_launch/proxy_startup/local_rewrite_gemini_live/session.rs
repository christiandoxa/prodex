//! Gemini Live websocket session pumps and translated event forwarding.

use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::GEMINI_LIVE_IDLE_SLEEP;
use super::local_rewrite_gemini_live_translation::RuntimeGeminiLiveState;
use crate::{
    RuntimeUpstreamWebSocket, WsMessage, WsSocket, runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_structured_log_message, runtime_set_upstream_websocket_io_timeout,
};
use anyhow::{Context, Result};
use prodex_provider_core::gemini_provider_core_live_binary_frame_error;
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;

pub(super) fn runtime_gemini_live_session<S>(
    request_id: u64,
    local_socket: &mut WsSocket<S>,
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<()>
where
    S: Read + Write,
{
    let mut state = RuntimeGeminiLiveState::new(request_id);
    loop {
        match local_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let translated = state.translate_client_message(text.as_ref())?;
                for event in translated.local_events {
                    runtime_gemini_live_send_json(local_socket, event)?;
                }
                for message in translated.upstream_messages {
                    upstream_socket
                        .send(WsMessage::Text(message.to_string().into()))
                        .context("failed to send Gemini Live upstream message")?;
                }
                let timeout = if translated.wait_for_setup {
                    Duration::from_secs(15)
                } else if translated.wait_for_turn {
                    Duration::from_secs(60)
                } else {
                    Duration::from_millis(10)
                };
                runtime_gemini_live_drain_upstream(
                    upstream_socket,
                    local_socket,
                    &mut state,
                    timeout,
                    translated.wait_for_setup,
                    translated.wait_for_turn,
                )?;
            }
            Ok(WsMessage::Ping(payload)) => {
                local_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to Gemini Live local ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = upstream_socket.close(frame.clone());
                let _ = local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Binary(_)) => {
                runtime_gemini_live_send_json(
                    local_socket,
                    gemini_provider_core_live_binary_frame_error(),
                )?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Ok(());
            }
            Err(err) => return Err(anyhow::anyhow!("Gemini Live local websocket failed: {err}")),
        }
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_gemini_live_frame",
                [runtime_proxy_log_field("request", request_id.to_string())],
            ),
        );
    }
}

pub(super) fn runtime_gemini_live_duplex_session<S>(
    request_id: u64,
    local_socket: &mut WsSocket<S>,
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<()>
where
    S: Read + Write,
{
    let mut state = RuntimeGeminiLiveState::new(request_id);
    loop {
        let mut progressed = false;
        match local_socket.read() {
            Ok(WsMessage::Text(text)) => {
                progressed = true;
                let translated = state.translate_client_message(text.as_ref())?;
                for event in translated.local_events {
                    runtime_gemini_live_send_json(local_socket, event)?;
                }
                for message in translated.upstream_messages {
                    upstream_socket
                        .send(WsMessage::Text(message.to_string().into()))
                        .context("failed to send Gemini Live upstream message")?;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                progressed = true;
                local_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to Gemini Live local ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = upstream_socket.close(frame.clone());
                let _ = local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Binary(_)) => {
                progressed = true;
                runtime_gemini_live_send_json(
                    local_socket,
                    gemini_provider_core_live_binary_frame_error(),
                )?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                progressed = true;
            }
            Err(err) if crate::runtime_websocket_timeout_error(&err) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Ok(());
            }
            Err(err) => return Err(anyhow::anyhow!("Gemini Live local websocket failed: {err}")),
        }

        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                progressed = true;
                let translated = state.translate_server_message(text.as_ref())?;
                for event in translated.events {
                    runtime_gemini_live_send_json(local_socket, event)?;
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                progressed = true;
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to Gemini Live upstream ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Binary(_)) | Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                progressed = true;
            }
            Err(err) if crate::runtime_websocket_timeout_error(&err) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Ok(());
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "Gemini Live upstream websocket failed: {err}"
                ));
            }
        }

        if !progressed {
            thread::sleep(GEMINI_LIVE_IDLE_SLEEP);
        } else {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_gemini_live_duplex_pump",
                    [runtime_proxy_log_field("request", request_id.to_string())],
                ),
            );
        }
    }
}

fn runtime_gemini_live_drain_upstream<S>(
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    local_socket: &mut WsSocket<S>,
    state: &mut RuntimeGeminiLiveState,
    timeout: Duration,
    stop_on_setup: bool,
    stop_on_turn: bool,
) -> Result<()>
where
    S: Read + Write,
{
    runtime_set_upstream_websocket_io_timeout(upstream_socket, Some(timeout))
        .context("failed to set Gemini Live drain timeout")?;
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let translated = state.translate_server_message(text.as_ref())?;
                for event in translated.events {
                    runtime_gemini_live_send_json(local_socket, event)?;
                }
                if (stop_on_setup && translated.setup_complete)
                    || (stop_on_turn && translated.turn_complete)
                {
                    return Ok(());
                }
            }
            Ok(WsMessage::Ping(payload)) => {
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to Gemini Live upstream ping")?;
            }
            Ok(WsMessage::Close(frame)) => {
                let _ = local_socket.close(frame);
                return Ok(());
            }
            Ok(WsMessage::Binary(_)) | Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Err(err) if crate::runtime_websocket_timeout_error(&err) => return Ok(()),
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Ok(());
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "Gemini Live upstream websocket failed: {err}"
                ));
            }
        }
    }
}

fn runtime_gemini_live_send_json<S>(
    socket: &mut WsSocket<S>,
    value: serde_json::Value,
) -> Result<()>
where
    S: Read + Write,
{
    socket
        .send(WsMessage::Text(value.to_string().into()))
        .context("failed to send translated Gemini Live event")
}

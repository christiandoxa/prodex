use super::gemini_rewrite::RuntimeGeminiAuth;
use super::local_rewrite::{RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared};
use super::local_rewrite_gemini::runtime_gemini_live_auth_attempts;
use crate::{
    RuntimeUpstreamWebSocket, TinyHeader, TinyResponse, TinyStatusCode, WsMessage, WsRole,
    WsSocket, acquire_runtime_proxy_active_request_slot_with_wait,
    build_runtime_proxy_text_response, derive_accept_key, mark_runtime_proxy_local_overload,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_next_request_id,
    runtime_proxy_structured_log_message, runtime_set_upstream_websocket_io_timeout,
};
use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tungstenite::client::IntoClientRequest;

const GEMINI_LIVE_WEBSOCKET_URL: &str = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";
pub(super) const GEMINI_LIVE_DEFAULT_MODEL: &str = "gemini-3.1-flash-live-preview";
const GEMINI_LIVE_PUMP_TIMEOUT: Duration = Duration::from_millis(20);
const GEMINI_LIVE_IDLE_SLEEP: Duration = Duration::from_millis(5);
const GEMINI_LIVE_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

#[path = "local_rewrite_gemini_live_audio.rs"]
mod local_rewrite_gemini_live_audio;
#[path = "local_rewrite_gemini_live_translation.rs"]
mod local_rewrite_gemini_live_translation;
use local_rewrite_gemini_live_translation::RuntimeGeminiLiveState;

pub(super) fn runtime_gemini_live_default_model() -> &'static str {
    GEMINI_LIVE_DEFAULT_MODEL
}

pub(super) fn spawn_runtime_gemini_live_sidecar(
    shared: RuntimeLocalRewriteProxyShared,
    shutdown: Arc<AtomicBool>,
    worker_threads: &mut Vec<JoinHandle<()>>,
) -> Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .context("failed to bind Gemini Live sidecar websocket listener")?;
    listener
        .set_nonblocking(true)
        .context("failed to configure Gemini Live sidecar listener")?;
    let listen_addr = listener
        .local_addr()
        .context("failed to read Gemini Live sidecar listener address")?;
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_live_sidecar_started",
            [runtime_proxy_log_field(
                "listen_addr",
                listen_addr.to_string(),
            )],
        ),
    );
    worker_threads.push(thread::spawn(move || {
        while !shutdown.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _peer)) => {
                    let request_id = runtime_proxy_next_request_id(&shared.runtime_shared);
                    let guard = match acquire_runtime_proxy_active_request_slot_with_wait(
                        &shared.runtime_shared,
                        "websocket",
                        "/v1/realtime",
                    ) {
                        Ok(guard) => guard,
                        Err(_) => {
                            mark_runtime_proxy_local_overload(
                                &shared.runtime_shared,
                                "gemini_live_sidecar_admission",
                            );
                            continue;
                        }
                    };
                    let result = crate::runtime_panic::catch_runtime_unwind_silently(|| {
                        if let Err(err) =
                            handle_runtime_gemini_live_tcp_stream(request_id, stream, &shared)
                        {
                            runtime_proxy_log(
                                &shared.runtime_shared,
                                runtime_proxy_structured_log_message(
                                    "local_rewrite_gemini_live_sidecar_error",
                                    [
                                        runtime_proxy_log_field("request", request_id.to_string()),
                                        runtime_proxy_log_field("error", format!("{err:#}")),
                                    ],
                                ),
                            );
                        }
                    });
                    drop(guard);
                    if let Err(panic) = result {
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            format!(
                                "runtime_proxy_worker_panic lane=gemini_live_sidecar panic={}",
                                crate::runtime_panic::runtime_panic_payload_label(panic.as_ref())
                            ),
                        );
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(GEMINI_LIVE_IDLE_SLEEP);
                }
                Err(err) => {
                    runtime_proxy_log(
                        &shared.runtime_shared,
                        runtime_proxy_structured_log_message(
                            "local_rewrite_gemini_live_sidecar_accept_error",
                            [runtime_proxy_log_field("error", err.to_string())],
                        ),
                    );
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }));
    Ok(listen_addr)
}

pub(super) fn handle_runtime_gemini_live_websocket_request(
    request_id: u64,
    request: tiny_http::Request,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } = &shared.provider else {
        let _ = request.respond(build_runtime_proxy_text_response(
            501,
            "Gemini Live is only available for the Gemini provider.",
        ));
        return;
    };
    let attempts = match runtime_gemini_live_auth_attempts(auth, shared) {
        Ok(attempts) => attempts,
        Err(err) => {
            let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
            return;
        }
    };
    let mut selected = None;
    let mut last_error = None;
    for (profile_name, auth) in attempts {
        match runtime_gemini_live_connect(&auth) {
            Ok(socket) => {
                selected = Some((profile_name, socket));
                break;
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }
    let Some((profile_name, mut upstream_socket)) = selected else {
        let message = last_error
            .map(|err| format!("failed to connect Gemini Live upstream: {err:#}"))
            .unwrap_or_else(|| "no Gemini Live auth attempts were available".to_string());
        let _ = request.respond(build_runtime_proxy_text_response(502, &message));
        return;
    };
    let Some(websocket_key) = runtime_gemini_live_websocket_key(&request) else {
        let _ = request.respond(build_runtime_proxy_text_response(
            400,
            "Missing Sec-WebSocket-Key header for Gemini Live.",
        ));
        return;
    };
    let response = match runtime_gemini_live_upgrade_response(&websocket_key) {
        Ok(response) => response,
        Err(err) => {
            let _ = request.respond(build_runtime_proxy_text_response(500, &err.to_string()));
            return;
        }
    };
    let upgraded = request.upgrade("websocket", response);
    let mut local_socket = WsSocket::from_raw_socket(upgraded, WsRole::Server, None);
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_live_connected",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", profile_name.as_str()),
            ],
        ),
    );
    if let Err(err) =
        runtime_gemini_live_session(request_id, &mut local_socket, &mut upstream_socket, shared)
    {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_gemini_live_error",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("profile", profile_name.as_str()),
                    runtime_proxy_log_field("error", format!("{err:#}")),
                ],
            ),
        );
        let _ = local_socket.send(WsMessage::Text(
            serde_json::json!({
                "type": "error",
                "error": {
                    "type": "provider_stream_error",
                    "message": err.to_string(),
                }
            })
            .to_string()
            .into(),
        ));
    }
    let _ = upstream_socket.close(None);
    let _ = local_socket.close(None);
}

fn runtime_gemini_live_connect(auth: &RuntimeGeminiAuth) -> Result<RuntimeUpstreamWebSocket> {
    let endpoint = std::env::var("PRODEX_GEMINI_LIVE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| GEMINI_LIVE_WEBSOCKET_URL.to_string());
    let url = match auth {
        RuntimeGeminiAuth::ApiKey { api_key } => {
            let separator = if endpoint.contains('?') { '&' } else { '?' };
            format!("{endpoint}{separator}key={api_key}")
        }
        RuntimeGeminiAuth::OAuth { .. } => endpoint,
    };
    let mut request = url
        .into_client_request()
        .context("failed to build Gemini Live websocket request")?;
    if let RuntimeGeminiAuth::OAuth { access_token, .. } = auth {
        request.headers_mut().insert(
            tungstenite::http::header::AUTHORIZATION,
            format!("Bearer {access_token}")
                .parse()
                .context("failed to encode Gemini Live OAuth header")?,
        );
    }
    let (mut socket, _) =
        tungstenite::connect(request).context("failed to connect Gemini Live websocket")?;
    runtime_set_upstream_websocket_io_timeout(&mut socket, Some(Duration::from_secs(30)))
        .context("failed to configure Gemini Live websocket timeout")?;
    Ok(socket)
}

fn handle_runtime_gemini_live_tcp_stream(
    request_id: u64,
    stream: TcpStream,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<()> {
    stream
        .set_read_timeout(Some(GEMINI_LIVE_HANDSHAKE_TIMEOUT))
        .context("failed to configure Gemini Live local handshake read timeout")?;
    stream
        .set_write_timeout(Some(GEMINI_LIVE_HANDSHAKE_TIMEOUT))
        .context("failed to configure Gemini Live local handshake write timeout")?;
    let mut local_socket =
        tungstenite::accept(stream).context("failed to accept Gemini Live sidecar websocket")?;
    local_socket
        .get_mut()
        .set_read_timeout(Some(GEMINI_LIVE_PUMP_TIMEOUT))
        .context("failed to configure Gemini Live local read timeout")?;
    local_socket
        .get_mut()
        .set_write_timeout(Some(GEMINI_LIVE_HANDSHAKE_TIMEOUT))
        .context("failed to configure Gemini Live local write timeout")?;

    let RuntimeLocalRewriteProviderOptions::Gemini { auth, .. } = &shared.provider else {
        let _ = local_socket.close(None);
        return Ok(());
    };
    let attempts = runtime_gemini_live_auth_attempts(auth, shared)?;
    let mut selected = None;
    let mut last_error = None;
    for (profile_name, auth) in attempts {
        match runtime_gemini_live_connect(&auth) {
            Ok(socket) => {
                selected = Some((profile_name, socket));
                break;
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }
    let Some((profile_name, mut upstream_socket)) = selected else {
        let message = last_error
            .map(|err| format!("failed to connect Gemini Live upstream: {err:#}"))
            .unwrap_or_else(|| "no Gemini Live auth attempts were available".to_string());
        let _ = local_socket.send(WsMessage::Text(
            serde_json::json!({
                "type": "error",
                "error": {
                    "type": "provider_stream_error",
                    "message": message,
                }
            })
            .to_string()
            .into(),
        ));
        let _ = local_socket.close(None);
        return Ok(());
    };
    runtime_set_upstream_websocket_io_timeout(&mut upstream_socket, Some(GEMINI_LIVE_PUMP_TIMEOUT))
        .context("failed to configure Gemini Live upstream pump timeout")?;
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_gemini_live_sidecar_connected",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("profile", profile_name.as_str()),
            ],
        ),
    );
    if let Err(err) = runtime_gemini_live_duplex_session(
        request_id,
        &mut local_socket,
        &mut upstream_socket,
        shared,
    ) {
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "local_rewrite_gemini_live_sidecar_session_error",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("profile", profile_name.as_str()),
                    runtime_proxy_log_field("error", format!("{err:#}")),
                ],
            ),
        );
        let _ = local_socket.send(WsMessage::Text(
            serde_json::json!({
                "type": "error",
                "error": {
                    "type": "provider_stream_error",
                    "message": err.to_string(),
                }
            })
            .to_string()
            .into(),
        ));
    }
    let _ = upstream_socket.close(None);
    let _ = local_socket.close(None);
    Ok(())
}

fn runtime_gemini_live_websocket_key(request: &tiny_http::Request) -> Option<String> {
    request.headers().iter().find_map(|header| {
        header
            .field
            .equiv("Sec-WebSocket-Key")
            .then(|| header.value.as_str().trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn runtime_gemini_live_upgrade_response(key: &str) -> Result<TinyResponse<std::io::Empty>> {
    let accept = derive_accept_key(key.as_bytes());
    let upgrade = TinyHeader::from_bytes("Upgrade", "websocket")
        .map_err(|err| anyhow::anyhow!("invalid websocket Upgrade header: {err:?}"))?;
    let connection = TinyHeader::from_bytes("Connection", "Upgrade")
        .map_err(|err| anyhow::anyhow!("invalid websocket Connection header: {err:?}"))?;
    let accept = TinyHeader::from_bytes("Sec-WebSocket-Accept", accept.as_bytes())
        .map_err(|err| anyhow::anyhow!("invalid websocket accept header: {err:?}"))?;
    Ok(TinyResponse::new_empty(TinyStatusCode(101))
        .with_header(upgrade)
        .with_header(connection)
        .with_header(accept))
}

fn runtime_gemini_live_session<S>(
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
                    serde_json::json!({
                        "type": "error",
                        "error": {
                            "type": "invalid_request_error",
                            "message": "Gemini Live bridge expects JSON text websocket frames.",
                        }
                    }),
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

fn runtime_gemini_live_duplex_session<S>(
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
                    serde_json::json!({
                        "type": "error",
                        "error": {
                            "type": "invalid_request_error",
                            "message": "Gemini Live bridge expects JSON text websocket frames.",
                        }
                    }),
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

#[cfg(test)]
#[path = "local_rewrite_gemini_live_tests.rs"]
mod local_rewrite_gemini_live_tests;

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
use base64::Engine;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tungstenite::client::IntoClientRequest;

const GEMINI_LIVE_WEBSOCKET_URL: &str = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";
const GEMINI_LIVE_DEFAULT_MODEL: &str = "gemini-3.1-flash-live-preview";
const GEMINI_LIVE_AUDIO_RATE: u64 = 24_000;
const GEMINI_LIVE_PUMP_TIMEOUT: Duration = Duration::from_millis(20);
const GEMINI_LIVE_IDLE_SLEEP: Duration = Duration::from_millis(5);
const GEMINI_LIVE_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

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

struct RuntimeGeminiLiveClientTranslation {
    upstream_messages: Vec<serde_json::Value>,
    local_events: Vec<serde_json::Value>,
    wait_for_setup: bool,
    wait_for_turn: bool,
}

struct RuntimeGeminiLiveServerTranslation {
    events: Vec<serde_json::Value>,
    setup_complete: bool,
    turn_complete: bool,
}

struct RuntimeGeminiLiveState {
    request_id: u64,
    turn_sequence: u64,
    response_id: String,
    item_id: String,
    response_created: bool,
    input_audio_format: RuntimeGeminiLiveAudioFormat,
    input_audio_rate: u64,
    output_audio_rate: u64,
    input_transcript: String,
    output_transcript: String,
    output_text: String,
    tool_names_by_call_id: HashMap<String, String>,
    suppress_current_turn: bool,
}

impl RuntimeGeminiLiveState {
    fn new(request_id: u64) -> Self {
        Self {
            request_id,
            turn_sequence: 1,
            response_id: format!("resp_gemini_live_{request_id}"),
            item_id: format!("item_gemini_live_{request_id}"),
            response_created: false,
            input_audio_format: RuntimeGeminiLiveAudioFormat::Pcm16,
            input_audio_rate: GEMINI_LIVE_AUDIO_RATE,
            output_audio_rate: GEMINI_LIVE_AUDIO_RATE,
            input_transcript: String::new(),
            output_transcript: String::new(),
            output_text: String::new(),
            tool_names_by_call_id: HashMap::new(),
            suppress_current_turn: false,
        }
    }

    fn translate_client_message(
        &mut self,
        text: &str,
    ) -> Result<RuntimeGeminiLiveClientTranslation> {
        let value: serde_json::Value =
            serde_json::from_str(text).context("failed to parse Codex realtime JSON")?;
        let message_type = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let mut upstream_messages = Vec::new();
        let mut local_events = Vec::new();
        let mut wait_for_setup = false;
        let mut wait_for_turn = false;
        match message_type {
            "session.update" => {
                let session = value
                    .get("session")
                    .and_then(serde_json::Value::as_object)
                    .context("Codex realtime session.update is missing session")?;
                let input_audio = runtime_gemini_live_session_audio_config(session, "input")
                    .or_else(|| {
                        runtime_gemini_live_legacy_session_audio_config(
                            session,
                            "input_audio_format",
                            "inputAudioFormat",
                        )
                    })
                    .unwrap_or(RuntimeGeminiLiveAudioConfig {
                        format: RuntimeGeminiLiveAudioFormat::Pcm16,
                        rate: GEMINI_LIVE_AUDIO_RATE,
                    });
                self.input_audio_format = input_audio.format;
                self.input_audio_rate = input_audio.rate;
                if let Some(output_audio) =
                    runtime_gemini_live_session_audio_config(session, "output").or_else(|| {
                        runtime_gemini_live_legacy_session_audio_config(
                            session,
                            "output_audio_format",
                            "outputAudioFormat",
                        )
                    })
                {
                    self.output_audio_rate = output_audio.rate;
                }
                upstream_messages.push(runtime_gemini_live_setup_message(session));
                wait_for_setup = true;
            }
            "input_audio_buffer.append" => {
                let audio = value
                    .get("audio")
                    .and_then(serde_json::Value::as_str)
                    .context("Codex realtime audio append is missing audio")?;
                let audio = self.translate_input_audio(audio)?;
                upstream_messages.push(serde_json::json!({
                    "realtime_input": {
                        "audio": {
                            "data": audio.data,
                            "mime_type": audio.mime_type,
                        }
                    }
                }));
            }
            "input_audio_buffer.commit" => {
                upstream_messages.push(serde_json::json!({
                    "realtime_input": {
                        "audio_stream_end": true,
                    }
                }));
                wait_for_turn = true;
            }
            "input_audio_buffer.clear" => {
                self.input_transcript.clear();
                local_events.push(serde_json::json!({
                    "type": "input_audio_buffer.cleared",
                }));
            }
            "response.create" => {
                local_events.extend(self.ensure_response_created());
                wait_for_turn = true;
            }
            "response.cancel" => {
                self.suppress_current_turn = true;
                self.response_created = false;
                self.input_transcript.clear();
                self.output_transcript.clear();
                self.output_text.clear();
                local_events.push(serde_json::json!({
                    "type": "response.cancelled",
                    "response": {"id": self.response_id},
                }));
            }
            "conversation.item.truncate" => {
                local_events.push(serde_json::json!({
                    "type": "conversation.item.truncated",
                    "item_id": value.get("item_id").cloned().unwrap_or(serde_json::Value::Null),
                    "content_index": value.get("content_index").cloned().unwrap_or(serde_json::Value::Null),
                    "audio_end_ms": value.get("audio_end_ms").cloned().unwrap_or(serde_json::Value::Null),
                }));
            }
            "conversation.item.delete" => {
                local_events.push(serde_json::json!({
                    "type": "conversation.item.deleted",
                    "item_id": value.get("item_id").cloned().unwrap_or(serde_json::Value::Null),
                }));
            }
            "conversation.item.create" | "conversation.handoff.append" => {
                if let Some(message) = runtime_gemini_live_client_content_message(&value) {
                    upstream_messages.push(message);
                    wait_for_turn = true;
                }
                if let Some((call_id, message)) =
                    runtime_gemini_live_tool_response_message(&value, &self.tool_names_by_call_id)
                {
                    upstream_messages.push(message);
                    self.tool_names_by_call_id.remove(&call_id);
                    wait_for_turn = true;
                }
            }
            "response.processed" => {}
            _ => {
                local_events.push(serde_json::json!({
                    "type": "error",
                    "error": {
                        "type": "invalid_request_error",
                        "message": format!("Unsupported Codex realtime event type: {message_type}"),
                    }
                }));
            }
        }
        Ok(RuntimeGeminiLiveClientTranslation {
            upstream_messages,
            local_events,
            wait_for_setup,
            wait_for_turn,
        })
    }

    fn translate_server_message(
        &mut self,
        text: &str,
    ) -> Result<RuntimeGeminiLiveServerTranslation> {
        let value: serde_json::Value =
            serde_json::from_str(text).context("failed to parse Gemini Live JSON")?;
        let setup_complete =
            runtime_gemini_live_field(&value, "setupComplete", "setup_complete").is_some();
        let mut events = Vec::new();
        if setup_complete {
            events.push(serde_json::json!({
                "type": "session.updated",
                "session": {
                    "id": "sess_gemini_live",
                    "type": "realtime",
                }
            }));
        }
        if let Some(error) = value.get("error") {
            events.push(serde_json::json!({
                "type": "error",
            "error": error,
            }));
        }
        if self.suppress_current_turn {
            let turn_complete = runtime_gemini_live_server_turn_complete(&value);
            if turn_complete {
                self.suppress_current_turn = false;
                self.response_created = false;
                self.input_transcript.clear();
                self.output_transcript.clear();
                self.output_text.clear();
                self.advance_turn();
            }
            return Ok(RuntimeGeminiLiveServerTranslation {
                events,
                setup_complete,
                turn_complete,
            });
        }
        if let Some(tool_call) = runtime_gemini_live_field(&value, "toolCall", "tool_call") {
            events.extend(self.ensure_response_created());
            if let Some(function_calls) =
                runtime_gemini_live_field(tool_call, "functionCalls", "function_calls")
                    .and_then(serde_json::Value::as_array)
            {
                for function_call in function_calls {
                    let call_id = function_call
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("call_gemini_live");
                    let name = function_call
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("tool_call");
                    self.tool_names_by_call_id
                        .insert(call_id.to_string(), name.to_string());
                    let args = function_call
                        .get("args")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({}));
                    events.push(serde_json::json!({
                        "type": "conversation.item.done",
                        "item": {
                            "id": call_id,
                            "type": "function_call",
                            "name": name,
                            "call_id": call_id,
                            "arguments": serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string()),
                        }
                    }));
                }
            }
        }
        let mut turn_complete = false;
        if let Some(content) = runtime_gemini_live_field(&value, "serverContent", "server_content")
        {
            if runtime_gemini_live_field(content, "interrupted", "interrupted")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                events.push(serde_json::json!({
                    "type": "response.cancelled",
                    "response": {"id": self.response_id},
                }));
            }
            if let Some(text) = runtime_gemini_live_transcription_text(
                content,
                "inputTranscription",
                "input_transcription",
            ) {
                let delta = runtime_gemini_live_transcript_delta(&self.input_transcript, text);
                self.input_transcript = text.to_string();
                if !delta.is_empty() {
                    events.push(serde_json::json!({
                        "type": "conversation.item.input_audio_transcription.delta",
                        "item_id": self.item_id,
                        "delta": delta,
                    }));
                }
            }
            if let Some(text) = runtime_gemini_live_transcription_text(
                content,
                "outputTranscription",
                "output_transcription",
            ) {
                events.extend(self.ensure_response_created());
                let delta = runtime_gemini_live_transcript_delta(&self.output_transcript, text);
                self.output_transcript = text.to_string();
                if !delta.is_empty() {
                    events.push(serde_json::json!({
                        "type": "response.output_audio_transcript.delta",
                        "item_id": self.item_id,
                        "response_id": self.response_id,
                        "delta": delta,
                    }));
                }
            }
            if let Some(parts) = runtime_gemini_live_field(content, "modelTurn", "model_turn")
                .and_then(|turn| turn.get("parts"))
                .and_then(serde_json::Value::as_array)
            {
                events.extend(self.ensure_response_created());
                for part in parts {
                    if let Some(text) = part.get("text").and_then(serde_json::Value::as_str) {
                        self.output_text.push_str(text);
                        events.push(serde_json::json!({
                            "type": "response.output_text.delta",
                            "item_id": self.item_id,
                            "response_id": self.response_id,
                            "delta": text,
                        }));
                    }
                    if let Some(inline) =
                        runtime_gemini_live_field(part, "inlineData", "inline_data")
                        && let Some(data) = inline.get("data").and_then(serde_json::Value::as_str)
                    {
                        let sample_rate = inline
                            .get("mimeType")
                            .or_else(|| inline.get("mime_type"))
                            .and_then(serde_json::Value::as_str)
                            .and_then(runtime_gemini_live_audio_rate_from_mime)
                            .unwrap_or(self.output_audio_rate);
                        self.output_audio_rate = sample_rate;
                        events.push(serde_json::json!({
                            "type": "response.output_audio.delta",
                            "item_id": self.item_id,
                            "response_id": self.response_id,
                            "delta": data,
                            "sample_rate": sample_rate,
                            "channels": 1,
                        }));
                    }
                }
            }
            turn_complete = runtime_gemini_live_server_turn_complete(&value);
            if turn_complete {
                if !self.input_transcript.is_empty() {
                    events.push(serde_json::json!({
                        "type": "conversation.item.input_audio_transcription.completed",
                        "item_id": self.item_id,
                        "transcript": self.input_transcript,
                    }));
                }
                if !self.output_transcript.is_empty() {
                    events.push(serde_json::json!({
                        "type": "response.output_audio_transcript.done",
                        "item_id": self.item_id,
                        "response_id": self.response_id,
                        "transcript": self.output_transcript,
                    }));
                }
                if !self.output_text.is_empty() {
                    events.push(serde_json::json!({
                        "type": "response.output_text.done",
                        "item_id": self.item_id,
                        "response_id": self.response_id,
                        "text": self.output_text,
                    }));
                }
                events.push(serde_json::json!({
                    "type": "response.done",
                    "response": {"id": self.response_id},
                }));
                self.response_created = false;
                self.input_transcript.clear();
                self.output_transcript.clear();
                self.output_text.clear();
                self.advance_turn();
            }
        }
        Ok(RuntimeGeminiLiveServerTranslation {
            events,
            setup_complete,
            turn_complete,
        })
    }

    fn ensure_response_created(&mut self) -> Vec<serde_json::Value> {
        if self.response_created {
            return Vec::new();
        }
        self.response_created = true;
        vec![serde_json::json!({
            "type": "response.created",
            "response": {"id": self.response_id},
        })]
    }

    fn advance_turn(&mut self) {
        self.turn_sequence += 1;
        self.response_id = format!(
            "resp_gemini_live_{}_{}",
            self.request_id, self.turn_sequence
        );
        self.item_id = format!(
            "item_gemini_live_{}_{}",
            self.request_id, self.turn_sequence
        );
    }

    fn translate_input_audio(&self, audio: &str) -> Result<RuntimeGeminiLiveAudioPayload> {
        match self.input_audio_format {
            RuntimeGeminiLiveAudioFormat::Pcm16 => Ok(RuntimeGeminiLiveAudioPayload {
                data: audio.to_string(),
                mime_type: format!("audio/pcm;rate={}", self.input_audio_rate),
            }),
            RuntimeGeminiLiveAudioFormat::G711Ulaw | RuntimeGeminiLiveAudioFormat::G711Alaw => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(audio)
                    .context("failed to decode Codex realtime G.711 audio")?;
                let mut pcm = Vec::with_capacity(bytes.len().saturating_mul(2));
                for byte in bytes {
                    let sample = match self.input_audio_format {
                        RuntimeGeminiLiveAudioFormat::G711Ulaw => {
                            runtime_gemini_live_decode_ulaw(byte)
                        }
                        RuntimeGeminiLiveAudioFormat::G711Alaw => {
                            runtime_gemini_live_decode_alaw(byte)
                        }
                        RuntimeGeminiLiveAudioFormat::Pcm16 => unreachable!(),
                    };
                    pcm.extend_from_slice(&sample.to_le_bytes());
                }
                Ok(RuntimeGeminiLiveAudioPayload {
                    data: base64::engine::general_purpose::STANDARD.encode(pcm),
                    mime_type: format!("audio/pcm;rate={}", self.input_audio_rate),
                })
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeGeminiLiveAudioFormat {
    Pcm16,
    G711Ulaw,
    G711Alaw,
}

struct RuntimeGeminiLiveAudioConfig {
    format: RuntimeGeminiLiveAudioFormat,
    rate: u64,
}

struct RuntimeGeminiLiveAudioPayload {
    data: String,
    mime_type: String,
}

fn runtime_gemini_live_session_audio_config(
    session: &serde_json::Map<String, serde_json::Value>,
    direction: &str,
) -> Option<RuntimeGeminiLiveAudioConfig> {
    let fallback_key = format!("{direction}_audio_format");
    let format = session
        .get("audio")?
        .get(direction)?
        .get("format")
        .or_else(|| session.get(fallback_key.as_str()))?;
    runtime_gemini_live_audio_config_from_value(format)
}

fn runtime_gemini_live_legacy_session_audio_config(
    session: &serde_json::Map<String, serde_json::Value>,
    snake_key: &str,
    camel_key: &str,
) -> Option<RuntimeGeminiLiveAudioConfig> {
    session
        .get(snake_key)
        .or_else(|| session.get(camel_key))
        .and_then(runtime_gemini_live_audio_config_from_value)
}

fn runtime_gemini_live_audio_config_from_value(
    value: &serde_json::Value,
) -> Option<RuntimeGeminiLiveAudioConfig> {
    match value {
        serde_json::Value::String(value) => {
            let format = RuntimeGeminiLiveAudioFormat::from_name(value)?;
            Some(RuntimeGeminiLiveAudioConfig {
                format,
                rate: runtime_gemini_live_audio_rate_from_mime(value)
                    .unwrap_or_else(|| format.default_rate()),
            })
        }
        serde_json::Value::Object(object) => {
            let format = object
                .get("type")
                .or_else(|| object.get("name"))
                .or_else(|| object.get("format"))
                .and_then(serde_json::Value::as_str)
                .and_then(RuntimeGeminiLiveAudioFormat::from_name)
                .unwrap_or(RuntimeGeminiLiveAudioFormat::Pcm16);
            let rate = object
                .get("rate")
                .or_else(|| object.get("sample_rate"))
                .or_else(|| object.get("sampleRate"))
                .and_then(serde_json::Value::as_u64)
                .filter(|rate| *rate > 0)
                .unwrap_or_else(|| format.default_rate());
            Some(RuntimeGeminiLiveAudioConfig { format, rate })
        }
        _ => None,
    }
}

impl RuntimeGeminiLiveAudioFormat {
    fn from_name(value: &str) -> Option<Self> {
        let normalized = value
            .trim()
            .split(';')
            .next()
            .unwrap_or(value)
            .to_ascii_lowercase()
            .replace(['-', '/'], "_");
        match normalized.as_str() {
            "pcm16" | "pcm" | "audio_pcm" | "linear16" => Some(Self::Pcm16),
            "g711_ulaw" | "mulaw" | "ulaw" | "pcmu" | "audio_pcmu" => Some(Self::G711Ulaw),
            "g711_alaw" | "alaw" | "pcma" | "audio_pcma" => Some(Self::G711Alaw),
            _ => None,
        }
    }

    fn default_rate(self) -> u64 {
        match self {
            Self::Pcm16 => GEMINI_LIVE_AUDIO_RATE,
            Self::G711Ulaw | Self::G711Alaw => 8_000,
        }
    }
}

fn runtime_gemini_live_audio_rate_from_mime(mime_type: &str) -> Option<u64> {
    mime_type.split(';').find_map(|part| {
        let part = part.trim();
        let value = part.strip_prefix("rate=")?;
        value.parse::<u64>().ok().filter(|rate| *rate > 0)
    })
}

fn runtime_gemini_live_decode_ulaw(byte: u8) -> i16 {
    let value = !byte;
    let sign = value & 0x80;
    let exponent = (value >> 4) & 0x07;
    let mantissa = value & 0x0f;
    let sample = ((((mantissa as i32) << 3) + 0x84) << exponent) - 0x84;
    if sign != 0 {
        -(sample as i16)
    } else {
        sample as i16
    }
}

fn runtime_gemini_live_decode_alaw(byte: u8) -> i16 {
    let value = byte ^ 0x55;
    let sign = value & 0x80;
    let exponent = (value & 0x70) >> 4;
    let mantissa = value & 0x0f;
    let sample = if exponent == 0 {
        ((mantissa as i32) << 4) + 8
    } else {
        (((mantissa as i32) << 4) + 0x108) << (exponent - 1)
    };
    if sign != 0 {
        sample as i16
    } else {
        -(sample as i16)
    }
}

fn runtime_gemini_live_setup_message(
    session: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let configured_model = std::env::var("PRODEX_GEMINI_LIVE_MODEL").ok();
    let requested_model = session
        .get("model")
        .and_then(serde_json::Value::as_str)
        .filter(|model| model.starts_with("gemini-"));
    let model = configured_model
        .as_deref()
        .filter(|model| !model.trim().is_empty())
        .or(requested_model)
        .unwrap_or(GEMINI_LIVE_DEFAULT_MODEL);
    let model = model.strip_prefix("models/").unwrap_or(model);
    let output_modalities = session
        .get("output_modalities")
        .and_then(serde_json::Value::as_array)
        .map(|modalities| {
            modalities
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|modality| modality.to_ascii_uppercase())
                .collect::<Vec<_>>()
        })
        .filter(|modalities| !modalities.is_empty())
        .unwrap_or_else(|| vec!["AUDIO".to_string()]);
    let mut setup = serde_json::json!({
        "model": format!("models/{model}"),
        "generation_config": {
            "response_modalities": output_modalities,
        },
        "input_audio_transcription": {},
        "output_audio_transcription": {},
    });
    if let Some(instructions) = session
        .get("instructions")
        .and_then(serde_json::Value::as_str)
        .filter(|instructions| !instructions.trim().is_empty())
    {
        setup["system_instruction"] = serde_json::json!({
            "parts": [{"text": instructions}],
        });
    }
    if let Some(tools) = session
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(runtime_gemini_live_function_declaration)
                .collect::<Vec<_>>()
        })
        .filter(|tools| !tools.is_empty())
    {
        setup["tools"] = serde_json::json!([{"function_declarations": tools}]);
    }
    serde_json::json!({"setup": setup})
}

fn runtime_gemini_live_function_declaration(tool: &serde_json::Value) -> Option<serde_json::Value> {
    if tool.get("type").and_then(serde_json::Value::as_str) != Some("function") {
        return None;
    }
    let name = tool.get("name").and_then(serde_json::Value::as_str)?;
    let mut declaration = serde_json::json!({
        "name": name,
        "parameters": tool
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"type": "object"})),
    });
    if let Some(description) = tool.get("description").filter(|value| !value.is_null()) {
        declaration["description"] = description.clone();
    }
    Some(declaration)
}

fn runtime_gemini_live_client_content_message(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    let item = value.get("item")?;
    if item.get("type").and_then(serde_json::Value::as_str) != Some("message") {
        return None;
    }
    let text = item
        .get("content")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|content| content.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then(|| {
        serde_json::json!({
            "client_content": {
                "turns": [{"role": "user", "parts": [{"text": text}]}],
                "turn_complete": true,
            }
        })
    })
}

fn runtime_gemini_live_tool_response_message(
    value: &serde_json::Value,
    tool_names_by_call_id: &HashMap<String, String>,
) -> Option<(String, serde_json::Value)> {
    let item = value.get("item")?;
    if item.get("type").and_then(serde_json::Value::as_str) != Some("function_call_output") {
        return None;
    }
    let call_id = item.get("call_id").and_then(serde_json::Value::as_str)?;
    let output = item
        .get("output")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::String(String::new()));
    let name = tool_names_by_call_id
        .get(call_id)
        .map(String::as_str)
        .unwrap_or(call_id);
    Some((
        call_id.to_string(),
        serde_json::json!({
            "tool_response": {
                "function_responses": [{
                    "id": call_id,
                    "name": name,
                    "response": {"output": output},
                }]
            }
        }),
    ))
}

fn runtime_gemini_live_field<'a>(
    value: &'a serde_json::Value,
    camel: &str,
    snake: &str,
) -> Option<&'a serde_json::Value> {
    value.get(camel).or_else(|| value.get(snake))
}

fn runtime_gemini_live_server_turn_complete(value: &serde_json::Value) -> bool {
    runtime_gemini_live_field(value, "serverContent", "server_content")
        .and_then(|content| runtime_gemini_live_field(content, "turnComplete", "turn_complete"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn runtime_gemini_live_transcription_text<'a>(
    content: &'a serde_json::Value,
    camel: &str,
    snake: &str,
) -> Option<&'a str> {
    runtime_gemini_live_field(content, camel, snake)?
        .get("text")
        .and_then(serde_json::Value::as_str)
}

fn runtime_gemini_live_transcript_delta<'a>(previous: &str, current: &'a str) -> &'a str {
    current.strip_prefix(previous).unwrap_or(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_live_translates_codex_session_audio_and_text() {
        let mut state = RuntimeGeminiLiveState::new(7);
        let setup = state
            .translate_client_message(
                &serde_json::json!({
                    "type": "session.update",
                    "session": {
                        "instructions": "Be concise",
                        "output_modalities": ["audio"],
                        "audio": {"input": {"format": {"rate": 16000}}},
                        "tools": [{
                            "type": "function",
                            "name": "background_agent",
                            "description": "Delegate work",
                            "parameters": {"type": "object"}
                        }]
                    }
                })
                .to_string(),
            )
            .unwrap();
        assert!(setup.wait_for_setup);
        assert_eq!(
            setup.upstream_messages[0]["setup"]["generation_config"]["response_modalities"][0],
            "AUDIO"
        );
        assert_eq!(
            setup.upstream_messages[0]["setup"]["tools"][0]["function_declarations"][0]["name"],
            "background_agent"
        );

        let audio = state
            .translate_client_message(
                &serde_json::json!({
                    "type": "input_audio_buffer.append",
                    "audio": "AAAA"
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(
            audio.upstream_messages[0]["realtime_input"]["audio"]["mime_type"],
            "audio/pcm;rate=16000"
        );
    }

    #[test]
    fn gemini_live_transcodes_g711_input_audio_to_pcm() {
        let mut state = RuntimeGeminiLiveState::new(8);
        state
            .translate_client_message(
                &serde_json::json!({
                    "type": "session.update",
                    "session": {
                        "input_audio_format": "g711_ulaw"
                    }
                })
                .to_string(),
            )
            .unwrap();
        let audio = base64::engine::general_purpose::STANDARD.encode([0xff_u8, 0x7f_u8]);
        let translated = state
            .translate_client_message(
                &serde_json::json!({
                    "type": "input_audio_buffer.append",
                    "audio": audio
                })
                .to_string(),
            )
            .unwrap();
        let upstream_audio = &translated.upstream_messages[0]["realtime_input"]["audio"];
        assert_eq!(upstream_audio["mime_type"], "audio/pcm;rate=8000");
        let pcm = base64::engine::general_purpose::STANDARD
            .decode(upstream_audio["data"].as_str().unwrap())
            .unwrap();
        assert_eq!(pcm.len(), 4);
    }

    #[test]
    fn gemini_live_translates_server_audio_transcripts_tools_and_turn_completion() {
        let mut state = RuntimeGeminiLiveState::new(9);
        let translated = state
            .translate_server_message(
                &serde_json::json!({
                    "serverContent": {
                        "inputTranscription": {"text": "hello"},
                        "outputTranscription": {"text": "hi"},
                        "modelTurn": {"parts": [
                            {"text": "hi"},
                            {"inlineData": {"data": "AAAA", "mimeType": "audio/pcm;rate=24000"}}
                        ]},
                        "turnComplete": true
                    },
                    "toolCall": {
                        "functionCalls": [{
                            "id": "call_bg",
                            "name": "background_agent",
                            "args": {"prompt": "work"}
                        }]
                    }
                })
                .to_string(),
            )
            .unwrap();
        assert!(translated.turn_complete);
        let serialized = serde_json::to_string(&translated.events).unwrap();
        assert!(serialized.contains("conversation.item.input_audio_transcription.delta"));
        assert!(serialized.contains("response.output_audio.delta"));
        assert!(serialized.contains("conversation.item.done"));
        assert!(serialized.contains("response.done"));
    }

    #[test]
    fn gemini_live_uses_server_audio_mime_rate_for_output_events() {
        let mut state = RuntimeGeminiLiveState::new(10);
        let translated = state
            .translate_server_message(
                &serde_json::json!({
                    "serverContent": {
                        "modelTurn": {"parts": [
                            {"inlineData": {"data": "AAAA", "mimeType": "audio/pcm;rate=16000"}}
                        ]}
                    }
                })
                .to_string(),
            )
            .unwrap();
        let audio = translated
            .events
            .iter()
            .find(|event| event["type"] == "response.output_audio.delta")
            .expect("audio delta event");
        assert_eq!(audio["sample_rate"], 16000);
    }

    #[test]
    fn gemini_live_translates_function_output_to_tool_response() {
        let mut state = RuntimeGeminiLiveState::new(11);
        state
            .translate_server_message(
                &serde_json::json!({
                    "toolCall": {
                        "functionCalls": [{
                            "id": "call_bg",
                            "name": "background_agent",
                            "args": {"prompt": "work"}
                        }]
                    }
                })
                .to_string(),
            )
            .unwrap();
        let translated = state
            .translate_client_message(
                &serde_json::json!({
                    "type": "conversation.item.create",
                    "item": {
                        "type": "function_call_output",
                        "call_id": "call_bg",
                        "output": "done"
                    }
                })
                .to_string(),
            )
            .unwrap();
        assert_eq!(
            translated.upstream_messages[0]["tool_response"]["function_responses"][0]["id"],
            "call_bg"
        );
        assert_eq!(
            translated.upstream_messages[0]["tool_response"]["function_responses"][0]["name"],
            "background_agent"
        );
        assert!(translated.wait_for_turn);
    }

    #[test]
    fn gemini_live_accepts_codex_housekeeping_events_without_errors() {
        let mut state = RuntimeGeminiLiveState::new(12);
        for (event, expected) in [
            (
                serde_json::json!({"type": "input_audio_buffer.clear"}),
                "input_audio_buffer.cleared",
            ),
            (
                serde_json::json!({
                    "type": "conversation.item.truncate",
                    "item_id": "item_1",
                    "content_index": 0,
                    "audio_end_ms": 42
                }),
                "conversation.item.truncated",
            ),
            (
                serde_json::json!({
                    "type": "conversation.item.delete",
                    "item_id": "item_1"
                }),
                "conversation.item.deleted",
            ),
        ] {
            let translated = state
                .translate_client_message(&event.to_string())
                .expect("housekeeping event should translate");
            assert_eq!(translated.local_events[0]["type"], expected);
            assert!(translated.upstream_messages.is_empty());
        }
    }

    #[test]
    fn gemini_live_suppresses_late_server_output_after_client_cancel() {
        let mut state = RuntimeGeminiLiveState::new(14);
        let created = state
            .translate_client_message(&serde_json::json!({"type": "response.create"}).to_string())
            .unwrap();
        assert_eq!(created.local_events[0]["type"], "response.created");
        let cancelled = state
            .translate_client_message(&serde_json::json!({"type": "response.cancel"}).to_string())
            .unwrap();
        assert_eq!(cancelled.local_events[0]["type"], "response.cancelled");

        let late_delta = state
            .translate_server_message(
                &serde_json::json!({
                    "serverContent": {
                        "modelTurn": {"parts": [{"text": "late"}]},
                        "turnComplete": false
                    }
                })
                .to_string(),
            )
            .unwrap();
        assert!(late_delta.events.is_empty());

        let late_done = state
            .translate_server_message(
                &serde_json::json!({
                    "serverContent": {
                        "modelTurn": {"parts": [{"text": "ignored"}]},
                        "turnComplete": true
                    }
                })
                .to_string(),
            )
            .unwrap();
        assert!(late_done.turn_complete);
        assert!(late_done.events.is_empty());

        let next = state
            .translate_server_message(
                &serde_json::json!({
                    "serverContent": {
                        "modelTurn": {"parts": [{"text": "next"}]},
                        "turnComplete": true
                    }
                })
                .to_string(),
            )
            .unwrap();
        let serialized = serde_json::to_string(&next.events).unwrap();
        assert!(serialized.contains("resp_gemini_live_14_2"));
        assert!(serialized.contains("next"));
    }

    #[test]
    fn gemini_live_uses_distinct_response_ids_per_turn() {
        let mut state = RuntimeGeminiLiveState::new(13);
        let first = state
            .translate_server_message(
                &serde_json::json!({
                    "serverContent": {
                        "modelTurn": {"parts": [{"text": "one"}]},
                        "turnComplete": true
                    }
                })
                .to_string(),
            )
            .unwrap();
        let second = state
            .translate_server_message(
                &serde_json::json!({
                    "serverContent": {
                        "modelTurn": {"parts": [{"text": "two"}]},
                        "turnComplete": true
                    }
                })
                .to_string(),
            )
            .unwrap();
        let first_serialized = serde_json::to_string(&first.events).unwrap();
        let second_serialized = serde_json::to_string(&second.events).unwrap();
        assert!(first_serialized.contains("resp_gemini_live_13"));
        assert!(second_serialized.contains("resp_gemini_live_13_2"));
    }
}

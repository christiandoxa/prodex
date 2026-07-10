use super::local_rewrite::{RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared};
use super::local_rewrite_gemini::runtime_gemini_live_auth_attempts;
use crate::{
    WsMessage, WsRole, WsSocket, acquire_runtime_proxy_active_request_slot_with_wait,
    build_runtime_proxy_text_response, mark_runtime_proxy_local_overload, runtime_proxy_log,
    runtime_proxy_log_field, runtime_proxy_next_request_id, runtime_proxy_structured_log_message,
    runtime_set_upstream_websocket_io_timeout,
};
use anyhow::{Context, Result};
use prodex_provider_core::gemini_provider_core_live_provider_stream_error;
use redaction::redaction_redact_secret_like_text;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;
const GEMINI_LIVE_WEBSOCKET_URL: &str = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";
pub(super) const GEMINI_LIVE_DEFAULT_MODEL: &str = "gemini-3.1-flash-live-preview";
const GEMINI_LIVE_PUMP_TIMEOUT: Duration = Duration::from_millis(20);
const GEMINI_LIVE_IDLE_SLEEP: Duration = Duration::from_millis(5);
const GEMINI_LIVE_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

const RUNTIME_GEMINI_LIVE_AUTH_FAILED_MESSAGE: &str =
    "Gemini Live authentication could not be prepared";
const RUNTIME_GEMINI_LIVE_UPGRADE_FAILED_MESSAGE: &str =
    "Gemini Live websocket upgrade could not be prepared";
const RUNTIME_GEMINI_LIVE_PROVIDER_STREAM_FAILED_MESSAGE: &str =
    "Gemini Live provider stream failed";

mod connection;
#[path = "local_rewrite_gemini_live_translation.rs"]
mod local_rewrite_gemini_live_translation;
mod session;

use self::connection::{
    runtime_gemini_live_connect, runtime_gemini_live_upgrade_response,
    runtime_gemini_live_websocket_key,
};
use self::session::{runtime_gemini_live_duplex_session, runtime_gemini_live_session};
#[cfg(test)]
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
                                        runtime_proxy_log_field(
                                            "error",
                                            runtime_gemini_live_redacted_log_error(&err),
                                        ),
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
                            [runtime_proxy_log_field(
                                "error",
                                runtime_gemini_live_redacted_log_text(&err.to_string()),
                            )],
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
        Err(_err) => {
            let _ = request.respond(build_runtime_proxy_text_response(
                502,
                RUNTIME_GEMINI_LIVE_AUTH_FAILED_MESSAGE,
            ));
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
        Err(_err) => {
            let _ = request.respond(build_runtime_proxy_text_response(
                500,
                RUNTIME_GEMINI_LIVE_UPGRADE_FAILED_MESSAGE,
            ));
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
                    runtime_proxy_log_field("error", runtime_gemini_live_redacted_log_error(&err)),
                ],
            ),
        );
        let _ = local_socket.send(WsMessage::Text(
            gemini_provider_core_live_provider_stream_error(
                RUNTIME_GEMINI_LIVE_PROVIDER_STREAM_FAILED_MESSAGE,
            )
            .to_string()
            .into(),
        ));
    }
    let _ = upstream_socket.close(None);
    let _ = local_socket.close(None);
}

fn runtime_gemini_live_redacted_log_error(error: &anyhow::Error) -> String {
    runtime_gemini_live_redacted_log_text(&format!("{error:#}"))
}

fn runtime_gemini_live_redacted_log_text(value: &str) -> String {
    redaction_redact_secret_like_text(value)
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
            gemini_provider_core_live_provider_stream_error(message)
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
                    runtime_proxy_log_field("error", runtime_gemini_live_redacted_log_error(&err)),
                ],
            ),
        );
        let _ = local_socket.send(WsMessage::Text(
            gemini_provider_core_live_provider_stream_error(
                RUNTIME_GEMINI_LIVE_PROVIDER_STREAM_FAILED_MESSAGE,
            )
            .to_string()
            .into(),
        ));
    }
    let _ = upstream_socket.close(None);
    let _ = local_socket.close(None);
    Ok(())
}

#[cfg(test)]
#[path = "local_rewrite_gemini_live_tests.rs"]
mod local_rewrite_gemini_live_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_live_error_messages_are_stable_and_redacted() {
        for message in [
            RUNTIME_GEMINI_LIVE_AUTH_FAILED_MESSAGE,
            RUNTIME_GEMINI_LIVE_UPGRADE_FAILED_MESSAGE,
            RUNTIME_GEMINI_LIVE_PROVIDER_STREAM_FAILED_MESSAGE,
        ] {
            assert!(!message.contains("api_key"));
            assert!(!message.contains("oauth"));
            assert!(!message.contains("wss://"));
            assert!(!message.contains("generativelanguage.googleapis.com"));
            assert!(!message.contains("WebSocket protocol error"));
        }
        assert_eq!(
            RUNTIME_GEMINI_LIVE_AUTH_FAILED_MESSAGE,
            "Gemini Live authentication could not be prepared"
        );
        assert_eq!(
            RUNTIME_GEMINI_LIVE_UPGRADE_FAILED_MESSAGE,
            "Gemini Live websocket upgrade could not be prepared"
        );
        assert_eq!(
            RUNTIME_GEMINI_LIVE_PROVIDER_STREAM_FAILED_MESSAGE,
            "Gemini Live provider stream failed"
        );
    }

    #[test]
    fn gemini_live_runtime_error_logs_redact_secret_like_material() {
        let raw = "failed to connect Gemini Live upstream: Authorization: Bearer fixture_live_notreal_12345 url=wss://example.test?api_key=sk-live-fixture-notreal-123456";

        let redacted = runtime_gemini_live_redacted_log_text(raw);

        assert!(redacted.contains("Authorization: Bearer <redacted>"));
        assert!(redacted.contains("api_key=<redacted>"));
        assert!(!redacted.contains("fixture_live_notreal_12345"));
        assert!(!redacted.contains("sk-live-fixture-notreal-123456"));
    }
}

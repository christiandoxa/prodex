use super::*;
mod connect;
mod response_tracking;
pub(crate) use connect::*;
pub(crate) use response_tracking::*;

mod session_state;
mod tcp_connect_executor;
mod unauthorized_recovery;

pub(in crate::runtime_proxy) use self::session_state::RuntimeWebsocketSessionState;
pub(crate) use self::session_state::acquire_runtime_profile_inflight_guard;
use self::tcp_connect_executor::*;
pub(super) use self::unauthorized_recovery::{
    RuntimeProfileUnauthorizedRecoveryStep,
    runtime_try_recover_profile_auth_from_unauthorized_steps,
};
use runtime_proxy_crate::{
    RuntimeWebsocketTarget, inspect_runtime_websocket_text_frame, runtime_interleave_socket_addrs,
    runtime_proxy_websocket_error_payload_text, runtime_realtime_websocket_terminal_event_kind,
    runtime_translate_precommit_previous_response_websocket_text_frame,
    runtime_translate_previous_response_websocket_text_frame, runtime_websocket_authority,
    runtime_websocket_error_payload_from_http_body, runtime_websocket_http_connect_request,
    runtime_websocket_no_proxy_value_matches, runtime_websocket_normalize_host,
    runtime_websocket_precommit_hold_promotion_allowed,
    runtime_websocket_precommit_hold_promotion_event_seen,
    runtime_websocket_precommit_transport_retry_allowed,
    runtime_websocket_proxy_authorization_header, runtime_websocket_proxy_env_keys,
    runtime_websocket_proxy_url_candidate, runtime_websocket_read_http_connect_response,
    runtime_websocket_target_from_parts,
};

pub(super) fn run_runtime_proxy_websocket_session(
    session_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<()> {
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    loop {
        match local_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let message_id = runtime_proxy_next_request_id(shared);
                let request_metadata = parse_runtime_websocket_request_metadata(text.as_ref());
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={message_id} websocket_session={session_id} inbound_text previous_response_id={:?} turn_state={:?} bytes={}",
                        request_metadata.previous_response_id,
                        request_metadata
                            .turn_state
                            .clone()
                            .or_else(|| runtime_request_turn_state(handshake_request)),
                        text.len()
                    ),
                );
                let compat_surface = runtime_detect_websocket_message_compatibility_surface(
                    handshake_request,
                    text.as_ref(),
                );
                runtime_proxy_log_request_compatibility(shared, message_id, &compat_surface);
                proxy_runtime_websocket_text_message(RuntimeWebsocketTextMessageInput {
                    session_id,
                    request_id: message_id,
                    local_socket,
                    handshake_request,
                    request_text: text.as_ref(),
                    request_metadata: &request_metadata,
                    shared,
                    websocket_session: &mut websocket_session,
                })?;
            }
            Ok(WsMessage::Binary(_)) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} inbound_binary_rejected"),
                );
                send_runtime_proxy_websocket_error(
                    local_socket,
                    400,
                    "invalid_request_error",
                    "Binary websocket messages are not supported by the runtime auto-rotate proxy.",
                )?;
            }
            Ok(WsMessage::Ping(payload)) => {
                local_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to runtime websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(frame)) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} local_close"),
                );
                websocket_session.close();
                let _ = local_socket.close(frame);
                break;
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} local_connection_closed"),
                );
                websocket_session.close();
                break;
            }
            Err(err) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} local_read_error={err}"),
                );
                websocket_session.close();
                return Err(anyhow::anyhow!(
                    "runtime websocket session ended unexpectedly: {err}"
                ));
            }
        }
    }

    Ok(())
}

pub(super) fn connect_runtime_proxy_upstream_websocket(
    request_id: u64,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<RuntimeWebsocketConnectResult> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .clone();
    let upstream_url = runtime_proxy_upstream_websocket_url(
        &runtime.upstream_base_url,
        &handshake_request.path_and_query,
    )?;
    let log_url = runtime_proxy_log_url(&upstream_url);
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
    loop {
        let auth = runtime_profile_usage_auth(shared, profile_name)?;
        let mut request = upstream_url
            .as_str()
            .into_client_request()
            .with_context(|| format!("failed to build runtime websocket request for {log_url}"))?;

        for (name, value) in runtime_forward_request_headers(
            handshake_request
                .headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        ) {
            if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
                continue;
            }
            if name.eq_ignore_ascii_case("cookie") {
                continue;
            }
            let Ok(header_name) = WsHeaderName::from_bytes(name.as_bytes()) else {
                continue;
            };
            let Ok(header_value) = WsHeaderValue::from_str(value) else {
                continue;
            };
            request.headers_mut().insert(header_name, header_value);
        }
        if let Some(turn_state) = turn_state_override {
            request.headers_mut().insert(
                WsHeaderName::from_static("x-codex-turn-state"),
                WsHeaderValue::from_str(turn_state)
                    .context("failed to encode websocket turn-state header")?,
            );
        }
        if let Some(cookie_header) = runtime_proxy_cookie_header_for_websocket(
            shared,
            profile_name,
            &upstream_url,
            &handshake_request.headers,
        ) && let Ok(cookie_header) = WsHeaderValue::from_str(&cookie_header)
        {
            request
                .headers_mut()
                .insert(WsHeaderName::from_static("cookie"), cookie_header);
        }

        request.headers_mut().insert(
            WsHeaderName::from_static("authorization"),
            WsHeaderValue::from_str(&format!("Bearer {}", auth.access_token))
                .context("failed to encode websocket authorization header")?,
        );
        if let Some(account_id) = auth.account_id.as_deref() {
            request.headers_mut().insert(
                WsHeaderName::from_static("chatgpt-account-id"),
                WsHeaderValue::from_str(account_id)
                    .context("failed to encode websocket account header")?,
            );
        }

        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "upstream_connect_start",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("url", log_url.as_str()),
                    runtime_proxy_log_field(
                        "turn_state_override",
                        format!("{turn_state_override:?}"),
                    ),
                ],
            ),
        );
        if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE") {
            let transport_error = anyhow::anyhow!("injected runtime websocket connect failure");
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Websocket,
                "websocket_connect",
                &transport_error,
            );
            return Err(transport_error);
        }
        let started_at = Instant::now();
        match connect_runtime_proxy_upstream_websocket_with_timeout(request_id, shared, request) {
            Ok((socket, response, selected_addr, resolved_addrs, attempted_addrs)) => {
                runtime_proxy_capture_websocket_cookies(
                    shared,
                    profile_name,
                    &upstream_url,
                    response.headers(),
                );
                return Ok(RuntimeWebsocketConnectResult::Connected {
                    socket,
                    turn_state: {
                        let turn_state = runtime_proxy_tungstenite_header_value(
                            response.headers(),
                            "x-codex-turn-state",
                        );
                        runtime_proxy_log(
                            shared,
                            runtime_proxy_structured_log_message(
                                "upstream_connect_ok",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field("transport", "websocket"),
                                    runtime_proxy_log_field("profile", profile_name),
                                    runtime_proxy_log_field(
                                        "status",
                                        response.status().as_u16().to_string(),
                                    ),
                                    runtime_proxy_log_field("addr", selected_addr.to_string()),
                                    runtime_proxy_log_field(
                                        "resolved_addrs",
                                        resolved_addrs.to_string(),
                                    ),
                                    runtime_proxy_log_field(
                                        "attempted_addrs",
                                        attempted_addrs.to_string(),
                                    ),
                                    runtime_proxy_log_field(
                                        "turn_state",
                                        format!("{turn_state:?}"),
                                    ),
                                ],
                            ),
                        );
                        note_runtime_profile_latency_observation(
                            shared,
                            profile_name,
                            RuntimeRouteKind::Websocket,
                            "connect",
                            started_at.elapsed().as_millis() as u64,
                        );
                        turn_state
                    },
                });
            }
            Err(WsError::Http(response)) => {
                runtime_proxy_capture_websocket_cookies(
                    shared,
                    profile_name,
                    &upstream_url,
                    response.headers(),
                );
                let status = response.status().as_u16();
                let body = response.body().clone().unwrap_or_default();
                if status == 401
                    && runtime_try_recover_profile_auth_from_unauthorized_steps(
                        request_id,
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        &mut recovery_steps,
                    )
                {
                    continue;
                }
                if (matches!(status, 401 | 403)
                    && (status == 401 || extract_runtime_proxy_quota_message(&body).is_none()))
                    || runtime_proxy_body_indicates_token_invalidated(&body)
                {
                    note_runtime_profile_auth_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        status,
                    );
                }
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "upstream_connect_http",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "websocket"),
                            runtime_proxy_log_field("profile", profile_name),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("body_bytes", body.len().to_string()),
                        ],
                    ),
                );
                if matches!(status, 402 | 403 | 429)
                    && extract_runtime_proxy_quota_message(&body).is_some()
                {
                    return Ok(RuntimeWebsocketConnectResult::QuotaBlocked(
                        runtime_websocket_error_payload_from_http_body(&body),
                    ));
                }
                if extract_runtime_proxy_overload_message(status, &body).is_some() {
                    return Ok(RuntimeWebsocketConnectResult::Overloaded(
                        runtime_websocket_error_payload_from_http_body(&body),
                    ));
                }
                bail!("runtime websocket upstream rejected the handshake with HTTP {status}");
            }
            Err(err) => {
                return Err(runtime_websocket_connect_transport_error(
                    shared,
                    request_id,
                    profile_name,
                    &err,
                ));
            }
        }
    }
}

fn runtime_websocket_connect_transport_error(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    profile_name: &str,
    err: &WsError,
) -> anyhow::Error {
    let transport_error = anyhow::anyhow!("failed to connect runtime websocket upstream: {err}");
    if let Some(local_pressure_kind) = runtime_websocket_local_pressure_kind_from_ws_error(err) {
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "websocket_connect_local_pressure",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("class", local_pressure_kind.as_str()),
                    runtime_proxy_log_field("error", err.to_string()),
                ],
            ),
        );
        return transport_error;
    }

    let failure_kind = runtime_transport_failure_kind_from_ws(err);
    log_runtime_upstream_connect_failure(
        shared,
        request_id,
        "websocket",
        profile_name,
        failure_kind,
        err,
    );
    note_runtime_profile_transport_failure(
        shared,
        profile_name,
        RuntimeRouteKind::Websocket,
        "websocket_connect",
        &transport_error,
    );
    transport_error
}

fn runtime_websocket_local_pressure_kind_from_ws_error(
    err: &WsError,
) -> Option<RuntimeWebsocketLocalPressureKind> {
    match err {
        WsError::Io(err) => runtime_websocket_local_pressure_kind_from_io_error(err),
        _ => None,
    }
}

pub(super) fn connect_runtime_proxy_upstream_websocket_with_timeout(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    request: tungstenite::http::Request<()>,
) -> std::result::Result<
    (
        RuntimeUpstreamWebSocket,
        tungstenite::handshake::client::Response,
        SocketAddr,
        usize,
        usize,
    ),
    WsError,
> {
    let stream = connect_runtime_proxy_upstream_tcp_stream(request_id, shared, request.uri())?;
    let selected_addr = stream.selected_addr;
    let resolved_addrs = stream.resolved_addrs;
    let attempted_addrs = stream.attempted_addrs;
    match client_tls_with_config(request, stream.stream, None, None) {
        Ok((socket, response)) => Ok((
            socket,
            response,
            selected_addr,
            resolved_addrs,
            attempted_addrs,
        )),
        Err(WsHandshakeError::Failure(err)) => Err(err),
        Err(WsHandshakeError::Interrupted(_)) => Err(WsError::Io(io::Error::new(
            io::ErrorKind::WouldBlock,
            "upstream websocket handshake interrupted before completion",
        ))),
    }
}

pub(super) fn runtime_configure_upstream_tcp_stream(
    stream: &TcpStream,
    io_timeout: Duration,
) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(io_timeout))?;
    stream.set_write_timeout(Some(io_timeout))?;
    Ok(())
}

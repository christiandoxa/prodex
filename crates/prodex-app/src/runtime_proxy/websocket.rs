use super::*;
use redaction::redaction_redact_secret_like_text;
mod connect;
mod response_tracking;
pub(crate) use connect::*;
pub(crate) use response_tracking::*;

mod session_state;
mod tcp_connect_executor;
mod unauthorized_recovery;

pub(in crate::runtime_proxy) use self::session_state::RuntimeWebsocketSessionState;
#[cfg(test)]
pub(crate) use self::session_state::acquire_runtime_profile_inflight_guard;
pub(crate) use self::session_state::try_acquire_runtime_profile_inflight_guard;
use self::tcp_connect_executor::*;
pub(super) use self::unauthorized_recovery::{
    RuntimeProfileUnauthorizedRecoveryStep, RuntimeProfileUnauthorizedRecoverySteps,
    runtime_try_recover_profile_auth_from_unauthorized_steps,
};
use runtime_proxy_crate::{
    RuntimeTokenUsageProgress, RuntimeWebsocketTarget,
    inspect_runtime_websocket_text_frame_with_phase, runtime_interleave_socket_addrs,
    runtime_proxy_websocket_error_payload_text, runtime_realtime_websocket_terminal_event_kind,
    runtime_translate_precommit_previous_response_websocket_text_frame,
    runtime_translate_previous_response_websocket_text_frame, runtime_websocket_authority,
    runtime_websocket_error_payload_from_http_body, runtime_websocket_http_connect_request,
    runtime_websocket_normalize_host, runtime_websocket_precommit_hold_promotion_allowed,
    runtime_websocket_precommit_hold_promotion_event_seen,
    runtime_websocket_precommit_transport_retry_allowed,
    runtime_websocket_proxy_authorization_header, runtime_websocket_read_http_connect_response,
    runtime_websocket_target_from_parts,
};

pub(super) fn runtime_websocket_error_log_value(error: &str) -> String {
    redaction_redact_secret_like_text(error).replace('\n', " ")
}

fn runtime_websocket_local_disconnect_error(error: &WsError) -> bool {
    match error {
        WsError::ConnectionClosed | WsError::AlreadyClosed => true,
        WsError::Io(error) => matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::NotConnected
                | io::ErrorKind::UnexpectedEof
        ),
        WsError::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake) => true,
        _ => false,
    }
}

pub(super) fn run_runtime_proxy_websocket_session(
    session_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    realtime_duplex: bool,
) -> Result<()> {
    let mut websocket_session = RuntimeWebsocketSessionState::with_realtime_duplex(realtime_duplex);
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
                if websocket_session.is_realtime_duplex() && websocket_session.has_socket() {
                    let result = run_runtime_realtime_websocket_duplex_session(
                        session_id,
                        local_socket,
                        handshake_request,
                        shared,
                        &mut websocket_session,
                    );
                    websocket_session.reset();
                    return result;
                }
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
                if runtime_websocket_local_disconnect_error(&err) {
                    runtime_proxy_log(
                        shared,
                        format!("websocket_session={session_id} local_connection_closed"),
                    );
                    websocket_session.close();
                    break;
                }
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "local_read_error",
                        [
                            runtime_proxy_log_field("websocket_session", session_id.to_string()),
                            runtime_proxy_log_field(
                                "error",
                                runtime_websocket_error_log_value(&err.to_string()),
                            ),
                        ],
                    ),
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
    let upstream_base_url = shared
        .lock_runtime_state()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .upstream_base_url
        .clone();
    let upstream_url = runtime_proxy_upstream_websocket_url(
        &upstream_base_url,
        &handshake_request.path_and_query,
    )?;
    let log_url = runtime_proxy_log_url(&upstream_url);
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
    loop {
        let auth = runtime_profile_usage_auth(shared, profile_name)?;
        let request = build_runtime_proxy_websocket_request(
            handshake_request,
            shared,
            profile_name,
            turn_state_override,
            auth,
            &upstream_url,
            &log_url,
        )?;

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
        if runtime_take_fault_injection_budget(
            "PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE",
            shared.runtime_config.fault_upstream_connect_error_once,
        ) {
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
        if let Some(result) = handle_runtime_proxy_websocket_connect_attempt(
            request_id,
            shared,
            profile_name,
            &upstream_url,
            request,
            started_at,
            &mut recovery_steps,
        )? {
            return Ok(result);
        }
    }
}

fn build_runtime_proxy_websocket_request(
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
    auth: UsageAuth,
    upstream_url: &str,
    log_url: &str,
) -> Result<tungstenite::http::Request<()>> {
    let mut request = upstream_url
        .into_client_request()
        .with_context(|| format!("failed to build runtime websocket request for {log_url}"))?;
    append_runtime_proxy_websocket_forwarded_headers(
        &mut request,
        handshake_request,
        turn_state_override,
    );
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
        upstream_url,
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
    Ok(request)
}

fn append_runtime_proxy_websocket_forwarded_headers(
    request: &mut tungstenite::http::Request<()>,
    handshake_request: &RuntimeProxyRequest,
    turn_state_override: Option<&str>,
) {
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
        request.headers_mut().append(header_name, header_value);
    }
}

fn handle_runtime_proxy_websocket_connect_attempt(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    upstream_url: &str,
    request: tungstenite::http::Request<()>,
    started_at: Instant,
    recovery_steps: &mut RuntimeProfileUnauthorizedRecoverySteps,
) -> Result<Option<RuntimeWebsocketConnectResult>> {
    match connect_runtime_proxy_upstream_websocket_with_timeout(request_id, shared, request) {
        Ok((socket, response, selected_addr, resolved_addrs, attempted_addrs)) => {
            runtime_proxy_capture_websocket_cookies(
                shared,
                profile_name,
                upstream_url,
                response.headers(),
            );
            let turn_state =
                runtime_proxy_tungstenite_header_value(response.headers(), "x-codex-turn-state");
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "upstream_connect_ok",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "websocket"),
                        runtime_proxy_log_field("profile", profile_name),
                        runtime_proxy_log_field("status", response.status().as_u16().to_string()),
                        runtime_proxy_log_field("addr", selected_addr.to_string()),
                        runtime_proxy_log_field("resolved_addrs", resolved_addrs.to_string()),
                        runtime_proxy_log_field("attempted_addrs", attempted_addrs.to_string()),
                        runtime_proxy_log_field(
                            "turn_state",
                            if turn_state.is_some() {
                                "present"
                            } else {
                                "none"
                            },
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
            Ok(Some(RuntimeWebsocketConnectResult::Connected {
                socket,
                turn_state,
            }))
        }
        Err(WsError::Http(response)) => {
            runtime_proxy_capture_websocket_cookies(
                shared,
                profile_name,
                upstream_url,
                response.headers(),
            );
            let status = response.status().as_u16();
            let body = response.body().clone().unwrap_or_default();
            handle_runtime_proxy_websocket_http_response(
                request_id,
                shared,
                profile_name,
                status,
                body,
                recovery_steps,
            )
        }
        Err(err) => Err(runtime_websocket_connect_transport_error(
            shared,
            request_id,
            profile_name,
            &err,
        )),
    }
}

fn handle_runtime_proxy_websocket_http_response(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    status: u16,
    body: Vec<u8>,
    recovery_steps: &mut RuntimeProfileUnauthorizedRecoverySteps,
) -> Result<Option<RuntimeWebsocketConnectResult>> {
    if status == 401
        && runtime_try_recover_profile_auth_from_unauthorized_steps(
            request_id,
            shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            recovery_steps,
        )
    {
        return Ok(None);
    }
    let error_policy = runtime_proxy_crate::runtime_http_error_policy(
        status,
        &body,
        runtime_proxy_crate::RuntimeHttpErrorPhase::PreCommit,
    );
    if (matches!(status, 401 | 403)
        && (status == 401
            || error_policy.action != runtime_proxy_crate::RuntimeHttpErrorAction::RotateProfile))
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
    if error_policy.action == runtime_proxy_crate::RuntimeHttpErrorAction::RotateProfile
        && error_policy.class == runtime_proxy_crate::RuntimeHttpErrorClass::Quota
    {
        return Ok(Some(RuntimeWebsocketConnectResult::QuotaBlocked(
            runtime_websocket_error_payload_from_http_body(&body),
        )));
    }
    if error_policy.action == runtime_proxy_crate::RuntimeHttpErrorAction::RetryProfile
        || (error_policy.action == runtime_proxy_crate::RuntimeHttpErrorAction::RotateProfile
            && error_policy.class == runtime_proxy_crate::RuntimeHttpErrorClass::ProfileUnavailable)
    {
        return Ok(Some(RuntimeWebsocketConnectResult::Overloaded(
            runtime_websocket_error_payload_from_http_body(&body),
        )));
    }
    let payload = if body.is_empty() {
        RuntimeWebsocketErrorPayload::Text(runtime_proxy_websocket_error_payload_text(
            status,
            "upstream_rejected",
            &format!("Upstream rejected the WebSocket handshake with HTTP {status}."),
        ))
    } else {
        runtime_websocket_error_payload_from_http_body(&body)
    };
    Ok(Some(RuntimeWebsocketConnectResult::Rejected(payload)))
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
                    runtime_proxy_log_field(
                        "error",
                        runtime_websocket_error_log_value(&err.to_string()),
                    ),
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
        Err(WsHandshakeError::Failure(WsError::Io(err)))
            if matches!(
                err.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            Err(runtime_websocket_handshake_timeout_error())
        }
        Err(WsHandshakeError::Failure(err)) => Err(err),
        Err(WsHandshakeError::Interrupted(_)) => Err(runtime_websocket_handshake_timeout_error()),
    }
}

fn runtime_websocket_handshake_timeout_error() -> WsError {
    WsError::Io(io::Error::new(
        io::ErrorKind::TimedOut,
        "upstream websocket handshake timed out before completion",
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_error_log_value_redacts_secret_like_material() {
        let message = runtime_websocket_error_log_value(
            "connect failed\nAuthorization: Bearer websocket-token\napi_key=websocket-key",
        );

        assert!(!message.contains('\n'));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("websocket-token"));
        assert!(!message.contains("websocket-key"));
    }

    #[test]
    fn local_disconnect_errors_are_not_reported_as_session_failures() {
        assert!(runtime_websocket_local_disconnect_error(
            &WsError::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake)
        ));
        assert!(runtime_websocket_local_disconnect_error(&WsError::Io(
            io::Error::from(io::ErrorKind::ConnectionReset),
        )));
        assert!(!runtime_websocket_local_disconnect_error(
            &WsError::Protocol(tungstenite::error::ProtocolError::WrongHttpMethod,)
        ));
    }

    #[test]
    fn websocket_forwarding_appends_metadata_and_filters_replacements_and_hops() {
        let handshake_request = RuntimeProxyRequest {
            method: "GET".to_string(),
            path_and_query: "/v1/realtime".to_string(),
            headers: vec![
                (
                    "x-codex-turn-metadata".to_string(),
                    "metadata-one".to_string(),
                ),
                (
                    "x-codex-turn-metadata".to_string(),
                    "metadata-two".to_string(),
                ),
                ("user-agent".to_string(), "fixture-client".to_string()),
                ("cookie".to_string(), "caller=session".to_string()),
                ("authorization".to_string(), "Bearer caller".to_string()),
                ("connection".to_string(), "keep-alive".to_string()),
            ],
            body: Vec::new(),
        };
        let mut request = "wss://example.com/v1/realtime"
            .into_client_request()
            .expect("websocket request should build");

        append_runtime_proxy_websocket_forwarded_headers(&mut request, &handshake_request, None);

        let metadata = request
            .headers()
            .get_all("x-codex-turn-metadata")
            .iter()
            .map(|value| value.to_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(metadata, ["metadata-one", "metadata-two"]);
        assert_eq!(request.headers()["user-agent"], "fixture-client");
        assert!(!request.headers().contains_key("cookie"));
        assert!(!request.headers().contains_key("authorization"));
        assert_eq!(request.headers().get_all("connection").iter().count(), 1);
        assert_eq!(request.headers()["connection"], "Upgrade");
    }
}

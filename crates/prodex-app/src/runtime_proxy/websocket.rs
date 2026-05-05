use super::*;

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
                        runtime_request_turn_state(handshake_request),
                        text.len()
                    ),
                );
                let compat_surface = runtime_detect_websocket_message_compatibility_surface(
                    handshake_request,
                    text.as_ref(),
                );
                runtime_proxy_log_request_compatibility(shared, message_id, &compat_surface);
                proxy_runtime_websocket_text_message(
                    session_id,
                    message_id,
                    local_socket,
                    handshake_request,
                    text.as_ref(),
                    &request_metadata,
                    shared,
                    &mut websocket_session,
                )?;
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
    let mut recovery_steps = RuntimeProfileUnauthorizedRecoveryStep::ordered();
    loop {
        let auth = runtime_profile_usage_auth(shared, profile_name)?;
        let mut request = upstream_url
            .as_str()
            .into_client_request()
            .with_context(|| {
                format!("failed to build runtime websocket request for {upstream_url}")
            })?;

        for (name, value) in &handshake_request.headers {
            if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
                continue;
            }
            if name.eq_ignore_ascii_case("cookie") {
                continue;
            }
            if should_skip_runtime_request_header(name) {
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
        let user_agent =
            runtime_proxy_effective_user_agent(&handshake_request.headers).unwrap_or("codex-cli");
        request.headers_mut().insert(
            WsHeaderName::from_static("user-agent"),
            WsHeaderValue::from_str(user_agent).context("failed to encode websocket user-agent")?,
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
                    runtime_proxy_log_field("url", upstream_url.as_str()),
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
                if matches!(status, 401 | 403)
                    && (status == 401 || extract_runtime_proxy_quota_message(&body).is_none())
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
                if matches!(status, 403 | 429)
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
        Err(WsHandshakeError::Interrupted(_)) => {
            unreachable!("blocking upstream websocket handshake should not interrupt")
        }
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

fn runtime_resolve_websocket_tcp_addrs(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    host: &str,
    port: u16,
    timeout: Duration,
) -> io::Result<Vec<SocketAddr>> {
    runtime_resolve_websocket_tcp_addrs_with_executor(
        runtime_websocket_dns_resolve_executor(),
        Some(shared.log_path.as_path()),
        Some(request_id),
        host.to_string(),
        port,
        timeout,
        |host, port| {
            (host.as_str(), port)
                .to_socket_addrs()
                .map(Iterator::collect)
        },
    )
}

pub(super) fn connect_runtime_proxy_upstream_tcp_stream(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    uri: &tungstenite::http::Uri,
) -> std::result::Result<RuntimeWebsocketTcpConnectSuccess, WsError> {
    let target = runtime_websocket_target_from_uri(uri)?;
    let connect_timeout = Duration::from_millis(runtime_proxy_websocket_connect_timeout_ms());
    let io_timeout = Duration::from_millis(runtime_proxy_websocket_precommit_progress_timeout_ms());
    let happy_eyeballs_delay =
        Duration::from_millis(runtime_proxy_websocket_happy_eyeballs_delay_ms());

    if let Some(proxy) =
        runtime_websocket_http_proxy_for_uri(shared, uri, &target).map_err(WsError::Io)?
    {
        return connect_runtime_proxy_upstream_tcp_stream_via_http_proxy(
            request_id,
            shared,
            &proxy,
            &target,
            connect_timeout,
            io_timeout,
            happy_eyeballs_delay,
        );
    }

    connect_runtime_proxy_tcp_stream_to_host(
        request_id,
        shared,
        &target.host,
        target.port,
        connect_timeout,
        io_timeout,
        happy_eyeballs_delay,
    )
}

fn connect_runtime_proxy_upstream_tcp_stream_via_http_proxy(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    proxy: &RuntimeWebsocketHttpProxy,
    target: &RuntimeWebsocketTarget,
    connect_timeout: Duration,
    io_timeout: Duration,
    happy_eyeballs_delay: Duration,
) -> std::result::Result<RuntimeWebsocketTcpConnectSuccess, WsError> {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "websocket_proxy_connect_start",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "websocket"),
                runtime_proxy_log_field("proxy", proxy.log_label()),
                runtime_proxy_log_field("target", target.authority.as_str()),
            ],
        ),
    );
    let mut connected = connect_runtime_proxy_tcp_stream_to_host(
        request_id,
        shared,
        &proxy.host,
        proxy.port,
        connect_timeout,
        io_timeout,
        happy_eyeballs_delay,
    )?;

    match runtime_websocket_establish_http_proxy_tunnel(
        &mut connected.stream,
        proxy,
        target,
        io_timeout,
    ) {
        Ok(status) => {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "websocket_proxy_tunnel_ok",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "websocket"),
                        runtime_proxy_log_field("proxy", proxy.log_label()),
                        runtime_proxy_log_field("target", target.authority.as_str()),
                        runtime_proxy_log_field("status", status.to_string()),
                        runtime_proxy_log_field("addr", connected.selected_addr.to_string()),
                    ],
                ),
            );
            Ok(connected)
        }
        Err(err) => {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "websocket_proxy_tunnel_failure",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "websocket"),
                        runtime_proxy_log_field("proxy", proxy.log_label()),
                        runtime_proxy_log_field("target", target.authority.as_str()),
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
            Err(WsError::Io(err))
        }
    }
}

fn connect_runtime_proxy_tcp_stream_to_host(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    host: &str,
    port: u16,
    connect_timeout: Duration,
    io_timeout: Duration,
    happy_eyeballs_delay: Duration,
) -> std::result::Result<RuntimeWebsocketTcpConnectSuccess, WsError> {
    let addrs = runtime_interleave_socket_addrs(
        runtime_resolve_websocket_tcp_addrs(request_id, shared, host, port, connect_timeout)
            .map_err(WsError::Io)?,
    );
    if addrs.is_empty() {
        return Err(WsError::Url(WsUrlError::UnableToConnect(
            runtime_websocket_authority(host, port),
        )));
    }

    let resolved_addrs = addrs.len();
    let (sender, receiver) = mpsc::channel::<RuntimeWebsocketTcpAttemptResult>();
    let mut next_index = 0usize;
    let mut attempted_addrs = 0usize;
    let mut in_flight = 0usize;
    let mut last_error = None;

    while next_index < addrs.len() || in_flight > 0 {
        if in_flight == 0 && next_index < addrs.len() {
            runtime_launch_websocket_tcp_connect_attempt(
                request_id,
                shared,
                sender.clone(),
                addrs[next_index],
                connect_timeout,
            );
            next_index += 1;
            attempted_addrs += 1;
            in_flight += 1;
        }

        let next = if in_flight == 1 && next_index < addrs.len() && !happy_eyeballs_delay.is_zero()
        {
            match receiver.recv_timeout(happy_eyeballs_delay) {
                Ok(result) => Some(result),
                Err(RecvTimeoutError::Timeout) => {
                    runtime_launch_websocket_tcp_connect_attempt(
                        request_id,
                        shared,
                        sender.clone(),
                        addrs[next_index],
                        connect_timeout,
                    );
                    next_index += 1;
                    attempted_addrs += 1;
                    in_flight += 1;
                    receiver.recv().ok()
                }
                Err(RecvTimeoutError::Disconnected) => None,
            }
        } else {
            receiver.recv().ok()
        };

        let Some(result) = next else {
            break;
        };
        in_flight = in_flight.saturating_sub(1);
        match result.result {
            Ok(stream) => {
                runtime_configure_upstream_tcp_stream(&stream, io_timeout).map_err(WsError::Io)?;
                return Ok(RuntimeWebsocketTcpConnectSuccess {
                    stream,
                    selected_addr: result.addr,
                    resolved_addrs,
                    attempted_addrs,
                });
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }

    match last_error {
        Some(err) => Err(WsError::Io(err)),
        None => Err(WsError::Url(WsUrlError::UnableToConnect(
            runtime_websocket_authority(host, port),
        ))),
    }
}

fn runtime_websocket_target_from_uri(
    uri: &tungstenite::http::Uri,
) -> std::result::Result<RuntimeWebsocketTarget, WsError> {
    let host = uri.host().ok_or(WsError::Url(WsUrlError::NoHostName))?;
    Ok(runtime_websocket_target_from_parts(
        host,
        uri.port_u16(),
        uri.scheme_str(),
    ))
}

#[derive(Debug, Clone)]
struct RuntimeWebsocketHttpProxy {
    url: reqwest::Url,
    host: String,
    port: u16,
}

impl RuntimeWebsocketHttpProxy {
    fn log_label(&self) -> String {
        let mut redacted = self.url.clone();
        let _ = redacted.set_username("");
        let _ = redacted.set_password(None);
        redacted.to_string()
    }
}

fn runtime_websocket_http_proxy_for_uri(
    shared: &RuntimeRotationProxyShared,
    uri: &tungstenite::http::Uri,
    target: &RuntimeWebsocketTarget,
) -> io::Result<Option<RuntimeWebsocketHttpProxy>> {
    if shared.upstream_no_proxy || runtime_websocket_no_proxy_matches(&target.host, target.port) {
        return Ok(None);
    }
    let Some(proxy_url) = runtime_websocket_proxy_env_url(uri.scheme_str().unwrap_or("wss")) else {
        return Ok(None);
    };
    if proxy_url.scheme() != "http" {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "runtime websocket proxy scheme '{}' is unsupported; use an http:// proxy or --no-proxy",
                proxy_url.scheme()
            ),
        ));
    }
    let Some(host) = proxy_url.host_str() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime websocket proxy URL is missing a host",
        ));
    };
    let host = runtime_websocket_normalize_host(host);
    let port = proxy_url.port_or_known_default().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime websocket proxy URL is missing a port",
        )
    })?;
    Ok(Some(RuntimeWebsocketHttpProxy {
        url: proxy_url,
        host,
        port,
    }))
}

fn runtime_websocket_proxy_env_url(scheme: &str) -> Option<reqwest::Url> {
    runtime_websocket_proxy_env_keys(scheme)
        .iter()
        .find_map(|key| {
            let value = env::var_os(key)?;
            runtime_websocket_parse_proxy_url(&value.to_string_lossy())
        })
}

fn runtime_websocket_parse_proxy_url(value: &str) -> Option<reqwest::Url> {
    let candidate = runtime_websocket_proxy_url_candidate(value)?;
    reqwest::Url::parse(&candidate).ok()
}

fn runtime_websocket_no_proxy_matches(host: &str, port: u16) -> bool {
    ["NO_PROXY", "no_proxy"].into_iter().any(|key| {
        env::var_os(key).is_some_and(|value| {
            runtime_websocket_no_proxy_value_matches(&value.to_string_lossy(), host, port)
        })
    })
}

fn runtime_websocket_establish_http_proxy_tunnel(
    stream: &mut TcpStream,
    proxy: &RuntimeWebsocketHttpProxy,
    target: &RuntimeWebsocketTarget,
    io_timeout: Duration,
) -> io::Result<u16> {
    stream.set_read_timeout(Some(io_timeout))?;
    stream.set_write_timeout(Some(io_timeout))?;
    let proxy_authorization =
        runtime_websocket_proxy_authorization_header(proxy.url.username(), proxy.url.password());
    let request =
        runtime_websocket_http_connect_request(&target.authority, proxy_authorization.as_deref());
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let (status, response_bytes) = runtime_websocket_read_http_connect_response(stream)?;
    if status != 200 {
        return Err(io::Error::other(format!(
            "runtime websocket proxy CONNECT returned HTTP {status} ({response_bytes} bytes)"
        )));
    }
    Ok(status)
}

pub(super) fn send_runtime_proxy_websocket_error(
    local_socket: &mut RuntimeLocalWebSocket,
    status: u16,
    code: &str,
    message: &str,
) -> Result<()> {
    let payload = runtime_proxy_websocket_error_payload_text(status, code, message);
    local_socket
        .send(WsMessage::Text(payload.into()))
        .context("failed to send runtime websocket error frame")
}

pub(super) fn forward_runtime_proxy_websocket_error(
    local_socket: &mut RuntimeLocalWebSocket,
    payload: &RuntimeWebsocketErrorPayload,
) -> Result<()> {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => local_socket
            .send(WsMessage::Text(text.clone().into()))
            .context("failed to forward runtime websocket text error frame"),
        RuntimeWebsocketErrorPayload::Binary(bytes) => local_socket
            .send(WsMessage::Binary(bytes.clone().into()))
            .context("failed to forward runtime websocket binary error frame"),
        RuntimeWebsocketErrorPayload::Empty => Ok(()),
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RuntimeWebsocketResponseBindingContext<'a> {
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) profile_name: &'a str,
    pub(crate) request_previous_response_id: Option<&'a str>,
    pub(crate) request_session_id: Option<&'a str>,
    pub(crate) request_turn_state: Option<&'a str>,
    pub(crate) response_turn_state: Option<&'a str>,
}

pub(crate) fn remember_runtime_websocket_response_ids(
    context: RuntimeWebsocketResponseBindingContext<'_>,
    response_ids: &[String],
    previous_response_owner_recorded: &mut bool,
) -> Result<()> {
    let RuntimeWebsocketResponseBindingContext {
        shared,
        profile_name,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        response_turn_state,
    } = context;

    if !*previous_response_owner_recorded {
        remember_runtime_successful_previous_response_owner(
            shared,
            profile_name,
            request_previous_response_id,
            RuntimeRouteKind::Websocket,
        )?;
        *previous_response_owner_recorded = true;
    }
    remember_runtime_response_ids_with_turn_state(
        shared,
        profile_name,
        response_ids,
        response_turn_state,
        RuntimeRouteKind::Websocket,
    )?;
    if !response_ids.is_empty() && response_turn_state.is_some() {
        let _ = release_runtime_compact_lineage(
            shared,
            profile_name,
            request_session_id,
            request_turn_state,
            "response_committed",
        );
    }
    Ok(())
}

pub(super) fn forward_runtime_proxy_buffered_websocket_text_frames(
    local_socket: &mut RuntimeLocalWebSocket,
    buffered_frames: &mut Vec<RuntimeBufferedWebsocketTextFrame>,
    context: RuntimeWebsocketResponseBindingContext<'_>,
    previous_response_owner_recorded: &mut bool,
) -> Result<()> {
    for frame in buffered_frames.drain(..) {
        remember_runtime_websocket_response_ids(
            context,
            &frame.response_ids,
            previous_response_owner_recorded,
        )?;
        let text = runtime_translate_precommit_previous_response_websocket_text_frame(&frame.text);
        local_socket
            .send(WsMessage::Text(text.into()))
            .context("failed to forward buffered runtime websocket text frame")?;
    }
    Ok(())
}

pub(super) struct RuntimeWebsocketAttemptRequest<'a> {
    pub(in crate::runtime_proxy) request_id: u64,
    pub(in crate::runtime_proxy) local_socket: &'a mut RuntimeLocalWebSocket,
    pub(in crate::runtime_proxy) handshake_request: &'a RuntimeProxyRequest,
    pub(in crate::runtime_proxy) request_text: &'a str,
    pub(in crate::runtime_proxy) request_previous_response_id: Option<&'a str>,
    pub(in crate::runtime_proxy) request_prompt_cache_key: Option<&'a str>,
    pub(in crate::runtime_proxy) request_session_id: Option<&'a str>,
    pub(in crate::runtime_proxy) request_turn_state: Option<&'a str>,
    pub(in crate::runtime_proxy) shared: &'a RuntimeRotationProxyShared,
    pub(in crate::runtime_proxy) websocket_session: &'a mut RuntimeWebsocketSessionState,
    pub(in crate::runtime_proxy) profile_name: &'a str,
    pub(in crate::runtime_proxy) turn_state_override: Option<&'a str>,
    pub(in crate::runtime_proxy) promote_committed_profile: bool,
}

pub(super) fn attempt_runtime_websocket_request(
    attempt: RuntimeWebsocketAttemptRequest<'_>,
) -> Result<RuntimeWebsocketAttempt> {
    let RuntimeWebsocketAttemptRequest {
        request_id,
        local_socket,
        handshake_request,
        request_text,
        request_previous_response_id,
        request_prompt_cache_key,
        request_session_id,
        request_turn_state,
        shared,
        websocket_session,
        profile_name,
        turn_state_override,
        promote_committed_profile,
    } = attempt;
    let request_model_name = runtime_smart_context_model_name_from_body(request_text.as_bytes());

    let realtime_websocket = is_runtime_realtime_websocket_path(&handshake_request.path_and_query);
    let quota_gate = runtime_precommit_quota_gate(RuntimePrecommitQuotaGateRequest {
        shared,
        profile_name,
        route_kind: RuntimeRouteKind::Websocket,
        has_continuation_context: request_previous_response_id.is_some()
            || request_session_id.is_some()
            || request_turn_state.is_some(),
        reprobe_context: "websocket_precommit_reprobe",
    })?;
    if let RuntimePrecommitQuotaGateDecision::Block {
        reason,
        summary,
        source,
    } = quota_gate
    {
        websocket_session.close();
        let reason_label = reason.as_str();
        let mut log_fields = vec![
            runtime_proxy_log_field("request", request_id.to_string()),
            runtime_proxy_log_field("transport", "websocket"),
            runtime_proxy_log_field("profile", profile_name),
            runtime_proxy_log_field("reason", reason_label),
            runtime_proxy_log_field(
                "quota_source",
                source.map(runtime_quota_source_label).unwrap_or("unknown"),
            ),
        ];
        log_fields.extend([
            runtime_proxy_log_field(
                "quota_band",
                runtime_quota_pressure_band_reason(summary.route_band),
            ),
            runtime_proxy_log_field(
                "five_hour_status",
                runtime_quota_window_status_reason(summary.five_hour.status),
            ),
            runtime_proxy_log_field(
                "five_hour_remaining",
                summary.five_hour.remaining_percent.to_string(),
            ),
            runtime_proxy_log_field("five_hour_reset_at", summary.five_hour.reset_at.to_string()),
            runtime_proxy_log_field(
                "weekly_status",
                runtime_quota_window_status_reason(summary.weekly.status),
            ),
            runtime_proxy_log_field(
                "weekly_remaining",
                summary.weekly.remaining_percent.to_string(),
            ),
            runtime_proxy_log_field("weekly_reset_at", summary.weekly.reset_at.to_string()),
        ]);
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message("websocket_pre_send_skip", log_fields),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: reason_label,
        });
    }

    let reuse_existing_session = websocket_session.can_reuse(profile_name, turn_state_override);
    let reuse_started_at = reuse_existing_session.then(Instant::now);
    let precommit_started_at = Instant::now();
    let (mut upstream_socket, mut upstream_turn_state, mut inflight_guard) =
        if reuse_existing_session {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "websocket_reuse_start",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "websocket"),
                        runtime_proxy_log_field("profile", profile_name),
                        runtime_proxy_log_field(
                            "turn_state_override",
                            format!("{turn_state_override:?}"),
                        ),
                    ],
                ),
            );
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=reuse profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            (
                websocket_session
                    .take_socket()
                    .expect("runtime websocket session should keep its upstream socket"),
                websocket_session.turn_state.clone(),
                None,
            )
        } else {
            websocket_session.close();
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=connect profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            match connect_runtime_proxy_upstream_websocket(
                request_id,
                handshake_request,
                shared,
                profile_name,
                turn_state_override,
            )? {
                RuntimeWebsocketConnectResult::Connected { socket, turn_state } => (
                    socket,
                    turn_state,
                    Some(acquire_runtime_profile_inflight_guard(
                        shared,
                        profile_name,
                        "websocket_session",
                    )?),
                ),
                RuntimeWebsocketConnectResult::QuotaBlocked(payload) => {
                    return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
                RuntimeWebsocketConnectResult::Overloaded(payload) => {
                    return Ok(RuntimeWebsocketAttempt::Overloaded {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
            }
        };
    runtime_set_upstream_websocket_io_timeout(
        &mut upstream_socket,
        Some(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms(),
        )),
    )
    .context("failed to configure runtime websocket pre-commit timeout")?;

    let upstream_request_text = prepare_runtime_smart_context_websocket_text(
        request_id,
        request_text,
        handshake_request,
        shared,
        profile_name,
    );
    if let Err(err) =
        upstream_socket.send(WsMessage::Text(upstream_request_text.into_owned().into()))
    {
        let _ = upstream_socket.close(None);
        websocket_session.reset();
        let transport_error =
            anyhow::anyhow!("failed to send runtime websocket request upstream: {err}");
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            "websocket_upstream_send",
            &transport_error,
        );
        runtime_proxy_log(
            shared,
            runtime_proxy_structured_log_message(
                "upstream_send_error",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "websocket"),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("error", err.to_string()),
                ],
            ),
        );
        if reuse_existing_session {
            return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name: profile_name.to_string(),
                event: "upstream_send_error",
            });
        }
        return Err(transport_error);
    }

    let mut committed = false;
    let mut first_upstream_frame_seen = false;
    let mut buffered_precommit_text_frames = Vec::new();
    let mut committed_response_ids = BTreeSet::new();
    let mut previous_response_owner_recorded = false;
    let mut precommit_hold_count = 0usize;
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(
                            runtime_proxy_websocket_precommit_progress_timeout_ms(),
                        )),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }

                let mut inspected = inspect_runtime_websocket_text_frame(text.as_str());
                if realtime_websocket
                    && inspected
                        .event_type
                        .as_deref()
                        .is_some_and(runtime_realtime_websocket_terminal_event_kind)
                {
                    inspected.terminal_event = true;
                }
                if let Some(turn_state) = inspected.turn_state.as_deref() {
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        Some(turn_state),
                        RuntimeRouteKind::Websocket,
                    )?;
                    upstream_turn_state = Some(turn_state.to_string());
                }

                if !committed {
                    match inspected.retry_kind {
                        Some(RuntimeWebsocketRetryInspectionKind::QuotaBlocked) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::Overloaded) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::Overloaded {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::PreviousResponseNotFound {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                                turn_state: upstream_turn_state.clone(),
                            });
                        }
                        None => {}
                    }
                }

                if !committed && inspected.precommit_hold {
                    if precommit_hold_count == 0 {
                        runtime_proxy_log(
                            shared,
                            runtime_proxy_structured_log_message(
                                "precommit_hold",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field("transport", "websocket"),
                                    runtime_proxy_log_field("profile", profile_name),
                                    runtime_proxy_log_field(
                                        "event_type",
                                        inspected.event_type.as_deref().unwrap_or("-"),
                                    ),
                                ],
                            ),
                        );
                    }
                    precommit_hold_count = precommit_hold_count.saturating_add(1);
                    buffered_precommit_text_frames.push(RuntimeBufferedWebsocketTextFrame {
                        text,
                        response_ids: inspected.response_ids,
                    });
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    let timeout_ms = runtime_proxy_websocket_precommit_progress_timeout_ms();
                    if elapsed_ms >= u128::from(timeout_ms) {
                        let _ = upstream_socket.close(None);
                        websocket_session.reset();
                        runtime_proxy_log(
                            shared,
                            runtime_proxy_structured_log_message(
                                "websocket_precommit_hold_timeout",
                                [
                                    runtime_proxy_log_field("profile", profile_name),
                                    runtime_proxy_log_field("elapsed_ms", elapsed_ms.to_string()),
                                    runtime_proxy_log_field("threshold_ms", timeout_ms.to_string()),
                                    runtime_proxy_log_field(
                                        "reuse",
                                        reuse_existing_session.to_string(),
                                    ),
                                    runtime_proxy_log_field(
                                        "hold_count",
                                        precommit_hold_count.to_string(),
                                    ),
                                ],
                            ),
                        );
                        let transport_error = anyhow::anyhow!(
                            "runtime websocket upstream remained in pre-commit hold beyond the progress deadline after {elapsed_ms} ms"
                        );
                        note_runtime_profile_transport_failure(
                            shared,
                            profile_name,
                            RuntimeRouteKind::Websocket,
                            "websocket_precommit_hold_timeout",
                            &transport_error,
                        );
                        return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                            profile_name: profile_name.to_string(),
                            event: "precommit_hold_timeout",
                        });
                    }
                    continue;
                }

                if !committed {
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id,
                        RuntimeRouteKind::Websocket,
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    let _ = commit_runtime_proxy_profile_selection_with_policy(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        promote_committed_profile,
                    )?;
                    remember_runtime_prompt_cache_profile(
                        shared,
                        profile_name,
                        request_prompt_cache_key,
                        RuntimeRouteKind::Websocket,
                    );
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket committed profile={profile_name}"
                        ),
                    );
                    committed = true;
                    for frame in &buffered_precommit_text_frames {
                        committed_response_ids.extend(frame.response_ids.iter().cloned());
                    }
                    forward_runtime_proxy_buffered_websocket_text_frames(
                        local_socket,
                        &mut buffered_precommit_text_frames,
                        RuntimeWebsocketResponseBindingContext {
                            shared,
                            profile_name,
                            request_previous_response_id,
                            request_session_id,
                            request_turn_state,
                            response_turn_state: upstream_turn_state.as_deref(),
                        },
                        &mut previous_response_owner_recorded,
                    )?;
                }

                committed_response_ids.extend(inspected.response_ids.iter().cloned());
                remember_runtime_websocket_response_ids(
                    RuntimeWebsocketResponseBindingContext {
                        shared,
                        profile_name,
                        request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        response_turn_state: upstream_turn_state.as_deref(),
                    },
                    &inspected.response_ids,
                    &mut previous_response_owner_recorded,
                )?;
                if committed
                    && runtime_token_usage_event_is_loggable(inspected.event_type.as_deref())
                {
                    log_runtime_token_usage(
                        shared,
                        request_id,
                        "websocket",
                        profile_name,
                        "responses_websocket",
                        request_prompt_cache_key,
                        request_model_name.as_deref(),
                        inspected.token_usage,
                    );
                }
                let committed_previous_response_not_found = committed
                    && matches!(
                        inspected.retry_kind,
                        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound)
                    );
                if committed_previous_response_not_found {
                    let mut dead_response_ids =
                        committed_response_ids.iter().cloned().collect::<Vec<_>>();
                    if let Some(previous_response_id) = request_previous_response_id {
                        dead_response_ids.push(previous_response_id.to_string());
                    }
                    let _ = clear_runtime_dead_response_bindings(
                        shared,
                        profile_name,
                        &dead_response_ids,
                        "previous_response_not_found_after_commit",
                    );
                    runtime_proxy_log_previous_response_stale_continuation(
                        shared,
                        RuntimePreviousResponseLogContext {
                            request_id,
                            transport: "websocket",
                            route: "websocket",
                            websocket_session: None,
                            via: None,
                        },
                        profile_name,
                    );
                    runtime_proxy_log_chain_dead_upstream_confirmed(
                        shared,
                        RuntimeProxyChainLog {
                            request_id,
                            transport: "websocket",
                            route: "websocket",
                            websocket_session: None,
                            profile_name,
                            previous_response_id: request_previous_response_id,
                            reason: "previous_response_not_found_locked_affinity",
                            via: None,
                        },
                        Some("post_commit"),
                    );
                }
                let text = if committed_previous_response_not_found {
                    runtime_translate_precommit_previous_response_websocket_text_frame(&text)
                } else {
                    runtime_translate_previous_response_websocket_text_frame(&text)
                };
                local_socket
                    .send(WsMessage::Text(text.into()))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket text frame"
                    })?;
                if inspected.terminal_event {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket terminal_event profile={profile_name} event_type={} precommit_hold_count={precommit_hold_count}",
                            inspected.event_type.as_deref().unwrap_or("-"),
                        ),
                    );
                    if committed_previous_response_not_found {
                        let _ = upstream_socket.close(None);
                        websocket_session.reset();
                    } else {
                        websocket_session.store(
                            upstream_socket,
                            profile_name,
                            upstream_turn_state,
                            inflight_guard.take(),
                        );
                    }
                    return Ok(RuntimeWebsocketAttempt::Delivered);
                }
            }
            Ok(WsMessage::Binary(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(
                            runtime_proxy_websocket_precommit_progress_timeout_ms(),
                        )),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }
                if !committed {
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id,
                        RuntimeRouteKind::Websocket,
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    let _ = commit_runtime_proxy_profile_selection_with_policy(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        promote_committed_profile,
                    )?;
                    remember_runtime_prompt_cache_profile(
                        shared,
                        profile_name,
                        request_prompt_cache_key,
                        RuntimeRouteKind::Websocket,
                    );
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket committed_binary profile={profile_name}"
                        ),
                    );
                    committed = true;
                    forward_runtime_proxy_buffered_websocket_text_frames(
                        local_socket,
                        &mut buffered_precommit_text_frames,
                        RuntimeWebsocketResponseBindingContext {
                            shared,
                            profile_name,
                            request_previous_response_id,
                            request_session_id,
                            request_turn_state,
                            response_turn_state: upstream_turn_state.as_deref(),
                        },
                        &mut previous_response_owner_recorded,
                    )?;
                }
                local_socket
                    .send(WsMessage::Binary(payload))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket binary frame"
                    })?;
            }
            Ok(WsMessage::Ping(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(
                            runtime_proxy_websocket_precommit_progress_timeout_ms(),
                        )),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to upstream websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(
                            runtime_proxy_websocket_precommit_progress_timeout_ms(),
                        )),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }
            }
            Ok(WsMessage::Close(frame)) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "websocket_reuse_watchdog",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field("event", "upstream_close_before_terminal"),
                                runtime_proxy_log_field(
                                    "elapsed_ms",
                                    started_at.elapsed().as_millis().to_string(),
                                ),
                                runtime_proxy_log_field("committed", committed.to_string()),
                            ],
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "upstream_close_before_completed",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "websocket"),
                            runtime_proxy_log_field("profile", profile_name),
                        ],
                    ),
                );
                let _ = frame;
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_close",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_close_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "websocket_reuse_watchdog",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field("event", "connection_closed"),
                                runtime_proxy_log_field(
                                    "elapsed_ms",
                                    started_at.elapsed().as_millis().to_string(),
                                ),
                                runtime_proxy_log_field("committed", committed.to_string()),
                            ],
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "upstream_connection_closed",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "websocket"),
                            runtime_proxy_log_field("profile", profile_name),
                        ],
                    ),
                );
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_connection_closed",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "connection_closed_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(err) => {
                websocket_session.reset();
                if !committed && precommit_hold_count > 0 && runtime_websocket_timeout_error(&err) {
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    let timeout_ms = runtime_proxy_websocket_precommit_progress_timeout_ms();
                    runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "websocket_precommit_hold_timeout",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field("elapsed_ms", elapsed_ms.to_string()),
                                runtime_proxy_log_field("threshold_ms", timeout_ms.to_string()),
                                runtime_proxy_log_field(
                                    "reuse",
                                    reuse_existing_session.to_string(),
                                ),
                                runtime_proxy_log_field(
                                    "hold_count",
                                    precommit_hold_count.to_string(),
                                ),
                            ],
                        ),
                    );
                    let transport_error = anyhow::anyhow!(
                        "runtime websocket upstream remained in pre-commit hold beyond the progress deadline after {elapsed_ms} ms: {err}"
                    );
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        "websocket_precommit_hold_timeout",
                        &transport_error,
                    );
                    if reuse_existing_session {
                        if let Some(started_at) = reuse_started_at {
                            runtime_proxy_log(
                                shared,
                                runtime_proxy_structured_log_message(
                                    "websocket_reuse_watchdog",
                                    [
                                        runtime_proxy_log_field("profile", profile_name),
                                        runtime_proxy_log_field("event", "precommit_hold_timeout"),
                                        runtime_proxy_log_field(
                                            "elapsed_ms",
                                            started_at.elapsed().as_millis().to_string(),
                                        ),
                                        runtime_proxy_log_field("committed", committed.to_string()),
                                    ],
                                ),
                            );
                        }
                        return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                            profile_name: profile_name.to_string(),
                            event: "precommit_hold_timeout",
                        });
                    }
                    return Err(transport_error);
                }
                if !committed && !first_upstream_frame_seen && runtime_websocket_timeout_error(&err)
                {
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "websocket_precommit_frame_timeout",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field(
                                    "event",
                                    "no_first_upstream_frame_before_deadline",
                                ),
                                runtime_proxy_log_field("elapsed_ms", elapsed_ms.to_string()),
                                runtime_proxy_log_field(
                                    "reuse",
                                    reuse_existing_session.to_string(),
                                ),
                            ],
                        ),
                    );
                    let transport_error = anyhow::anyhow!(
                        "runtime websocket upstream produced no first frame before the pre-commit deadline: {err}"
                    );
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        "websocket_first_frame_timeout",
                        &transport_error,
                    );
                    if reuse_existing_session {
                        runtime_proxy_log(
                            shared,
                            runtime_proxy_structured_log_message(
                                "websocket_reuse_watchdog",
                                [
                                    runtime_proxy_log_field("profile", profile_name),
                                    runtime_proxy_log_field(
                                        "event",
                                        "no_first_upstream_frame_before_deadline",
                                    ),
                                    runtime_proxy_log_field("elapsed_ms", elapsed_ms.to_string()),
                                    runtime_proxy_log_field("committed", committed.to_string()),
                                ],
                            ),
                        );
                        return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                            profile_name: profile_name.to_string(),
                            event: "no_first_upstream_frame_before_deadline",
                        });
                    }
                    return Err(transport_error);
                }
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "websocket_reuse_watchdog",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field("event", "read_error"),
                                runtime_proxy_log_field(
                                    "elapsed_ms",
                                    started_at.elapsed().as_millis().to_string(),
                                ),
                                runtime_proxy_log_field("committed", committed.to_string()),
                            ],
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "upstream_read_error",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "websocket"),
                            runtime_proxy_log_field("profile", profile_name),
                            runtime_proxy_log_field("error", err.to_string()),
                        ],
                    ),
                );
                let transport_error = anyhow::anyhow!(
                    "runtime websocket upstream failed before response.completed: {err}"
                );
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_read",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_read_error",
                    });
                }
                return Err(transport_error);
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/websocket.rs"]
mod tests;

use super::*;

pub(crate) fn runtime_resolve_websocket_tcp_addrs(
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

pub(crate) fn connect_runtime_proxy_upstream_tcp_stream(
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

pub(crate) fn connect_runtime_proxy_upstream_tcp_stream_via_http_proxy(
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

pub(crate) fn connect_runtime_proxy_tcp_stream_to_host(
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

pub(crate) fn runtime_websocket_target_from_uri(
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
pub(crate) struct RuntimeWebsocketHttpProxy {
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

pub(crate) fn runtime_websocket_http_proxy_for_uri(
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

pub(crate) fn runtime_websocket_proxy_env_url(scheme: &str) -> Option<reqwest::Url> {
    runtime_websocket_proxy_env_keys(scheme)
        .iter()
        .find_map(|key| {
            let value = env::var_os(key)?;
            runtime_websocket_parse_proxy_url(&value.to_string_lossy())
        })
}

pub(crate) fn runtime_websocket_parse_proxy_url(value: &str) -> Option<reqwest::Url> {
    let candidate = runtime_websocket_proxy_url_candidate(value)?;
    reqwest::Url::parse(&candidate).ok()
}

pub(crate) fn runtime_websocket_no_proxy_matches(host: &str, port: u16) -> bool {
    ["NO_PROXY", "no_proxy"].into_iter().any(|key| {
        env::var_os(key).is_some_and(|value| {
            runtime_websocket_no_proxy_value_matches(&value.to_string_lossy(), host, port)
        })
    })
}

pub(crate) fn runtime_websocket_establish_http_proxy_tunnel(
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

pub(crate) fn send_runtime_proxy_websocket_error(
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

pub(crate) fn forward_runtime_proxy_websocket_error(
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

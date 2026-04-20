use super::*;

#[derive(Default)]
pub(crate) struct RuntimeWebsocketSessionState {
    upstream_socket: Option<RuntimeUpstreamWebSocket>,
    pub(super) profile_name: Option<String>,
    pub(super) turn_state: Option<String>,
    inflight_guard: Option<RuntimeProfileInFlightGuard>,
    last_terminal_at: Option<Instant>,
}

impl RuntimeWebsocketSessionState {
    pub(super) fn can_reuse(&self, profile_name: &str, turn_state_override: Option<&str>) -> bool {
        self.upstream_socket.is_some()
            && self.profile_name.as_deref() == Some(profile_name)
            && turn_state_override.is_none_or(|value| self.turn_state.as_deref() == Some(value))
    }

    pub(super) fn take_socket(&mut self) -> Option<RuntimeUpstreamWebSocket> {
        self.upstream_socket.take()
    }

    pub(super) fn last_terminal_elapsed(&self) -> Option<Duration> {
        self.last_terminal_at.map(|timestamp| timestamp.elapsed())
    }

    pub(super) fn store(
        &mut self,
        socket: RuntimeUpstreamWebSocket,
        profile_name: &str,
        turn_state: Option<String>,
        inflight_guard: Option<RuntimeProfileInFlightGuard>,
    ) {
        self.upstream_socket = Some(socket);
        self.profile_name = Some(profile_name.to_string());
        self.turn_state = turn_state;
        self.last_terminal_at = Some(Instant::now());
        if let Some(inflight_guard) = inflight_guard {
            self.inflight_guard = Some(inflight_guard);
        }
    }

    pub(super) fn reset(&mut self) {
        self.upstream_socket = None;
        self.profile_name = None;
        self.turn_state = None;
        self.inflight_guard = None;
    }

    pub(super) fn close(&mut self) {
        if let Some(mut socket) = self.upstream_socket.take() {
            let _ = socket.close(None);
        }
        self.profile_name = None;
        self.turn_state = None;
        self.inflight_guard = None;
    }
}

pub(crate) fn acquire_runtime_profile_inflight_guard(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &'static str,
) -> Result<RuntimeProfileInFlightGuard> {
    let weight = runtime_profile_inflight_weight(context);
    let count = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let count = runtime
            .profile_inflight
            .entry(profile_name.to_string())
            .or_insert(0);
        *count = count.saturating_add(weight);
        *count
    };
    runtime_proxy_log(
        shared,
        format!(
            "profile_inflight profile={profile_name} count={count} weight={weight} context={context} event=acquire"
        ),
    );
    Ok(RuntimeProfileInFlightGuard {
        shared: shared.clone(),
        profile_name: profile_name.to_string(),
        context,
        weight,
    })
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeProfileUnauthorizedRecoveryStep {
    Reload,
    Refresh,
}

type RuntimeProfileUnauthorizedRecoverySteps =
    std::array::IntoIter<RuntimeProfileUnauthorizedRecoveryStep, 2>;

struct RuntimeProfileUnauthorizedRecoveryOutcome {
    source: &'static str,
    changed: bool,
}

impl RuntimeProfileUnauthorizedRecoveryStep {
    pub(super) fn ordered() -> RuntimeProfileUnauthorizedRecoverySteps {
        [Self::Reload, Self::Refresh].into_iter()
    }

    fn recover(
        self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
    ) -> Result<Option<RuntimeProfileUnauthorizedRecoveryOutcome>> {
        let (cached_entry, codex_home) = {
            let runtime = shared
                .runtime
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
            let profile = runtime
                .state
                .profiles
                .get(profile_name)
                .with_context(|| format!("profile '{}' is missing", profile_name))?;
            (
                runtime.profile_usage_auth.get(profile_name).cloned(),
                profile.codex_home.clone(),
            )
        };

        match self {
            RuntimeProfileUnauthorizedRecoveryStep::Reload => {
                let entry = load_runtime_profile_usage_auth_cache_entry(&codex_home)?;
                let auth_changed = cached_entry
                    .as_ref()
                    .is_some_and(|cached_entry| cached_entry.auth != entry.auth);
                if !auth_changed {
                    return Ok(None);
                }
                update_runtime_profile_usage_auth_cache_entry(
                    shared,
                    profile_name,
                    cached_entry.as_ref().map(|entry| &entry.auth),
                    entry,
                    "auth_reloaded",
                );
                Ok(Some(RuntimeProfileUnauthorizedRecoveryOutcome {
                    source: "reloaded",
                    changed: true,
                }))
            }
            RuntimeProfileUnauthorizedRecoveryStep::Refresh => {
                let outcome = sync_usage_auth_from_disk_or_refresh(
                    &codex_home,
                    cached_entry.as_ref().map(|entry| &entry.auth),
                )?;
                let entry = load_runtime_profile_usage_auth_cache_entry(&codex_home)?;
                update_runtime_profile_usage_auth_cache_entry(
                    shared,
                    profile_name,
                    cached_entry.as_ref().map(|entry| &entry.auth),
                    entry,
                    &format!("auth_{}", usage_auth_sync_source_label(outcome.source)),
                );
                Ok(Some(RuntimeProfileUnauthorizedRecoveryOutcome {
                    source: usage_auth_sync_source_label(outcome.source),
                    changed: outcome.auth_changed,
                }))
            }
        }
    }
}

fn runtime_try_recover_profile_auth_from_unauthorized(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    step: RuntimeProfileUnauthorizedRecoveryStep,
) -> bool {
    let attempt = (|| -> Result<bool> {
        let Some(outcome) = step.recover(shared, profile_name)? else {
            return Ok(false);
        };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} profile_auth_recovered profile={profile_name} route={} source={} changed={}",
                runtime_route_kind_label(route_kind),
                outcome.source,
                outcome.changed,
            ),
        );
        Ok(true)
    })();

    match attempt {
        Ok(recovered) => recovered,
        Err(err) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} profile_auth_recovery_failed profile={profile_name} route={} error={err}",
                    runtime_route_kind_label(route_kind),
                ),
            );
            false
        }
    }
}

pub(super) fn runtime_try_recover_profile_auth_from_unauthorized_steps(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    recovery_steps: &mut RuntimeProfileUnauthorizedRecoverySteps,
) -> bool {
    for step in recovery_steps.by_ref() {
        if runtime_try_recover_profile_auth_from_unauthorized(
            request_id,
            shared,
            profile_name,
            route_kind,
            step,
        ) {
            return true;
        }
    }
    false
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
            format!(
                "request={request_id} transport=websocket upstream_connect_start profile={profile_name} url={upstream_url} turn_state_override={:?}",
                turn_state_override
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
        match connect_runtime_proxy_upstream_websocket_with_timeout(request) {
            Ok((socket, response, selected_addr, resolved_addrs, attempted_addrs)) => {
                return Ok(RuntimeWebsocketConnectResult::Connected {
                    socket,
                    turn_state: {
                        let turn_state = runtime_proxy_tungstenite_header_value(
                            response.headers(),
                            "x-codex-turn-state",
                        );
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=websocket upstream_connect_ok profile={profile_name} status={} addr={} resolved_addrs={} attempted_addrs={} turn_state={:?}",
                                response.status().as_u16(),
                                selected_addr,
                                resolved_addrs,
                                attempted_addrs,
                                turn_state
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
                    format!(
                        "request={request_id} transport=websocket upstream_connect_http profile={profile_name} status={status} body_bytes={}",
                        body.len()
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
                let failure_kind = runtime_transport_failure_kind_from_ws(&err);
                log_runtime_upstream_connect_failure(
                    shared,
                    request_id,
                    "websocket",
                    profile_name,
                    failure_kind,
                    &err,
                );
                let transport_error =
                    anyhow::anyhow!("failed to connect runtime websocket upstream: {err}");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_connect",
                    &transport_error,
                );
                return Err(transport_error);
            }
        }
    }
}

pub(super) fn runtime_websocket_error_payload_from_http_body(
    body: &[u8],
) -> RuntimeWebsocketErrorPayload {
    if body.is_empty() {
        return RuntimeWebsocketErrorPayload::Empty;
    }

    match std::str::from_utf8(body) {
        Ok(text) => RuntimeWebsocketErrorPayload::Text(text.to_string()),
        Err(_) => RuntimeWebsocketErrorPayload::Binary(body.to_vec()),
    }
}

pub(super) fn connect_runtime_proxy_upstream_websocket_with_timeout(
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
    let stream = connect_runtime_proxy_upstream_tcp_stream(request.uri())?;
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

pub(super) fn runtime_interleave_socket_addrs(addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    let (mut primary, mut secondary): (VecDeque<_>, VecDeque<_>) =
        addrs.into_iter().partition(|addr| addr.is_ipv6());
    let prefer_ipv6 = primary.front().is_some();
    if !prefer_ipv6 {
        std::mem::swap(&mut primary, &mut secondary);
    }

    let mut ordered = Vec::with_capacity(primary.len().saturating_add(secondary.len()));
    loop {
        let mut progressed = false;
        if let Some(addr) = primary.pop_front() {
            ordered.push(addr);
            progressed = true;
        }
        if let Some(addr) = secondary.pop_front() {
            ordered.push(addr);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    ordered
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

pub(super) fn runtime_launch_websocket_tcp_connect_attempt(
    sender: mpsc::Sender<RuntimeWebsocketTcpAttemptResult>,
    addr: SocketAddr,
    connect_timeout: Duration,
) {
    thread::spawn(move || {
        let result = TcpStream::connect_timeout(&addr, connect_timeout);
        let _ = sender.send(RuntimeWebsocketTcpAttemptResult { addr, result });
    });
}

pub(super) fn connect_runtime_proxy_upstream_tcp_stream(
    uri: &tungstenite::http::Uri,
) -> std::result::Result<RuntimeWebsocketTcpConnectSuccess, WsError> {
    let host = uri.host().ok_or(WsError::Url(WsUrlError::NoHostName))?;
    let host = if host.starts_with('[') && host.ends_with(']') {
        &host[1..host.len() - 1]
    } else {
        host
    };
    let port = uri.port_u16().unwrap_or(match uri.scheme_str() {
        Some("wss") => 443,
        _ => 80,
    });
    let connect_timeout = Duration::from_millis(runtime_proxy_websocket_connect_timeout_ms());
    let io_timeout = Duration::from_millis(runtime_proxy_websocket_precommit_progress_timeout_ms());
    let happy_eyeballs_delay =
        Duration::from_millis(runtime_proxy_websocket_happy_eyeballs_delay_ms());
    let addrs = runtime_interleave_socket_addrs(
        (host, port)
            .to_socket_addrs()
            .map_err(WsError::Io)?
            .collect(),
    );
    if addrs.is_empty() {
        return Err(WsError::Url(WsUrlError::UnableToConnect(uri.to_string())));
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
        None => Err(WsError::Url(WsUrlError::UnableToConnect(uri.to_string()))),
    }
}

pub(super) fn send_runtime_proxy_websocket_error(
    local_socket: &mut RuntimeLocalWebSocket,
    status: u16,
    code: &str,
    message: &str,
) -> Result<()> {
    let payload = serde_json::json!({
        "type": "error",
        "status": status,
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string();
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
pub(super) struct RuntimeWebsocketResponseBindingContext<'a> {
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: &'a str,
    pub(super) request_previous_response_id: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) response_turn_state: Option<&'a str>,
}

pub(super) fn remember_runtime_websocket_response_ids(
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
    if !response_ids.is_empty() {
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
        local_socket
            .send(WsMessage::Text(frame.text.into()))
            .context("failed to forward buffered runtime websocket text frame")?;
    }
    Ok(())
}

pub(super) fn inspect_runtime_websocket_text_frame(
    payload: &str,
) -> RuntimeInspectedWebsocketTextFrame {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return RuntimeInspectedWebsocketTextFrame::default();
    };

    let event_type = runtime_response_event_type_from_value(&value);
    let retry_kind = if extract_runtime_proxy_previous_response_message_from_value(&value).is_some()
    {
        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound)
    } else if extract_runtime_proxy_overload_message_from_value(&value).is_some() {
        Some(RuntimeWebsocketRetryInspectionKind::Overloaded)
    } else if extract_runtime_proxy_quota_message_from_value(&value).is_some() {
        Some(RuntimeWebsocketRetryInspectionKind::QuotaBlocked)
    } else {
        None
    };
    let precommit_hold = event_type
        .as_deref()
        .is_some_and(runtime_proxy_precommit_hold_event_kind);
    let terminal_event = event_type
        .as_deref()
        .is_some_and(|kind| matches!(kind, "response.completed" | "response.failed"));

    RuntimeInspectedWebsocketTextFrame {
        event_type,
        turn_state: extract_runtime_turn_state_from_value(&value),
        response_ids: extract_runtime_response_ids_from_value(&value),
        retry_kind,
        precommit_hold,
        terminal_event,
    }
}

pub(super) fn runtime_response_event_type_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
pub(super) fn runtime_response_event_type(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|value| runtime_response_event_type_from_value(&value))
}

pub(super) fn runtime_proxy_precommit_hold_event_kind(kind: &str) -> bool {
    matches!(
        kind,
        "response.created"
            | "response.in_progress"
            | "response.queued"
            | "response.output_item.added"
            | "response.content_part.added"
            | "response.reasoning_summary_part.added"
    )
}

pub(super) fn runtime_realtime_websocket_terminal_event_kind(kind: &str) -> bool {
    matches!(
        kind,
        "session.updated"
            | "conversation.item.added"
            | "conversation.item.done"
            | "response.cancelled"
            | "response.done"
            | "error"
    )
}

#[cfg(test)]
pub(crate) fn is_runtime_terminal_event(payload: &str) -> bool {
    runtime_response_event_type(payload)
        .is_some_and(|kind| matches!(kind.as_str(), "response.completed" | "response.failed"))
}

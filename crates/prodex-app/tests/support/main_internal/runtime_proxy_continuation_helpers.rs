use super::*;

pub(super) fn wait_for_runtime_continuations<F>(
    paths: &AppPaths,
    predicate: F,
) -> RuntimeContinuationStore
where
    F: Fn(&RuntimeContinuationStore) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_continuations = None;
    loop {
        if let Ok(continuations) = load_runtime_continuations_with_recovery(
            paths,
            &AppState::load(paths).unwrap_or_default().profiles,
        )
        .map(|loaded| loaded.value)
        {
            if predicate(&continuations) {
                return continuations;
            }
            last_continuations = Some(continuations);
        }
        if Instant::now() >= deadline {
            let continuations = load_runtime_continuations_with_recovery(
                paths,
                &AppState::load(paths).unwrap_or_default().profiles,
            )
            .map(|loaded| loaded.value)
            .expect("runtime continuations should reload");
            panic!(
                "timed out waiting for runtime continuations predicate; last_continuations={:?} final_continuations={:?}",
                last_continuations, continuations
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn read_runtime_http_stream_until<F>(
    mut response: reqwest::blocking::Response,
    predicate: F,
) -> String
where
    F: Fn(&str) -> bool,
{
    use std::io::Read as _;

    let mut body = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        match response.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                body.extend_from_slice(&chunk[..read]);
                if predicate(&String::from_utf8_lossy(&body)) {
                    break;
                }
            }
            Err(err) => panic!("HTTP stream body should read: {err}"),
        }
    }
    String::from_utf8_lossy(&body).into_owned()
}

pub(super) fn dead_continuation_status(now: i64) -> RuntimeContinuationBindingStatus {
    RuntimeContinuationBindingStatus {
        state: RuntimeContinuationBindingLifecycle::Dead,
        confidence: 0,
        last_touched_at: Some(now),
        last_verified_at: Some(now.saturating_sub(5)),
        last_verified_route: Some("responses".to_string()),
        last_not_found_at: Some(now),
        not_found_streak: RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT,
        success_count: 1,
        failure_count: 1,
    }
}

pub(super) struct RuntimeContinuationHeader {
    name: &'static str,
    value: String,
}

pub(super) type RuntimeContinuationClientWebSocket = WsSocket<MaybeTlsStream<TcpStream>>;

pub(super) fn runtime_continuation_header(
    name: &'static str,
    value: impl Into<String>,
) -> RuntimeContinuationHeader {
    RuntimeContinuationHeader {
        name,
        value: value.into(),
    }
}

pub(super) fn codex_0135_compaction_turn_metadata() -> String {
    concat!(
        r#"{"request_kind":"compaction","#,
        r#""session_id":"sess-rich-metadata","#,
        r#""thread_id":"thread-rich-metadata","#,
        r#""turn_id":"turn-rich-metadata","#,
        r#""window_id":"thread-rich-metadata:2","#,
        r#""compaction":{"trigger":"token_limit","#,
        r#""reason":"context_window_full","#,
        r#""implementation":"responses_compact","#,
        r#""phase":"pre_turn","#,
        r#""strategy":"memento"}}"#
    )
    .to_string()
}

pub(super) struct RuntimeContinuationFixture {
    proxy: RuntimeRotationProxy,
    pub(super) backend: RuntimeProxyBackend,
    pub(super) paths: AppPaths,
    _temp_dir: TestDir,
}

pub(super) fn start_runtime_continuation_fixture(
    backend: RuntimeProxyBackend,
    active_profile: &str,
    profiles: &[&str],
    response_bindings: &[(&str, &str)],
    session_bindings: Vec<(String, &str)>,
) -> RuntimeContinuationFixture {
    let temp_dir = TestDir::isolated();
    let now = Local::now().timestamp();

    let profiles = profiles
        .iter()
        .map(|profile_name| {
            let codex_home = temp_dir.path.join(format!("homes/{profile_name}"));
            let account_id = runtime_proxy_backend_account_id_for_profile_name(profile_name)
                .unwrap_or_else(|| panic!("missing backend account id for profile {profile_name}"));
            write_auth_json(&codex_home.join("auth.json"), account_id);
            (
                (*profile_name).to_string(),
                ProfileEntry {
                    codex_home,
                    managed: true,
                    email: Some(format!("{profile_name}@example.com")),
                    provider: ProfileProvider::Openai,
                },
            )
        })
        .collect();

    let state = AppState {
        active_profile: Some(active_profile.to_string()),
        profiles,
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: response_bindings
            .iter()
            .map(|(response_id, profile_name)| {
                (
                    (*response_id).to_string(),
                    ResponseProfileBinding {
                        binding_identity: None,
                profile_name: (*profile_name).to_string(),
                        bound_at: now,
                    },
                )
            })
            .collect(),
        session_profile_bindings: session_bindings
            .into_iter()
            .map(|(session_key, profile_name)| {
                (
                    session_key,
                    ResponseProfileBinding {
                        binding_identity: None,
                profile_name: profile_name.to_string(),
                        bound_at: now,
                    },
                )
            })
            .collect(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy =
        start_runtime_rotation_proxy(&paths, &state, active_profile, backend.base_url(), false)
            .expect("runtime proxy should start");

    RuntimeContinuationFixture {
        proxy,
        backend,
        paths,
        _temp_dir: temp_dir,
    }
}

impl RuntimeContinuationFixture {
    pub(super) fn restart(self) -> Self {
        let Self {
            proxy,
            backend,
            paths,
            _temp_dir,
        } = self;
        drop(proxy);
        let state = AppState::load(&paths).expect("runtime state should reload after restart");
        let active_profile = state
            .active_profile
            .as_deref()
            .expect("reloaded runtime state should have an active profile");
        let proxy = start_runtime_rotation_proxy(
            &paths,
            &state,
            active_profile,
            backend.base_url(),
            false,
        )
        .expect("runtime proxy should restart");
        Self {
            proxy,
            backend,
            paths,
            _temp_dir,
        }
    }

    pub(super) fn restart_with_stale_session_binding(
        self,
        session_id: &str,
        profile_name: &str,
    ) -> Self {
        wait_for_runtime_background_queues_idle();
        let mut state = AppState::load(&self.paths).expect("runtime state should load");
        let binding_identity = prodex_provider_core::RuntimeProviderBindingIdentity::from_profile(
            prodex_provider_core::ProviderId::OpenAi,
            profile_name,
            "https://stale.example.com/v1",
        )
        .expect("stale binding identity should resolve");
        state.session_profile_bindings.insert(
            session_id.to_string(),
            ResponseProfileBinding {
                binding_identity: Some(binding_identity),
                profile_name: profile_name.to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
        state.save(&self.paths).expect("runtime state should save");
        self.restart()
    }

    pub(super) fn restart_with_transport_backoff(
        self,
        profile_name: &str,
        route_kind: RuntimeRouteKind,
    ) -> Self {
        wait_for_runtime_background_queues_idle();
        save_runtime_profile_backoffs(
            &self.paths,
            &RuntimeProfileBackoffs {
                updated_at: BTreeMap::new(),
                retry_backoff_until: BTreeMap::new(),
                transport_backoff_until: BTreeMap::from([(
                    runtime_profile_transport_backoff_key(profile_name, route_kind),
                    Local::now().timestamp() + 60,
                )]),
                route_circuit_open_until: BTreeMap::new(),
            },
        )
        .expect("transport backoff should save");
        self.restart()
    }

    pub(super) fn restart_with_journal_continuations(
        self,
        continuations: RuntimeContinuationStore,
    ) -> Self {
        let Self {
            proxy,
            backend,
            paths,
            _temp_dir,
        } = self;
        drop(proxy);
        wait_for_runtime_background_queues_idle();
        let state = AppState::load(&paths).expect("runtime state should reload after restart");
        save_runtime_continuation_journal_for_profiles(
            &paths,
            &continuations,
            &state.profiles,
            Local::now().timestamp(),
        )
        .expect("journal-only continuations should save");
        let active_profile = state
            .active_profile
            .as_deref()
            .expect("reloaded runtime state should have an active profile");
        let proxy = start_runtime_rotation_proxy(
            &paths,
            &state,
            active_profile,
            backend.base_url(),
            false,
        )
        .expect("runtime proxy should restart from continuation journal");
        Self {
            proxy,
            backend,
            paths,
            _temp_dir,
        }
    }

    pub(super) fn post_json(
        &self,
        route: &str,
        body: serde_json::Value,
    ) -> reqwest::blocking::Response {
        self.post_json_with_headers(route, &[], body)
    }

    pub(super) fn post_json_with_headers(
        &self,
        route: &str,
        headers: &[RuntimeContinuationHeader],
        body: serde_json::Value,
    ) -> reqwest::blocking::Response {
        let mut request = Client::builder()
            .build()
            .expect("client")
            .post(format!("http://{}/{}", self.proxy.listen_addr, route))
            .header(reqwest::header::CONTENT_TYPE, "application/json");
        for header in headers {
            request = request.header(header.name, header.value.as_str());
        }
        request
            .body(body.to_string())
            .send()
            .expect("request should succeed")
    }

    pub(super) fn connect_websocket(&self, route: &str) -> RuntimeContinuationClientWebSocket {
        self.connect_websocket_with_headers(route, &[])
    }

    pub(super) fn connect_realtime_websocket(
        &self,
        route: &str,
    ) -> RuntimeContinuationClientWebSocket {
        let addr = self
            .proxy
            .realtime_ws_sidecar_addr
            .expect("runtime realtime websocket sidecar should start");
        let (mut socket, _response) =
            ws_connect(format!("ws://{addr}/{route}")).expect("realtime websocket should connect");
        set_test_websocket_io_timeout(&mut socket, ci_timing_upper_bound_ms(1_000, 10_000));
        socket
    }

    pub(super) fn connect_websocket_with_headers(
        &self,
        route: &str,
        headers: &[RuntimeContinuationHeader],
    ) -> RuntimeContinuationClientWebSocket {
        use tungstenite::client::IntoClientRequest;

        let mut request = format!("ws://{}/{}", self.proxy.listen_addr, route)
            .into_client_request()
            .expect("websocket client request should build");
        for header in headers {
            request.headers_mut().insert(
                tungstenite::http::HeaderName::from_bytes(header.name.as_bytes())
                    .expect("websocket header name should be valid"),
                tungstenite::http::HeaderValue::from_str(header.value.as_str())
                    .expect("websocket header value should be valid"),
            );
        }
        let (mut socket, _response) =
            ws_connect(request).expect("websocket client should connect");
        set_test_websocket_io_timeout(&mut socket, ci_timing_upper_bound_ms(1_000, 10_000));
        socket
    }

    pub(super) fn wait_for_log<F>(&self, predicate: F) -> String
    where
        F: Fn(&str) -> bool,
    {
        let log_tail = wait_for_runtime_log_tail_until(
            || fs::read(&self.proxy.log_path).ok(),
            predicate,
            2_000,
            5_000,
            20,
        );
        String::from_utf8_lossy(&log_tail).into_owned()
    }
}

pub(super) fn assert_single_recorded_request<'a>(
    requests: &'a [String],
    len_message: &str,
) -> &'a str {
    assert_eq!(requests.len(), 1, "{len_message}: {requests:?}");
    &requests[0]
}

pub(super) fn assert_request_json_field(request: &str, field: &str, value: &str, message: &str) {
    let fragment = format!("\"{field}\":\"{value}\"");
    assert!(request.contains(&fragment), "{message}: {request}");
}

pub(super) fn assert_all_requests_json_field(
    requests: &[String],
    field: &str,
    value: &str,
    empty_message: &str,
    message: &str,
) {
    assert!(!requests.is_empty(), "{empty_message}");
    for request in requests {
        assert_request_json_field(request, field, value, message);
    }
}

pub(super) fn send_runtime_websocket_json(
    socket: &mut RuntimeContinuationClientWebSocket,
    payload: serde_json::Value,
) {
    socket
        .send(WsMessage::Text(payload.to_string().into()))
        .expect("websocket request should send");
}

pub(super) fn read_runtime_websocket_text(
    socket: &mut RuntimeContinuationClientWebSocket,
) -> String {
    loop {
        match socket.read().expect("websocket response should read") {
            WsMessage::Text(text) => return text.to_string(),
            WsMessage::Ping(payload) => socket
                .send(WsMessage::Pong(payload))
                .expect("websocket pong should send"),
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket response: {other:?}"),
        }
    }
}

pub(super) fn read_runtime_websocket_until<F>(
    socket: &mut RuntimeContinuationClientWebSocket,
    mut predicate: F,
) -> (Vec<String>, String)
where
    F: FnMut(&str) -> bool,
{
    let mut frames = Vec::new();
    loop {
        match socket.read().expect("websocket response should read") {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let done = predicate(&text);
                frames.push(text.clone());
                if done {
                    return (frames, text);
                }
            }
            WsMessage::Ping(payload) => socket
                .send(WsMessage::Pong(payload))
                .expect("websocket pong should send"),
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket response: {other:?}"),
        }
    }
}

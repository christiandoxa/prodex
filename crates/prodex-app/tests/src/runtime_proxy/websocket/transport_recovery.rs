//! Regressions for upstream failures and quiet precommit Responses streams.
use super::*;
use std::sync::mpsc;

#[test]
fn websocket_precommit_continuation_close_reaches_outer_recovery_without_raw_error() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let shared = websocket_test_shared_with_main_profile(
        "continuation-transport-close",
        listener.local_addr().unwrap(),
    );
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.created","response":{"id":"resp-pending"}}"#.into(),
            ))
            .unwrap();
        socket.close(None).unwrap();
    });
    let (mut local, _client) = websocket_test_local_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    let handshake = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let result = attempt_runtime_websocket_request_with_hard_affinity(
        RuntimeWebsocketAttemptRequest {
            request_id: 801,
            local_socket: &mut local,
            handshake_request: &handshake,
            request_text: r#"{"type":"response.create","previous_response_id":"resp-owner","input":[]}"#,
            request_previous_response_id: Some("resp-owner"),
            request_prompt_cache_key: None,
            request_session_id: Some("session-owner"),
            request_turn_state: Some("turn-owner"),
            shared: &shared,
            websocket_session: &mut session,
            profile_name: "main",
            turn_state_override: Some("turn-owner"),
            promote_committed_profile: false,
        },
        true,
    );
    backend.join().unwrap();
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    assert!(!session.has_socket());
    assert!(
        matches!(result, Ok(RuntimeWebsocketAttempt::TransportFailed { .. })),
        "precommit transport failure must reach the affinity-aware outer handler, got {result:?}"
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_buffered_prelude_keeps_client_transport_alive_without_committing() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut shared = websocket_test_shared_with_main_profile(
        "quiet-prelude-heartbeat",
        listener.local_addr().unwrap(),
    );
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .stream_idle_timeout_ms = 10_000;
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .websocket_precommit_progress_timeout_ms = 5_000;
    shared.precommit_deadline = Some(Instant::now() + Duration::from_secs(10));
    let (release, wait) = mpsc::channel();
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.created","response":{"id":"resp-quiet"}}"#.into(),
            ))
            .unwrap();
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-quiet"}}"#.into(),
            ))
            .unwrap();
    });
    let (mut local, mut client) = websocket_test_local_pair();
    crate::runtime_set_upstream_websocket_io_timeout(&mut client, Some(Duration::from_secs(2)))
        .unwrap();
    let mut session = RuntimeWebsocketSessionState::default();
    let handshake = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let (first, outcome) = thread::scope(|scope| {
        let pending = scope.spawn(|| {
            attempt_runtime_websocket_request_with_hard_affinity(
                RuntimeWebsocketAttemptRequest {
                    request_id: 802,
                    local_socket: &mut local,
                    handshake_request: &handshake,
                    request_text: r#"{"type":"response.create","input":[]}"#,
                    request_previous_response_id: None,
                    request_prompt_cache_key: None,
                    request_session_id: None,
                    request_turn_state: None,
                    shared: &shared,
                    websocket_session: &mut session,
                    profile_name: "main",
                    turn_state_override: None,
                    promote_committed_profile: true,
                },
                false,
            )
        });
        let first = client.read();
        // Always release and join even when the baseline times out.
        release.send(()).unwrap();
        (first, pending.join().unwrap())
    });
    backend.join().unwrap();
    assert!(
        matches!(first, Ok(WsMessage::Ping(_))),
        "quiet precommit must send transport-only liveness, not metadata/commit or timeout: {first:?}"
    );
    assert!(
        matches!(outcome, Ok(RuntimeWebsocketAttempt::Delivered)),
        "{outcome:?}"
    );
    let mut events = Vec::new();
    while events.len() < 2 {
        match client.read().unwrap() {
            WsMessage::Ping(_) | WsMessage::Pong(_) => continue,
            WsMessage::Text(text) => events.push(text.to_string()),
            other => panic!("unexpected completion frame: {other:?}"),
        }
    }
    assert!(events[0].contains("response.created"));
    assert!(events[1].contains("response.completed"));
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    session.close();
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_heartbeat_does_not_renew_upstream_idle_or_invent_quota_failure() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut shared = websocket_test_shared_with_main_profile(
        "heartbeat-real-idle-deadline",
        listener.local_addr().unwrap(),
    );
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .stream_idle_timeout_ms = 1500;
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .websocket_precommit_progress_timeout_ms = 5000;
    let (release, wait) = mpsc::channel();
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.created","response":{"id":"resp-idle"}}"#.into(),
            ))
            .unwrap();
        let _ = wait.recv_timeout(Duration::from_secs(5));
    });
    let (mut local, mut client) = websocket_test_local_pair();
    crate::runtime_set_upstream_websocket_io_timeout(&mut client, Some(Duration::from_secs(3)))
        .unwrap();
    let mut session = RuntimeWebsocketSessionState::default();
    let handshake = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let started = Instant::now();
    let result = attempt_runtime_websocket_request_with_hard_affinity(
        RuntimeWebsocketAttemptRequest {
            request_id: 803,
            local_socket: &mut local,
            handshake_request: &handshake,
            request_text: r#"{"type":"response.create","input":[]}"#,
            request_previous_response_id: None,
            request_prompt_cache_key: None,
            request_session_id: None,
            request_turn_state: None,
            shared: &shared,
            websocket_session: &mut session,
            profile_name: "main",
            turn_state_override: None,
            promote_committed_profile: true,
        },
        false,
    );
    release.send(()).unwrap();
    backend.join().unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "heartbeats must not renew upstream idle time"
    );
    assert!(
        matches!(result, Ok(RuntimeWebsocketAttempt::TransportFailed { .. })),
        "{result:?}"
    );
    assert!(matches!(client.read().unwrap(), WsMessage::Ping(_)));
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    assert_eq!(
        shared.runtime.lock().unwrap().profile_usage_snapshots["main"].weekly_remaining_percent,
        100
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_committed_transport_close_never_returns_a_retryable_attempt() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let shared = websocket_test_shared_with_main_profile(
        "committed-transport-no-replay",
        listener.local_addr().unwrap(),
    );
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.output_text.delta","delta":"once"}"#.into(),
            ))
            .unwrap();
        socket.close(None).unwrap();
    });
    let (mut local, mut client) = websocket_test_local_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    let handshake = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let result = attempt_runtime_websocket_request_with_hard_affinity(
        RuntimeWebsocketAttemptRequest {
            request_id: 804,
            local_socket: &mut local,
            handshake_request: &handshake,
            request_text: r#"{"type":"response.create","input":[]}"#,
            request_previous_response_id: None,
            request_prompt_cache_key: None,
            request_session_id: None,
            request_turn_state: None,
            shared: &shared,
            websocket_session: &mut session,
            profile_name: "main",
            turn_state_override: None,
            promote_committed_profile: true,
        },
        false,
    );
    backend.join().unwrap();
    assert!(
        result.is_err(),
        "committed response cannot be replayed: {result:?}"
    );
    assert!(client.read().unwrap().to_string().contains("once"));
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    let _ = std::fs::remove_file(&shared.log_path);
}

struct SlowMeaningfulWrite {
    inner: Box<dyn crate::TinyReadWrite + Send>,
    delayed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl std::io::Read for SlowMeaningfulWrite {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buffer)
    }
}

impl std::io::Write for SlowMeaningfulWrite {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let count = self.inner.write(buffer)?;
        let event = b"response.output_text.delta";
        if buffer[..count]
            .windows(event.len())
            .any(|part| part == event)
            && !self.delayed.swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            // Deliberately exceed upstream idle time inside host dispatch,
            // just as synchronous state persistence can on Windows.
            thread::sleep(Duration::from_millis(200));
        }
        Ok(count)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[test]
fn websocket_buffered_terminal_survives_slow_local_dispatch_without_false_idle() {
    let _guard = acquire_test_runtime_lock();
    for terminal in ["response.completed", "response.failed"] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut shared = websocket_test_shared_with_main_profile(
            &format!("buffered-after-slow-dispatch-{terminal}"),
            listener.local_addr().unwrap(),
        );
        let tuning = &mut Arc::make_mut(&mut shared.runtime_config).tuning;
        tuning.stream_idle_timeout_ms = 100;
        tuning.websocket_precommit_progress_timeout_ms = 5_000;
        let (release, wait) = mpsc::channel();
        let backend = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            socket.read().unwrap();
            for payload in [
                serde_json::json!({"type":"response.created","response":{"id":"resp-buffered"}}),
                serde_json::json!({"type":"response.output_text.delta","delta":"exactly-once"}),
                serde_json::json!({"type":terminal,"response":{"id":"resp-buffered"}}),
            ] {
                socket
                    .send(WsMessage::Text(payload.to_string().into()))
                    .unwrap();
            }
            let _ = wait.recv_timeout(Duration::from_secs(5));
        });
        let (local, mut client) = websocket_test_local_pair();
        let delayed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stream = SlowMeaningfulWrite {
            inner: local.into_inner(),
            delayed: Arc::clone(&delayed),
        };
        let mut local = tungstenite::WebSocket::from_raw_socket(
            Box::new(stream) as Box<dyn crate::TinyReadWrite + Send>,
            tungstenite::protocol::Role::Server,
            None,
        );
        let mut session = RuntimeWebsocketSessionState::default();
        let handshake = RuntimeProxyRequest {
            method: "GET".to_string(),
            path_and_query: "/backend-api/prodex/responses".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let outcome = attempt_runtime_websocket_request_with_hard_affinity(
            RuntimeWebsocketAttemptRequest {
                request_id: 805,
                local_socket: &mut local,
                handshake_request: &handshake,
                request_text: r#"{"type":"response.create","input":[]}"#,
                request_previous_response_id: None,
                request_prompt_cache_key: None,
                request_session_id: None,
                request_turn_state: None,
                shared: &shared,
                websocket_session: &mut session,
                profile_name: "main",
                turn_state_override: None,
                promote_committed_profile: false,
            },
            false,
        );
        release.send(()).unwrap();
        backend.join().unwrap();
        assert!(delayed.load(std::sync::atomic::Ordering::SeqCst));
        assert!(
            matches!(outcome, Ok(RuntimeWebsocketAttempt::Delivered)),
            "queued {terminal} must be consumed before declaring upstream idle after slow local dispatch: {outcome:?}"
        );
        let mut texts = Vec::new();
        while texts.len() < 3 {
            match client.read().unwrap() {
                WsMessage::Text(text) => texts.push(text.to_string()),
                WsMessage::Ping(_) | WsMessage::Pong(_) => {}
                other => panic!("unexpected queued response frame: {other:?}"),
            }
        }
        assert_eq!(
            texts
                .iter()
                .filter(|text| text.contains("exactly-once"))
                .count(),
            1
        );
        assert!(texts[2].contains(terminal));
        assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
        session.close();
        let _ = std::fs::remove_file(&shared.log_path);
    }
}

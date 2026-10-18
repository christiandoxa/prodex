use super::*;
use crate::runtime_proxy::RuntimeProxyRecoveryTimeout;
use std::sync::mpsc;

fn handshake() -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    }
}

fn attempt(
    local: &mut RuntimeLocalWebSocket,
    session: &mut RuntimeWebsocketSessionState,
    shared: &RuntimeRotationProxyShared,
    id: u64,
) -> Result<RuntimeWebsocketAttempt> {
    attempt_runtime_websocket_request_with_hard_affinity(
        RuntimeWebsocketAttemptRequest {
            request_id: id,
            local_socket: local,
            handshake_request: &handshake(),
            request_text: r#"{"type":"response.create","input":[]}"#,
            request_previous_response_id: None,
            request_prompt_cache_key: None,
            request_session_id: None,
            request_turn_state: None,
            shared,
            websocket_session: session,
            profile_name: "main",
            turn_state_override: None,
            promote_committed_profile: true,
        },
        false,
    )
}

#[test]
fn websocket_idle_completed_response_releases_and_reuse_reacquires_profile_capacity() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut shared = websocket_test_shared_with_main_profile(
        "idle-response-capacity",
        listener.local_addr().unwrap(),
    );
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .profile_inflight_hard_limit = 2;
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .stream_idle_timeout_ms = 5_000;
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .websocket_precommit_progress_timeout_ms = 5_000;
    let (mut local, _client) = websocket_test_local_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    let (second_seen, observed) = mpsc::channel();
    let (complete_second, permit_completion) = mpsc::channel();
    let (release_backend, keep_backend) = mpsc::channel();
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-idle-first"}}"#.into(),
            ))
            .unwrap();
        if socket.read().is_err() {
            return;
        }
        second_seen.send(()).unwrap();
        if permit_completion
            .recv_timeout(Duration::from_secs(10))
            .is_err()
        {
            return;
        }
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-idle-second"}}"#.into(),
            ))
            .unwrap();
        let _ = keep_backend.recv_timeout(Duration::from_secs(10));
    });
    assert!(matches!(
        attempt(&mut local, &mut session, &shared, 701).unwrap(),
        RuntimeWebsocketAttempt::Delivered
    ));
    assert!(
        session.can_reuse("main", None),
        "retain the actual socket, not just its profile label"
    );
    assert_eq!(
        shared.lane_admission.profile_inflight_count("main"),
        0,
        "a completed idle socket must not reserve model execution capacity"
    );
    let competing =
        try_acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http", false)
            .unwrap()
            .expect("another request can run while this socket remains idle");
    assert!(matches!(
        attempt(&mut local, &mut session, &shared, 702).unwrap(),
        RuntimeWebsocketAttempt::LocalSelectionBlocked {
            reason: "profile_inflight_saturated",
            ..
        }
    ));
    assert!(
        session.can_reuse("main", None),
        "backpressure must not destroy the idle connection lineage"
    );
    drop(competing);
    thread::scope(|scope| {
        let pending = scope.spawn(|| attempt(&mut local, &mut session, &shared, 703));
        observed.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(
            shared.lane_admission.profile_inflight_count("main"),
            2,
            "a reused connection must reacquire capacity for its active response"
        );
        complete_second.send(()).unwrap();
        assert!(matches!(
            pending.join().unwrap().unwrap(),
            RuntimeWebsocketAttempt::Delivered
        ));
    });
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    assert!(session.can_reuse("main", None));
    release_backend.send(()).unwrap();
    backend.join().unwrap();
    session.close();
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_hidden_prelude_cannot_outlive_the_logical_request_deadline() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut shared = websocket_test_shared_with_main_profile(
        "hidden-prelude-deadline",
        listener.local_addr().unwrap(),
    );
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .websocket_precommit_progress_timeout_ms = 5_000;
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .stream_idle_timeout_ms = 5_000;
    let (mut local, _client) = websocket_test_local_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    let (release_backend, keep_backend) = mpsc::channel();
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        socket
            .send(WsMessage::Text(r#"{"type":"response.in_progress"}"#.into()))
            .unwrap();
        let _ = keep_backend.recv_timeout(Duration::from_secs(3));
    });
    shared.precommit_deadline = Some(Instant::now() + Duration::from_secs(1));
    let result = attempt(&mut local, &mut session, &shared, 704);
    let _ = release_backend.send(());
    backend.join().unwrap();
    match result {
        Err(error) => assert!(
            error.is::<RuntimeProxyRecoveryTimeout>(),
            "expected typed request expiry, got {error:#}"
        ),
        Ok(_) => panic!(
            "the response loop must return the original request deadline, not a retryable account failure"
        ),
    }
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    assert!(!session.has_socket());
    let state = shared.runtime.lock().unwrap();
    assert!(
        state.profile_transport_backoff_until.is_empty(),
        "local expiry is not evidence of an account transport failure"
    );
    drop(state);
    let log = read_websocket_test_log_after_marker(
        &shared.log_path,
        "precommit_recovery_deadline_exceeded",
    );
    assert!(!log.contains("transport=websocket committed profile=main"));
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_committed_response_keeps_streaming_after_precommit_deadline() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut shared = websocket_test_shared_with_main_profile(
        "committed-stream-deadline",
        listener.local_addr().unwrap(),
    );
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .stream_idle_timeout_ms = 10_000;
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .websocket_precommit_progress_timeout_ms = 5_000;
    let (mut local, mut client) = websocket_test_local_pair();
    crate::runtime_set_upstream_websocket_io_timeout(&mut client, Some(Duration::from_secs(10)))
        .unwrap();
    let mut session = RuntimeWebsocketSessionState::default();
    let (complete, permit_completion) = mpsc::channel();
    let (release_backend, keep_backend) = mpsc::channel();
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.output_text.delta","delta":"once"}"#.into(),
            ))
            .unwrap();
        permit_completion
            .recv_timeout(Duration::from_secs(10))
            .unwrap();
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"committed-safe"}}"#.into(),
            ))
            .unwrap();
        let _ = keep_backend.recv_timeout(Duration::from_secs(10));
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    shared.precommit_deadline = Some(deadline);
    thread::scope(|scope| {
        let pending = scope.spawn(|| attempt(&mut local, &mut session, &shared, 705));
        let text = client.read().unwrap().into_text().unwrap();
        assert!(text.contains("response.output_text.delta"));
        // Synchronize on committed output before moving beyond the watchdog.
        thread::sleep(
            deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(30),
        );
        assert_eq!(shared.lane_admission.profile_inflight_count("main"), 2);
        complete.send(()).unwrap();
        // Control heartbeats must neither complete nor duplicate the response.
        loop {
            match client.read().unwrap() {
                WsMessage::Ping(_) | WsMessage::Pong(_) => continue,
                WsMessage::Text(text) => {
                    assert!(text.contains("response.completed"), "{text}");
                    break;
                }
                other => panic!("unexpected post-commit frame: {other:?}"),
            }
        }
        assert!(matches!(
            pending.join().unwrap().unwrap(),
            RuntimeWebsocketAttempt::Delivered
        ));
    });
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    release_backend.send(()).unwrap();
    backend.join().unwrap();
    session.close();
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_realtime_connection_retains_its_live_duplex_permit() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let shared = websocket_test_shared_with_main_profile(
        "realtime-live-permit",
        listener.local_addr().unwrap(),
    );
    let (mut local, _client) = websocket_test_local_pair();
    let mut session = RuntimeWebsocketSessionState::with_realtime_duplex(true);
    let (release_backend, keep_backend) = mpsc::channel();
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        let _ = keep_backend.recv_timeout(Duration::from_secs(10));
    });
    assert!(matches!(
        attempt(&mut local, &mut session, &shared, 706).unwrap(),
        RuntimeWebsocketAttempt::Delivered
    ));
    assert_eq!(
        shared.lane_admission.profile_inflight_count("main"),
        2,
        "a realtime duplex conversation remains active, not an idle completed Responses turn"
    );
    session.close();
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    let _ = release_backend.send(());
    backend.join().unwrap();
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_request_timeout_is_delivered_and_next_request_uses_same_local_socket() {
    use crate::runtime_proxy::{
        RuntimeWebsocketTextMessageInput, proxy_runtime_websocket_text_message,
    };
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut shared = websocket_test_shared_with_main_profile(
        "same-socket-timeout",
        listener.local_addr().unwrap(),
    );
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .stream_idle_timeout_ms = 5_000;
    Arc::make_mut(&mut shared.runtime_config)
        .tuning
        .websocket_precommit_progress_timeout_ms = 5_000;
    let (mut local, mut client) = websocket_test_local_pair();
    crate::runtime_set_upstream_websocket_io_timeout(&mut client, Some(Duration::from_secs(10)))
        .unwrap();
    let (release_backend, keep_backend) = mpsc::channel();
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut first = tungstenite::accept(stream).unwrap();
        first.read().unwrap();
        // Hidden prelude traffic must not grant an unbounded new idle budget.
        let until = Instant::now() + Duration::from_secs(4);
        while Instant::now() < until {
            if first
                .send(WsMessage::Text(r#"{"type":"response.in_progress"}"#.into()))
                .is_err()
            {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        listener.set_nonblocking(true).unwrap();
        let next_deadline = Instant::now() + Duration::from_secs(10);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < next_deadline =>
                {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("next request must reconnect upstream: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        // Winsock accepted sockets can inherit the listener's nonblocking
        // mode. This synchronous mock needs bounded blocking reads on every OS.
        stream.set_nonblocking(false).unwrap();
        let mut second = tungstenite::accept(stream).unwrap();
        second.read().unwrap();
        second
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"healthy-after-timeout"}}"#.into(),
            ))
            .unwrap();
        let _ = keep_backend.recv_timeout(Duration::from_secs(10));
    });
    let text = r#"{"type":"response.create","input":[]}"#;
    let metadata = parse_runtime_websocket_request_metadata(text);
    let request = handshake();
    let mut session = RuntimeWebsocketSessionState::default();
    let mut expired = shared.clone();
    expired.precommit_deadline = Some(Instant::now() + Duration::from_secs(1));
    for (id, request_shared) in [(707, &expired), (708, &shared)] {
        proxy_runtime_websocket_text_message(RuntimeWebsocketTextMessageInput {
            session_id: 70,
            request_id: id,
            local_socket: &mut local,
            handshake_request: &request,
            request_text: text,
            request_metadata: &metadata,
            shared: request_shared,
            websocket_session: &mut session,
        })
        .unwrap();
        let event: serde_json::Value =
            serde_json::from_str(&client.read().unwrap().to_string()).unwrap();
        if id == 707 {
            assert_eq!(
                event["type"], "error",
                "no hidden prelude should precede the terminal error"
            );
            assert_eq!(event["status"], 504);
            assert_eq!(event["error"]["code"], "request_timeout");
        } else {
            assert_eq!(event["type"], "response.completed");
            assert_eq!(event["response"]["id"], "healthy-after-timeout");
        }
    }
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    assert!(
        shared
            .runtime
            .lock()
            .unwrap()
            .profile_transport_backoff_until
            .is_empty()
    );
    let _ = release_backend.send(());
    backend.join().unwrap();
    session.close();
    let _ = std::fs::remove_file(&shared.log_path);
}

use super::*;
use crate::runtime_proxy::{
    RuntimeWebsocketTextMessageInput, proxy_runtime_websocket_text_message,
};
use std::sync::mpsc;

const INTERRUPT: &str =
    r#"{"type":"response.interrupt","response_id":"resp-original","mode":"discard_partial_items"}"#;

fn send_text(
    local: &mut RuntimeLocalWebSocket,
    session: &mut RuntimeWebsocketSessionState,
    shared: &RuntimeRotationProxyShared,
    text: &str,
) -> Result<()> {
    let handshake = RuntimeProxyRequest {
        method: "GET".into(),
        path_and_query: "/backend-api/prodex/responses".into(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let metadata = parse_runtime_websocket_request_metadata(text);
    proxy_runtime_websocket_text_message(RuntimeWebsocketTextMessageInput {
        session_id: 1,
        request_id: 2,
        local_socket: local,
        handshake_request: &handshake,
        request_text: text,
        request_metadata: &metadata,
        shared,
        websocket_session: session,
    })
}

#[test]
fn interrupt_without_its_original_socket_never_opens_a_new_inference_connection_01651() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let shared = websocket_test_shared_with_main_profile(
        "interrupt-no-owner",
        listener.local_addr().unwrap(),
    );
    let (mut local, _client) = websocket_test_local_pair();
    let (finish, finished) = mpsc::channel();
    let backend = thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(10);
        let mut first_events = Vec::new();
        while Instant::now() < until && finished.try_recv().is_err() {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut socket = tungstenite::accept(stream).unwrap();
                    first_events.push(socket.read().unwrap().into_text().unwrap().to_string());
                    socket.send(WsMessage::Text(r#"{"type":"error","status":400,"error":{"type":"invalid_request_error","message":"Expected a 'response.create' message as the first websocket event."}}"#.into())).unwrap();
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("fixture listener: {error}"),
            }
        }
        first_events
    });
    let result = send_text(
        &mut local,
        &mut RuntimeWebsocketSessionState::default(),
        &shared,
        INTERRUPT,
    );
    finish.send(()).unwrap();
    let first_events = backend.join().unwrap();
    assert!(
        first_events.is_empty(),
        "connection-bound control was sent as a new inference: {first_events:?}"
    );
    assert!(
        result.is_err(),
        "missing control owner must close locally, not fabricate an acknowledgement"
    );
    let state = shared.runtime.lock().unwrap();
    assert!(state.profile_transport_backoff_until.is_empty());
    assert!(state.profile_retry_backoff_until.is_empty());
}

fn controlled_local_pair() -> (
    tiny_http::Server,
    RuntimeLocalWebSocket,
    RuntimeUpstreamWebSocket,
    tiny_http::ConnectionControl,
) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let address = server.server_addr().to_ip().unwrap();
    let client = thread::spawn(move || {
        tungstenite::connect(format!("ws://{address}/responses"))
            .unwrap()
            .0
    });
    let request = server
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let key = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Sec-WebSocket-Key"))
        .unwrap()
        .value
        .as_str();
    let response = tiny_http::Response::empty(101).with_header(
        tiny_http::Header::from_bytes(
            "Sec-WebSocket-Accept",
            tungstenite::handshake::derive_accept_key(key.as_bytes()),
        )
        .unwrap(),
    );
    let control = request.connection_control().unwrap();
    let local = tungstenite::WebSocket::from_raw_socket(
        request.upgrade("websocket", response),
        tungstenite::protocol::Role::Server,
        None,
    );
    let mut client = client.join().unwrap();
    crate::runtime_set_upstream_websocket_io_timeout(&mut client, Some(Duration::from_secs(8)))
        .unwrap();
    (server, local, client, control)
}

fn create_response(
    local: &mut RuntimeLocalWebSocket,
    session: &mut RuntimeWebsocketSessionState,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeWebsocketAttempt> {
    let request = RuntimeProxyRequest {
        method: "GET".into(),
        path_and_query: "/backend-api/prodex/responses".into(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    attempt_runtime_websocket_request_with_hard_affinity(
        RuntimeWebsocketAttemptRequest {
            request_id: 31,
            local_socket: local,
            handshake_request: &request,
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

fn next_text(client: &mut RuntimeUpstreamWebSocket) -> String {
    loop {
        match client.read().unwrap() {
            WsMessage::Text(text) => return text.to_string(),
            WsMessage::Ping(_) | WsMessage::Pong(_) => {}
            other => panic!("unexpected fixture frame: {other:?}"),
        }
    }
}

fn active_interrupt_case(ack_first: bool) {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut shared = websocket_test_shared_with_main_profile(
        if ack_first {
            "interrupt-ack-first"
        } else {
            "interrupt-completed-first"
        },
        listener.local_addr().unwrap(),
    );
    // This is an interactive control-ordering test, not a 250ms idle-timeout
    // test. Match both fixture progress budgets to its bounded peer handshake;
    // the deliberately delayed steering below still fails the old 250ms setup.
    let tuning = &mut Arc::make_mut(&mut shared.runtime_config).tuning;
    tuning.websocket_precommit_progress_timeout_ms = 5_000;
    tuning.stream_idle_timeout_ms = 5_000;
    let (_server, mut local, mut client, control) = controlled_local_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    session.local_control = Some(control);
    let (release, released) = mpsc::channel();
    let backend = thread::spawn(move || -> Result<Vec<String>> {
        let (stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let mut socket = tungstenite::accept(stream)?;
        let first = socket.read()?.into_text()?.to_string();
        socket.send(WsMessage::Text(
            r#"{"type":"response.created","response":{"id":"resp-original"}}"#.into(),
        ))?;
        socket.send(WsMessage::Text(r#"{"type":"response.output_text.delta","response_id":"resp-original","delta":"exactly-once"}"#.into()))?;
        // Completion is impossible until the proxy forwards the live interrupt.
        let interrupt = socket.read()?.into_text()?.to_string();
        let ack = r#"{"type":"response.interrupt.accepted","response_id":"resp-original"}"#;
        let completed = r#"{"type":"response.completed","response":{"id":"resp-original","usage":{"input_tokens":2,"output_tokens":1}}}"#;
        for text in if ack_first {
            [ack, completed]
        } else {
            [completed, ack]
        } {
            socket.send(WsMessage::Text(text.into()))?;
        }
        let _ = released.recv_timeout(Duration::from_secs(8));
        Ok(vec![first, interrupt])
    });
    thread::scope(|scope| {
        let worker = scope.spawn(|| create_response(&mut local, &mut session, &shared));
        let mut messages = Vec::new();
        loop {
            let text = next_text(&mut client);
            let is_delta = text.contains("response.output_text.delta");
            messages.push(text);
            if is_delta {
                break;
            }
        }
        if ack_first {
            // Exercise real user steering latency, not an immediate same-tick reply.
            thread::sleep(Duration::from_millis(350));
        }
        client.send(WsMessage::Text(INTERRUPT.into())).unwrap();
        while !messages
            .iter()
            .any(|text| text.contains("response.completed"))
            || !messages
                .iter()
                .any(|text| text.contains("response.interrupt.accepted"))
        {
            messages.push(next_text(&mut client));
        }
        assert!(matches!(
            worker.join().unwrap().unwrap(),
            RuntimeWebsocketAttempt::Delivered
        ));
        for event in [
            "response.completed",
            "response.interrupt.accepted",
            "response.output_text.delta",
        ] {
            assert_eq!(
                messages.iter().filter(|text| text.contains(event)).count(),
                1,
                "{messages:?}"
            );
        }
    });
    assert!(session.can_reuse("main", None));
    assert_eq!(session.transport_generation(), 1);
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    release.send(()).unwrap();
    let sent = backend.join().unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&sent[0]).unwrap()["type"],
        "response.create"
    );
    assert_eq!(
        sent[1], INTERRUPT,
        "control payload must remain byte-for-byte unchanged"
    );
}

#[test]
fn live_interrupt_preserves_create_before_control_and_ack_before_completion_01651() {
    active_interrupt_case(true);
}

#[test]
fn live_interrupt_waits_for_real_ack_after_completion_without_duplicate_output_01651() {
    active_interrupt_case(false);
}

#[test]
fn late_interrupt_uses_only_its_cached_transport_without_reacquiring_quota_01651() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let shared = websocket_test_shared_with_main_profile(
        "late-interrupt-owned",
        listener.local_addr().unwrap(),
    );
    let (mut local, mut client) = websocket_test_local_pair();
    crate::runtime_set_upstream_websocket_io_timeout(&mut client, Some(Duration::from_secs(5)))
        .unwrap();
    let (release, released) = mpsc::channel();
    let backend = thread::spawn(move || -> Result<String> {
        let (stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut socket = tungstenite::accept(stream)?;
        socket.read()?;
        socket.send(WsMessage::Text(
            r#"{"type":"response.completed","response":{"id":"resp-original"}}"#.into(),
        ))?;
        let text = socket.read()?.into_text()?.to_string();
        socket.send(WsMessage::Text(
            r#"{"type":"response.interrupt.accepted","response_id":"resp-original"}"#.into(),
        ))?;
        let _ = released.recv_timeout(Duration::from_secs(5));
        Ok(text)
    });
    let mut session = RuntimeWebsocketSessionState::default();
    assert!(matches!(
        create_response(&mut local, &mut session, &shared).unwrap(),
        RuntimeWebsocketAttempt::Delivered
    ));
    // Execution capacity is occupied by unrelated work; control needs none.
    shared.lane_admission.set_profile_inflight("main", 100);
    send_text(&mut local, &mut session, &shared, INTERRUPT).unwrap();
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 100);
    assert!(next_text(&mut client).contains("response.completed"));
    assert!(next_text(&mut client).contains("response.interrupt.accepted"));
    assert_eq!(session.transport_generation(), 1);
    assert!(session.can_reuse("main", None));
    shared.lane_admission.set_profile_inflight("main", 0);
    release.send(()).unwrap();
    assert_eq!(backend.join().unwrap().unwrap(), INTERRUPT);
}

#[test]
fn interrupt_response_id_cannot_be_rebound_to_another_transport_01651() {
    let _guard = acquire_test_runtime_lock();
    let shared = websocket_test_shared("interrupt-generation");
    let (mut local, _client) = websocket_test_local_pair();
    let mut session = RuntimeWebsocketSessionState::default();
    let old = session.advance_transport_generation();
    session.remember_response_transport_generation(["resp-original".into()], old);
    session.advance_transport_generation();
    assert!(
        send_text(&mut local, &mut session, &shared, INTERRUPT)
            .unwrap_err()
            .to_string()
            .contains("original response transport")
    );
    assert!(
        send_text(
            &mut local,
            &mut session,
            &shared,
            r#"{"type":"response.interrupt","response_id":null}"#
        )
        .is_err()
    );
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
}

#[test]
fn late_interrupt_owner_failure_cannot_rotate_or_replay_01651() {
    let _guard = acquire_test_runtime_lock();
    for (name, acknowledgement) in [
        (
            "wrong-ack",
            Some(r#"{"type":"response.interrupt.accepted","response_id":"another-response"}"#),
        ),
        (
            "rejected",
            Some(r#"{"type":"response.interrupt.failed","response_id":"resp-original"}"#),
        ),
        ("closed", None),
    ] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let shared = websocket_test_shared_with_main_profile(name, listener.local_addr().unwrap());
        let (mut local, _client) = websocket_test_local_pair();
        let backend = thread::spawn(move || -> Result<Vec<String>> {
            let (stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            stream.set_write_timeout(Some(Duration::from_secs(5)))?;
            let mut socket = tungstenite::accept(stream)?;
            let create = socket.read()?.into_text()?.to_string();
            socket.send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-original"}}"#.into(),
            ))?;
            let control = socket.read()?.into_text()?.to_string();
            if let Some(text) = acknowledgement {
                socket.send(WsMessage::Text(text.into()))?;
            }
            // Close this owner: no replacement transport may inherit control.
            Ok(vec![create, control])
        });
        let mut session = RuntimeWebsocketSessionState::default();
        assert!(matches!(
            create_response(&mut local, &mut session, &shared).unwrap(),
            RuntimeWebsocketAttempt::Delivered
        ));
        assert!(
            send_text(&mut local, &mut session, &shared, INTERRUPT).is_err(),
            "{name}"
        );
        assert!(
            !session.has_socket(),
            "failed controls invalidate only their socket"
        );
        assert_eq!(
            session.transport_generation(),
            1,
            "no replacement transport"
        );
        assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
        let state = shared.runtime.lock().unwrap();
        assert!(state.profile_transport_backoff_until.is_empty(), "{name}");
        assert!(state.profile_retry_backoff_until.is_empty(), "{name}");
        drop(state);
        let sent = backend.join().unwrap().unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[1], INTERRUPT);
    }
}

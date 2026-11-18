//! Native Codex must see size rejection before it starts a response stream.
use super::*;
use crate::runtime_proxy::websocket_message::{
    RuntimeWebsocketTextMessageInput, proxy_runtime_websocket_text_message,
};
use tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};

struct MessageLimitHeaders(Vec<String>);

impl tungstenite::handshake::server::Callback for MessageLimitHeaders {
    fn on_request(
        self,
        _: &tungstenite::handshake::server::Request,
        mut response: tungstenite::handshake::server::Response,
    ) -> std::result::Result<
        tungstenite::handshake::server::Response,
        tungstenite::handshake::server::ErrorResponse,
    > {
        for header in &self.0 {
            response.headers_mut().append(
                "x-codex-websocket-max-message-bytes",
                tungstenite::http::HeaderValue::from_str(header).unwrap(),
            );
        }
        Ok(response)
    }
}

fn run_size_request(
    local: &mut RuntimeLocalWebSocket,
    shared: &RuntimeRotationProxyShared,
    session: &mut RuntimeWebsocketSessionState,
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
        session_id: 170,
        request_id: 170,
        local_socket: local,
        handshake_request: &handshake,
        request_text: text,
        request_metadata: &metadata,
        shared,
        websocket_session: session,
    })
}

fn next_payload(client: &mut RuntimeUpstreamWebSocket) -> WsMessage {
    loop {
        match client.read().expect("bounded downstream response") {
            WsMessage::Ping(_) | WsMessage::Pong(_) => {}
            message => return message,
        }
    }
}

#[test]
fn websocket_size_early_close_reaches_client_without_transport_retry() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut shared =
        websocket_test_shared_with_main_profile("size-early-close", listener.local_addr().unwrap());
    shared.precommit_deadline = Some(Instant::now() + Duration::from_secs(3));
    let (release, wait) = std::sync::mpsc::channel();
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        drop(listener);
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        assert!(matches!(socket.read().unwrap(), WsMessage::Text(_)));
        socket.send(WsMessage::Ping("size-probe".into())).unwrap();
        socket
            .close(Some(CloseFrame {
                code: CloseCode::Size,
                reason: "request exceeds message limit".into(),
            }))
            .unwrap();
        let _ = wait.recv_timeout(Duration::from_secs(5));
    });
    let (mut local, mut client) = websocket_test_local_pair();
    crate::runtime_set_upstream_websocket_io_timeout(&mut client, Some(Duration::from_secs(5)))
        .unwrap();
    let mut session = RuntimeWebsocketSessionState::default();
    let result = run_size_request(
        &mut local,
        &shared,
        &mut session,
        r#"{"type":"response.create","input":[]}"#,
    );
    let observed = next_payload(&mut client);
    release.send(()).unwrap();
    backend.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        observed,
        WsMessage::Close(Some(CloseFrame {
            code: CloseCode::Size,
            reason: "request exceeds message limit".into(),
        }))
    );
    assert!(!session.has_socket());
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    assert!(
        shared
            .runtime
            .lock()
            .unwrap()
            .profile_transport_backoff_until
            .is_empty()
    );
    let _ = std::fs::remove_file(&shared.log_path);
}

fn advertised_limit_case(name: &str, headers: &[&str], text: &str, fallback: bool) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let shared = websocket_test_shared_with_main_profile(name, listener.local_addr().unwrap());
    let (release, wait) = std::sync::mpsc::channel();
    let headers: Vec<String> = headers.iter().map(|value| (*value).to_string()).collect();
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut socket = tungstenite::accept_hdr(stream, MessageLimitHeaders(headers)).unwrap();
        let first = socket.read().unwrap();
        if matches!(first, WsMessage::Text(_)) {
            socket
                .send(WsMessage::Text(
                    r#"{"type":"response.completed","response":{"id":"resp-size"}}"#.into(),
                ))
                .unwrap();
        }
        let _ = wait.recv_timeout(Duration::from_secs(5));
        first
    });
    let (mut local, mut client) = websocket_test_local_pair();
    crate::runtime_set_upstream_websocket_io_timeout(&mut client, Some(Duration::from_secs(5)))
        .unwrap();
    let mut session = RuntimeWebsocketSessionState::default();
    let result = run_size_request(&mut local, &shared, &mut session, text);
    let observed = next_payload(&mut client);
    release.send(()).unwrap();
    let upstream_frame = backend.join().unwrap();
    assert!(result.is_ok(), "{name}: {result:?}");
    if fallback {
        assert!(
            matches!(upstream_frame, WsMessage::Close(_)),
            "{name}: advertised limit must prevent every upstream generation request, got {upstream_frame:?}"
        );
        assert!(matches!(observed, WsMessage::Close(Some(frame)) if frame.code == CloseCode::Size));
        assert!(!session.has_socket());
    } else {
        assert_eq!(upstream_frame, WsMessage::Text(text.into()), "{name}");
        assert!(matches!(observed, WsMessage::Text(value) if value.contains("response.completed")));
        assert!(session.has_socket());
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
    session.close();
    let _ = std::fs::remove_file(&shared.log_path);
}

#[test]
fn websocket_size_advertised_limit_stops_generation_before_native_http_fallback() {
    let _guard = acquire_test_runtime_lock();
    advertised_limit_case(
        "size-advertised-limit",
        &["1"],
        r#"{"type":"response.create","input":[],"generate":false}"#,
        true,
    );
}

#[test]
fn websocket_size_advertised_limit_uses_native_parse_and_final_utf8_bytes() {
    let _guard = acquire_test_runtime_lock();
    let text = r#"{"type":"response.create","input":[{"role":"user","content":"雪🙂"}]}"#;
    let bytes = text.len().to_string();
    let chars = text.chars().count().to_string();
    for (name, headers, fallback) in [
        ("size-missing", vec![], false),
        ("size-invalid", vec!["not-a-number"], false),
        ("size-negative", vec!["-1"], false),
        ("size-overflow", vec!["184467440737095516160"], false),
        ("size-zero", vec!["0"], true),
        ("size-equal-bytes", vec![bytes.as_str()], false),
        ("size-multibyte", vec![chars.as_str()], true),
        ("size-first-small", vec!["1", "100000"], true),
        ("size-first-large", vec!["100000", "1"], false),
    ] {
        advertised_limit_case(name, &headers, text, fallback);
    }
}

#[test]
fn websocket_size_close_after_any_text_preserves_stream_recovery() {
    let _guard = acquire_test_runtime_lock();
    for (name, text, committed) in [
        ("size-after-empty", "", false),
        (
            "size-after-ack",
            r#"{"type":"response.interrupt.accepted","response_id":"resp-pending"}"#,
            true,
        ),
        (
            "size-after-metadata",
            r#"{"type":"codex.response.metadata","headers":{"openai-model":"test-model"}}"#,
            false,
        ),
        (
            "size-after-rates",
            r#"{"type":"codex.rate_limits","rate_limits":{"primary":{"used_percent":1.0}}}"#,
            false,
        ),
        (
            "size-after-created",
            r#"{"type":"response.created","response":{"id":"resp-pending"}}"#,
            false,
        ),
        (
            "size-after-output",
            r#"{"type":"response.output_text.delta","delta":"begun"}"#,
            true,
        ),
    ] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let shared = websocket_test_shared_with_main_profile(name, listener.local_addr().unwrap());
        let (release, wait) = std::sync::mpsc::channel();
        let backend = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            assert!(matches!(socket.read().unwrap(), WsMessage::Text(_)));
            socket.send(WsMessage::Text(text.into())).unwrap();
            socket
                .close(Some(CloseFrame {
                    code: CloseCode::Size,
                    reason: "too large".into(),
                }))
                .unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(5));
        });
        let (mut local, _client) = websocket_test_local_pair();
        let mut session = RuntimeWebsocketSessionState::default();
        let handshake = RuntimeProxyRequest {
            method: "GET".into(),
            path_and_query: "/backend-api/prodex/responses".into(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let result = attempt_runtime_websocket_request_with_hard_affinity(
            RuntimeWebsocketAttemptRequest {
                request_id: 171,
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
        if committed {
            assert!(
                result.is_err(),
                "committed output may never produce a replay attempt: {result:?}"
            );
        } else {
            assert!(
                matches!(
                    result,
                    Err(_) | Ok(RuntimeWebsocketAttempt::TransportFailed { .. })
                ),
                "{name}: {result:?}"
            );
        }
        assert!(!session.has_socket());
        assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
        let _ = std::fs::remove_file(&shared.log_path);
    }
}

#[test]
fn websocket_size_reused_connection_retains_limit_and_response_affinity() {
    let _guard = acquire_test_runtime_lock();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let shared = websocket_test_shared_with_main_profile(
        "size-reused-affinity",
        listener.local_addr().unwrap(),
    );
    let (release, wait) = std::sync::mpsc::channel();
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut socket =
            tungstenite::accept_hdr(stream, MessageLimitHeaders(vec!["128".into()])).unwrap();
        assert!(matches!(socket.read().unwrap(), WsMessage::Text(_)));
        socket
            .send(WsMessage::Text(
                r#"{"type":"response.completed","response":{"id":"resp-size-owner"}}"#.into(),
            ))
            .unwrap();
        let second = socket.read().unwrap();
        if matches!(second, WsMessage::Text(_)) {
            socket
                .send(WsMessage::Text(
                    r#"{"type":"response.completed","response":{"id":"resp-size-next"}}"#.into(),
                ))
                .unwrap();
        }
        let _ = wait.recv_timeout(Duration::from_secs(5));
        second
    });
    let (mut local, mut client) = websocket_test_local_pair();
    crate::runtime_set_upstream_websocket_io_timeout(&mut client, Some(Duration::from_secs(5)))
        .unwrap();
    let mut session = RuntimeWebsocketSessionState::default();
    run_size_request(
        &mut local,
        &shared,
        &mut session,
        r#"{"type":"response.create","input":[]}"#,
    )
    .unwrap();
    assert!(matches!(next_payload(&mut client), WsMessage::Text(_)));
    assert!(session.can_reuse("main", None));
    let followup = serde_json::json!({
        "type": "response.create",
        "previous_response_id": "resp-size-owner",
        "input": [{"role":"user", "content": "雪🙂".repeat(64)}],
    })
    .to_string();
    let result = run_size_request(&mut local, &shared, &mut session, &followup);
    let observed = next_payload(&mut client);
    release.send(()).unwrap();
    let upstream_frame = backend.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    assert!(
        matches!(upstream_frame, WsMessage::Close(_)),
        "{upstream_frame:?}"
    );
    assert!(matches!(observed, WsMessage::Close(Some(frame)) if frame.code == CloseCode::Size));
    assert!(!session.has_socket());
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
    let runtime = shared.runtime.lock().unwrap();
    assert_eq!(
        runtime
            .state
            .response_profile_bindings
            .get("resp-size-owner")
            .map(|binding| binding.profile_name.as_str()),
        Some("main"),
    );
    assert!(runtime.profile_transport_backoff_until.is_empty());
    drop(runtime);
    let _ = std::fs::remove_file(&shared.log_path);
}

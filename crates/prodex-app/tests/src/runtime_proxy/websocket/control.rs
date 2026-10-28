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

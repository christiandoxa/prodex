use super::*;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

#[test]
fn websocket_readiness_poll_preserves_partial_frame_and_next_buffered_frame() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (advance, steps) = mpsc::channel();
    let (sent, observed) = mpsc::channel();
    let payload = br#"{"type":"response.completed","response":{"id":"resp-ready"}}"#;
    let next = br#"{"type":"response.failed","error":{"code":"previous_response_not_found"}}"#;
    let backend = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_nodelay(true).unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        steps.recv_timeout(Duration::from_secs(5)).unwrap();
        // An unmasked server text frame is deliberately split inside its
        // payload. The client's first nonblocking read must retain this prefix.
        socket
            .get_mut()
            .write_all(&[0x81, u8::try_from(payload.len()).unwrap()])
            .unwrap();
        socket.get_mut().write_all(&payload[..7]).unwrap();
        sent.send(()).unwrap();
        steps.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut tail = payload[7..].to_vec();
        tail.extend([0x81, u8::try_from(next.len()).unwrap()]);
        tail.extend(next);
        socket.get_mut().write_all(&tail).unwrap();
        sent.send(()).unwrap();
        // Keep the mock alive until both terminal messages are consumed.
        steps.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let (mut client, _) = tungstenite::connect(format!("ws://{address}/")).unwrap();
    let original_timeout = Some(Duration::from_secs(3));
    transport(&client)
        .unwrap()
        .set_read_timeout(original_timeout)
        .unwrap();
    let start = Instant::now();
    assert!(
        read_response_frame(&mut client, Duration::from_millis(10))
            .unwrap()
            .is_none()
    );
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "poll must not wait for SO_RCVTIMEO"
    );
    advance.send(()).unwrap();
    observed.recv_timeout(Duration::from_secs(5)).unwrap();
    for _ in 0..2 {
        assert!(
            read_response_frame(&mut client, Duration::from_millis(10))
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(
        transport(&client).unwrap().read_timeout().unwrap(),
        original_timeout
    );
    advance.send(()).unwrap();
    observed.recv_timeout(Duration::from_secs(5)).unwrap();
    let mut frames = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while frames.len() < 2 && Instant::now() < deadline {
        if let Some(frame) = read_response_frame(&mut client, Duration::from_millis(10)).unwrap() {
            match frame {
                WsMessage::Text(text) => frames.push(text.to_string()),
                other => panic!("unexpected frame after split-read recovery: {other:?}"),
            }
        }
    }
    assert_eq!(
        frames,
        [
            std::str::from_utf8(payload).unwrap(),
            std::str::from_utf8(next).unwrap()
        ]
    );
    assert_eq!(
        transport(&client).unwrap().read_timeout().unwrap(),
        original_timeout
    );
    advance.send(()).unwrap();
    backend.join().unwrap();
}

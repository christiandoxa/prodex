use super::{GatewayServerMode, full_response, spawn_direct_frontend, stop_frontend};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
    time::timeout,
};

#[tokio::test]
async fn incomplete_headers_release_the_connection_slot_after_timeout() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = Arc::clone(&calls);
    let (front_addr, stop, front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |config| {
            config.max_connections = 1;
            config.request_header_timeout = Duration::from_millis(75);
        },
        move |_| {
            handler_calls.fetch_add(1, Ordering::SeqCst);
            async { full_response(b"ok") }
        },
    )
    .await;
    let mut stalled = TcpStream::connect(front_addr).await.unwrap();
    stalled
        .write_all(b"GET /responses HTTP/1.1\r\nHost:")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let mut valid = TcpStream::connect(front_addr).await.unwrap();
    valid
        .write_all(b"GET /responses HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = Vec::new();
    timeout(Duration::from_secs(1), valid.read_to_end(&mut response))
        .await
        .expect("valid request should not remain blocked behind incomplete headers")
        .unwrap();
    assert!(response.starts_with(b"HTTP/1.1 200"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    stop_frontend(stop, front).await;
}

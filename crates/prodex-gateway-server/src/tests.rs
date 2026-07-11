use std::{
    convert::Infallible,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::{Request, Response, StatusCode, body::Body, body::Incoming, service::service_fn};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::{TokioExecutor, TokioIo},
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::{Notify, oneshot},
    task::JoinHandle,
    time::timeout,
};

use super::{GatewayServerConfig, GatewayServerMode, run};

type TestClient = Client<HttpConnector, Full<Bytes>>;

#[tokio::test]
async fn route_isolation_keeps_data_and_control_planes_separate() {
    let calls = Arc::new(AtomicUsize::new(0));
    let backend_calls = Arc::clone(&calls);
    let (backend_addr, backend) = spawn_backend(move |_| {
        let calls = Arc::clone(&backend_calls);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Response::new(Full::new(Bytes::from_static(b"ok")))
        }
    })
    .await;
    let client = test_client();

    let (data_addr, data_stop, data) =
        spawn_frontend(GatewayServerMode::DataPlane, backend_addr, |_| {}).await;
    let response = request(&client, data_addr, "/admin/keys", Bytes::new()).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let response = request(&client, data_addr, "/responses", Bytes::new()).await;
    assert_eq!(response.status(), StatusCode::OK);
    stop_frontend(data_stop, data).await;

    let (control_addr, control_stop, control) =
        spawn_frontend(GatewayServerMode::ControlPlane, backend_addr, |_| {}).await;
    let response = request(&client, control_addr, "/responses", Bytes::new()).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = request(&client, control_addr, "/readyz", Bytes::new()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    stop_frontend(control_stop, control).await;
    backend.abort();
}

#[tokio::test]
async fn response_body_streams_without_waiting_for_completion() {
    let backend = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let backend_addr = backend.local_addr().unwrap();
    let (first_sent_tx, first_sent_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let backend = tokio::spawn(async move {
        let (mut stream, _) = backend.accept().await.unwrap();
        let mut request = vec![0; 4096];
        let _ = stream.read(&mut request).await.unwrap();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: text/plain\r\n\r\n5\r\nfirst\r\n",
            )
            .await
            .unwrap();
        stream.flush().await.unwrap();
        first_sent_tx.send(()).unwrap();
        release_rx.await.unwrap();
        stream.write_all(b"6\r\nsecond\r\n0\r\n\r\n").await.unwrap();
    });
    let (front_addr, stop, front) =
        spawn_frontend(GatewayServerMode::DataPlane, backend_addr, |_| {}).await;
    let response = request(&test_client(), front_addr, "/responses", Bytes::new()).await;
    first_sent_rx.await.unwrap();
    let mut body = response.into_body();
    let first = timeout(Duration::from_secs(1), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    assert_eq!(first, "first");
    release_tx.send(()).unwrap();
    assert_eq!(body.collect().await.unwrap().to_bytes(), "second");
    stop_frontend(stop, front).await;
    backend.await.unwrap();
}

#[tokio::test]
async fn content_length_over_limit_is_rejected_before_backend() {
    let calls = Arc::new(AtomicUsize::new(0));
    let backend_calls = Arc::clone(&calls);
    let (backend_addr, backend) = spawn_backend(move |_| {
        let calls = Arc::clone(&backend_calls);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Response::new(Full::new(Bytes::new()))
        }
    })
    .await;
    let (front_addr, stop, front) =
        spawn_frontend(GatewayServerMode::DataPlane, backend_addr, |config| {
            config.max_request_body_bytes = 4
        })
        .await;
    let response = request(
        &test_client(),
        front_addr,
        "/responses",
        Bytes::from("12345"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        body,
        r#"{"error":{"code":"request_body_too_large","message":"request body exceeds the configured limit"}}"#
    );
    stop_frontend(stop, front).await;
    backend.abort();
}

#[tokio::test]
async fn shutdown_drains_an_inflight_response() {
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let backend_started = Arc::clone(&started);
    let backend_release = Arc::clone(&release);
    let (backend_addr, backend) = spawn_backend(move |_| {
        let started = Arc::clone(&backend_started);
        let release = Arc::clone(&backend_release);
        async move {
            started.notify_one();
            release.notified().await;
            Response::new(Full::new(Bytes::from_static(b"done")))
        }
    })
    .await;
    let (front_addr, stop, mut front) =
        spawn_frontend(GatewayServerMode::DataPlane, backend_addr, |config| {
            config.drain_timeout = Duration::from_secs(1)
        })
        .await;
    let pending = tokio::spawn(async move {
        request(&test_client(), front_addr, "/responses", Bytes::new()).await
    });
    started.notified().await;
    stop.send(()).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!front.is_finished());
    release.notify_one();
    assert_eq!(pending.await.unwrap().status(), StatusCode::OK);
    timeout(Duration::from_secs(1), &mut front)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    backend.abort();
}

#[tokio::test]
async fn websocket_upgrade_tunnels_bytes_until_connection_closes() {
    let backend = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let backend_addr = backend.local_addr().unwrap();
    let backend = tokio::spawn(async move {
        let (mut stream, _) = backend.accept().await.unwrap();
        let headers = read_headers(&mut stream).await;
        assert!(headers.starts_with("GET /realtime HTTP/1.1"));
        stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
            )
            .await
            .unwrap();
        let mut payload = [0_u8; 4];
        stream.read_exact(&mut payload).await.unwrap();
        assert_eq!(&payload, b"ping");
        stream.write_all(b"pong").await.unwrap();
    });
    let (front_addr, stop, front) =
        spawn_frontend(GatewayServerMode::DataPlane, backend_addr, |_| {}).await;
    let mut client = TcpStream::connect(front_addr).await.unwrap();
    client
        .write_all(
            b"GET /realtime HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
        )
        .await
        .unwrap();
    let headers = read_headers(&mut client).await;
    assert!(headers.starts_with("HTTP/1.1 101"));
    client.write_all(b"ping").await.unwrap();
    let mut payload = [0_u8; 4];
    client.read_exact(&mut payload).await.unwrap();
    assert_eq!(&payload, b"pong");
    drop(client);
    backend.await.unwrap();
    stop_frontend(stop, front).await;
}

#[tokio::test]
async fn chunked_body_over_limit_is_rejected_while_streaming() {
    let (backend_addr, backend) = spawn_backend(|request| async move {
        let _ = request.into_body().collect().await;
        Response::new(Full::new(Bytes::new()))
    })
    .await;
    let (front_addr, stop, front) =
        spawn_frontend(GatewayServerMode::DataPlane, backend_addr, |config| {
            config.max_request_body_bytes = 4
        })
        .await;
    let mut client = TcpStream::connect(front_addr).await.unwrap();
    client
        .write_all(
            b"POST /responses HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n12345\r\n0\r\n\r\n",
        )
        .await
        .unwrap();
    let headers = read_headers(&mut client).await;
    assert!(headers.starts_with("HTTP/1.1 413"), "{headers}");
    stop_frontend(stop, front).await;
    backend.abort();
}

async fn read_headers(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut byte = [0_u8; 1];
    while !bytes.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).await.unwrap();
        bytes.push(byte[0]);
    }
    String::from_utf8(bytes).unwrap()
}

async fn spawn_frontend(
    mode: GatewayServerMode,
    backend_addr: std::net::SocketAddr,
    configure: impl FnOnce(&mut GatewayServerConfig),
) -> (
    std::net::SocketAddr,
    oneshot::Sender<()>,
    JoinHandle<anyhow::Result<()>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mut config = GatewayServerConfig::production(addr, mode);
    config.response_header_timeout = Duration::from_secs(1);
    config.drain_timeout = Duration::from_secs(1);
    configure(&mut config);
    let (stop_tx, stop_rx) = oneshot::channel();
    let task = tokio::spawn(run(listener, config, backend_addr, async move {
        stop_rx.await.map_err(anyhow::Error::from)
    }));
    (addr, stop_tx, task)
}

async fn stop_frontend(stop: oneshot::Sender<()>, task: JoinHandle<anyhow::Result<()>>) {
    stop.send(()).unwrap();
    timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

fn test_client() -> TestClient {
    Client::builder(TokioExecutor::new()).build_http()
}

async fn request(
    client: &TestClient,
    addr: std::net::SocketAddr,
    path: &str,
    body: Bytes,
) -> Response<Incoming> {
    let length = body.len();
    client
        .request(
            Request::builder()
                .uri(format!("http://{addr}{path}"))
                .header(hyper::header::CONTENT_LENGTH, length)
                .body(Full::new(body))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn spawn_backend<F, Fut, B>(handler: F) -> (std::net::SocketAddr, JoinHandle<()>)
where
    F: Fn(Request<Incoming>) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Response<B>> + Send + 'static,
    B: Body<Data = Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let handler = handler.clone();
            tokio::spawn(async move {
                let service = service_fn(move |request| {
                    let handler = handler.clone();
                    async move { Ok::<_, Infallible>(handler(request).await) }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    });
    (addr, task)
}

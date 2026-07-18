use std::{
    convert::Infallible,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::{
    Request, Response, StatusCode,
    body::{Body, Incoming},
    service::service_fn,
};
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

use super::{
    GatewayHandlerRequest, GatewayHandlerResponse, GatewayServerBrowserSecurity,
    GatewayServerConfig, GatewayServerMode, GatewayServerReloadHandle, LoopbackBackend,
    run_with_handler,
};
use prodex_gateway_http::GatewayHttpRouteKind;

mod direct_support;
mod slowloris;
use direct_support::{DropSignal, GatedBody, full_response, spawn_direct_frontend};

type TestClient = Client<HttpConnector, Full<Bytes>>;
type TestFrontend = (
    std::net::SocketAddr,
    oneshot::Sender<()>,
    JoinHandle<anyhow::Result<()>>,
);

#[test]
fn reload_handle_swaps_edge_security_for_new_connections() {
    let mut config = GatewayServerConfig::production(
        "127.0.0.1:4000".parse().unwrap(),
        GatewayServerMode::DataPlane,
    );
    let reload = GatewayServerReloadHandle::new(&config).unwrap();
    config.edge_security.expected_host = "gateway.example.com".to_string();
    config.edge_security.trusted_proxies = vec!["192.0.2.10".parse().unwrap()];
    reload.reload(&config).unwrap();

    let security = reload.load();
    assert_eq!(security.edge_security.expected_host, "gateway.example.com");
    assert_eq!(
        security.edge_security.trusted_proxies,
        vec!["192.0.2.10".parse::<std::net::IpAddr>().unwrap()]
    );
}
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
async fn unknown_routes_are_rejected_before_backend_in_both_planes() {
    let calls = Arc::new(AtomicUsize::new(0));
    let backend_calls = Arc::clone(&calls);
    let (backend_addr, backend) = spawn_backend(move |_| {
        let calls = Arc::clone(&backend_calls);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Response::new(Full::new(Bytes::from_static(b"unexpected")))
        }
    })
    .await;
    let client = test_client();

    let (data_addr, data_stop, data) =
        spawn_frontend(GatewayServerMode::DataPlane, backend_addr, |_| {}).await;
    let response = request(&client, data_addr, "/v1/not-supported", Bytes::new()).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    stop_frontend(data_stop, data).await;

    let (control_addr, control_stop, control) =
        spawn_frontend(GatewayServerMode::ControlPlane, backend_addr, |_| {}).await;
    let response = request(
        &client,
        control_addr,
        "/v1/prodex/gateway/not-supported",
        Bytes::new(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    stop_frontend(control_stop, control).await;
    backend.abort();
}

#[tokio::test]
async fn canonical_target_is_forwarded_exactly_and_ambiguous_targets_never_reach_backend() {
    let targets = Arc::new(Mutex::new(Vec::new()));
    let backend_targets = Arc::clone(&targets);
    let (backend_addr, backend) = spawn_backend(move |request| {
        let targets = Arc::clone(&backend_targets);
        async move {
            targets.lock().unwrap().push(request.uri().to_string());
            Response::new(Full::new(Bytes::from_static(b"ok")))
        }
    })
    .await;
    let (front_addr, stop, front) =
        spawn_frontend(GatewayServerMode::DataPlane, backend_addr, |_| {}).await;

    let response = request(
        &test_client(),
        front_addr,
        "/v1/responses?cursor=a%2Fb&limit=1",
        Bytes::new(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        targets.lock().unwrap().as_slice(),
        ["/v1/responses?cursor=a%2Fb&limit=1"]
    );

    let invalid = request(&test_client(), front_addr, "/v1//responses", Bytes::new()).await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid.into_body().collect().await.unwrap().to_bytes(),
        r#"{"error":{"code":"invalid_request_target","message":"request target is invalid"}}"#
    );

    for target in ["/v1/%2e%2e/admin", "/v1/%2Fadmin", "/v1/%zz"] {
        let mut client = TcpStream::connect(front_addr).await.unwrap();
        client
            .write_all(format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let headers = read_headers(&mut client).await;
        assert!(headers.starts_with("HTTP/1.1 400"), "{target}: {headers}");
    }

    let mut client = TcpStream::connect(front_addr).await.unwrap();
    client
        .write_all(b"GET http://example.com/v1/responses HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    let headers = read_headers(&mut client).await;
    assert!(headers.starts_with("HTTP/1.1 400"), "{headers}");
    assert_eq!(targets.lock().unwrap().len(), 1);

    stop_frontend(stop, front).await;
    backend.abort();
}

#[tokio::test]
async fn direct_handler_receives_the_parsed_target_and_typed_route_without_rewrite() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let handler_seen = Arc::clone(&seen);
    let (front_addr, stop, front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |_| {},
        move |request: GatewayHandlerRequest| {
            let seen = Arc::clone(&handler_seen);
            async move {
                seen.lock().unwrap().push((
                    request.target.path_and_query().to_string(),
                    request.route,
                    request.request.uri().to_string(),
                ));
                full_response(b"ok")
            }
        },
    )
    .await;
    let target = "/v1/responses?cursor=a%2Fb&limit=1";
    let response = request(&test_client(), front_addr, target, Bytes::new()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        [(
            target.to_string(),
            GatewayHttpRouteKind::DataPlaneResponses,
            target.to_string(),
        )]
    );
    stop_frontend(stop, front).await;
}

#[tokio::test]
async fn direct_handler_stream_preserves_order_and_producer_backpressure() {
    let (release_tx, release_rx) = oneshot::channel();
    let release_rx = Arc::new(Mutex::new(Some(release_rx)));
    let handler_release = Arc::clone(&release_rx);
    let (front_addr, stop, front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |_| {},
        move |_| {
            let release = handler_release.lock().unwrap().take().unwrap();
            async move {
                Ok(GatewayHandlerResponse::new(Response::new(GatedBody::new(
                    release, None,
                ))))
            }
        },
    )
    .await;
    let response = request(&test_client(), front_addr, "/responses", Bytes::new()).await;
    let mut body = response.into_body();
    let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(first, "first");
    assert!(
        timeout(Duration::from_millis(50), body.frame())
            .await
            .is_err()
    );
    release_tx.send(()).unwrap();
    let second = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(second, "second");
    assert!(body.frame().await.is_none());
    stop_frontend(stop, front).await;
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
async fn compatibility_front_matches_backend_response_contract() {
    let (backend_addr, backend) = spawn_backend(|_| async {
        Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("x-upstream-contract", "exact")
            .body(Full::new(Bytes::from_static(b"generic upstream 429")))
            .unwrap()
    })
    .await;
    let client = test_client();
    let direct = request(&client, backend_addr, "/responses", Bytes::new()).await;
    let direct_status = direct.status();
    let direct_header = direct.headers()["x-upstream-contract"].clone();
    let direct_body = direct.into_body().collect().await.unwrap().to_bytes();

    let (front_addr, stop, front) =
        spawn_frontend(GatewayServerMode::DataPlane, backend_addr, |_| {}).await;
    let proxied = request(&client, front_addr, "/responses", Bytes::new()).await;
    assert_eq!(proxied.status(), direct_status);
    assert_eq!(proxied.headers()["x-upstream-contract"], direct_header);
    assert_eq!(
        proxied.into_body().collect().await.unwrap().to_bytes(),
        direct_body
    );

    stop_frontend(stop, front).await;
    backend.abort();
}

#[tokio::test]
async fn client_disconnect_cancels_backend_stream() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let backend_addr = listener.local_addr().unwrap();
    let (closed_tx, closed_rx) = oneshot::channel();
    let backend = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_headers(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nfirst\r\n")
            .await
            .unwrap();
        stream.flush().await.unwrap();
        let mut byte = [0_u8; 1];
        assert_eq!(stream.read(&mut byte).await.unwrap(), 0);
        closed_tx.send(()).unwrap();
    });
    let (front_addr, stop, front) =
        spawn_frontend(GatewayServerMode::DataPlane, backend_addr, |_| {}).await;
    let mut client = TcpStream::connect(front_addr).await.unwrap();
    client
        .write_all(b"GET /responses HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    read_headers(&mut client).await;
    drop(client);

    timeout(Duration::from_secs(1), closed_rx)
        .await
        .unwrap()
        .unwrap();
    backend.await.unwrap();
    stop_frontend(stop, front).await;
}

#[tokio::test]
async fn direct_handler_is_dropped_when_client_disconnects_before_headers() {
    let started = Arc::new(Notify::new());
    let handler_started = Arc::clone(&started);
    let (dropped_tx, dropped_rx) = oneshot::channel();
    let dropped_tx = Arc::new(Mutex::new(Some(dropped_tx)));
    let handler_dropped = Arc::clone(&dropped_tx);
    let (front_addr, stop, front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |config| config.response_header_timeout = Duration::from_secs(5),
        move |_| {
            let started = Arc::clone(&handler_started);
            let dropped = handler_dropped.lock().unwrap().take().unwrap();
            async move {
                let _drop_signal = DropSignal::new(dropped);
                started.notify_one();
                std::future::pending::<()>().await;
                full_response(b"")
            }
        },
    )
    .await;
    let mut client = TcpStream::connect(front_addr).await.unwrap();
    client
        .write_all(b"GET /responses HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    started.notified().await;
    drop(client);
    timeout(Duration::from_secs(1), dropped_rx)
        .await
        .unwrap()
        .unwrap();
    stop_frontend(stop, front).await;
}

#[tokio::test]
async fn direct_handler_stream_is_dropped_when_client_disconnects_mid_body() {
    let (release_tx, release_rx) = oneshot::channel();
    let _release_tx = release_tx;
    let release_rx = Arc::new(Mutex::new(Some(release_rx)));
    let handler_release = Arc::clone(&release_rx);
    let (dropped_tx, dropped_rx) = oneshot::channel();
    let dropped_tx = Arc::new(Mutex::new(Some(dropped_tx)));
    let handler_dropped = Arc::clone(&dropped_tx);
    let (front_addr, stop, front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |_| {},
        move |_| {
            let release = handler_release.lock().unwrap().take().unwrap();
            let dropped = handler_dropped.lock().unwrap().take().unwrap();
            async move {
                Ok(GatewayHandlerResponse::new(Response::new(GatedBody::new(
                    release,
                    Some(dropped),
                ))))
            }
        },
    )
    .await;
    let mut client = TcpStream::connect(front_addr).await.unwrap();
    client
        .write_all(b"GET /responses HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    read_headers(&mut client).await;
    let mut chunk = [0_u8; 32];
    let size = timeout(Duration::from_secs(1), client.read(&mut chunk))
        .await
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&chunk[..size]).contains("first"));
    drop(client);
    timeout(Duration::from_secs(1), dropped_rx)
        .await
        .unwrap()
        .unwrap();
    stop_frontend(stop, front).await;
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
async fn direct_handler_respects_the_connection_limit() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = Arc::clone(&calls);
    let first_started = Arc::new(Notify::new());
    let handler_started = Arc::clone(&first_started);
    let release_first = Arc::new(Notify::new());
    let handler_release = Arc::clone(&release_first);
    let (front_addr, stop, front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |config| config.max_connections = 1,
        move |_| {
            let call = handler_calls.fetch_add(1, Ordering::SeqCst) + 1;
            let started = Arc::clone(&handler_started);
            let release = Arc::clone(&handler_release);
            async move {
                if call == 1 {
                    started.notify_one();
                    release.notified().await;
                }
                full_response(b"ok")
            }
        },
    )
    .await;
    let mut first = TcpStream::connect(front_addr).await.unwrap();
    first
        .write_all(b"GET /responses HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    first_started.notified().await;
    let mut second = TcpStream::connect(front_addr).await.unwrap();
    second
        .write_all(b"GET /responses HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    release_first.notify_one();
    let mut first_response = Vec::new();
    first.read_to_end(&mut first_response).await.unwrap();
    assert!(first_response.starts_with(b"HTTP/1.1 200"));
    timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut second_response = Vec::new();
    second.read_to_end(&mut second_response).await.unwrap();
    assert!(second_response.starts_with(b"HTTP/1.1 200"));
    stop_frontend(stop, front).await;
}

#[tokio::test]
async fn direct_handler_shutdown_closes_listener_before_draining_response() {
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let handler_started = Arc::clone(&started);
    let handler_release = Arc::clone(&release);
    let (front_addr, stop, mut front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |config| config.drain_timeout = Duration::from_secs(1),
        move |_| {
            let started = Arc::clone(&handler_started);
            let release = Arc::clone(&handler_release);
            async move {
                started.notify_one();
                release.notified().await;
                full_response(b"done")
            }
        },
    )
    .await;
    let pending = tokio::spawn(async move {
        request(&test_client(), front_addr, "/responses", Bytes::new()).await
    });
    started.notified().await;
    stop.send(()).unwrap();
    let rebound = timeout(Duration::from_secs(1), async {
        loop {
            if let Ok(listener) = TcpListener::bind(front_addr).await {
                break listener;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!front.is_finished());
    drop(rebound);
    release.notify_one();
    assert_eq!(pending.await.unwrap().status(), StatusCode::OK);
    timeout(Duration::from_secs(1), &mut front)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
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
async fn direct_handler_upgrade_handoff_closes_before_drain_completes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let backend_addr = listener.local_addr().unwrap();
    let (closed_tx, closed_rx) = oneshot::channel();
    let backend = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_headers(&mut stream).await;
        stream
            .write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
            )
            .await
            .unwrap();
        let mut byte = [0_u8; 1];
        assert_eq!(stream.read(&mut byte).await.unwrap(), 0);
        closed_tx.send(()).unwrap();
    });
    let backend_handler = LoopbackBackend::new(backend_addr).unwrap();
    let (front_addr, stop, mut front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |_| {},
        move |request| {
            let backend = backend_handler.clone();
            async move { backend.handle(request).await }
        },
    )
    .await;
    let mut client = TcpStream::connect(front_addr).await.unwrap();
    client
        .write_all(
            b"GET /realtime HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
        )
        .await
        .unwrap();
    assert!(read_headers(&mut client).await.starts_with("HTTP/1.1 101"));

    stop.send(()).unwrap();
    timeout(Duration::from_secs(1), &mut front)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let mut byte = [0_u8; 1];
    assert_eq!(client.read(&mut byte).await.unwrap(), 0);
    timeout(Duration::from_secs(1), closed_rx)
        .await
        .unwrap()
        .unwrap();
    backend.await.unwrap();
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

#[tokio::test]
async fn production_edge_security_uses_peer_trust_and_rejects_host_origin_csrf_spoofing() {
    let untrusted_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&untrusted_calls);
    let (untrusted_addr, untrusted_stop, untrusted_front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |_| {},
        move |_| {
            let calls = Arc::clone(&calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                full_response(b"ok")
            }
        },
    )
    .await;
    let normal = send_raw_request(
        untrusted_addr,
        format!("POST /responses HTTP/1.1\r\nHost: {untrusted_addr}\r\nContent-Length: 0\r\n\r\n"),
    )
    .await;
    assert!(normal.starts_with("HTTP/1.1 200"), "{normal}");
    let spoofed = send_raw_request(
        untrusted_addr,
        format!(
            "POST /responses HTTP/1.1\r\nHost: {untrusted_addr}\r\nX-Forwarded-For: 198.51.100.7\r\nContent-Length: 0\r\n\r\n"
        ),
    )
    .await;
    assert!(spoofed.starts_with("HTTP/1.1 403"), "{spoofed}");
    assert_eq!(untrusted_calls.load(Ordering::SeqCst), 1);
    stop_frontend(untrusted_stop, untrusted_front).await;

    let observed = Arc::new(Mutex::new(Vec::new()));
    let handler_observed = Arc::clone(&observed);
    let (trusted_addr, trusted_stop, trusted_front) = spawn_direct_frontend(
        GatewayServerMode::ControlPlane,
        |config| {
            config.edge_security.trusted_proxies = vec!["127.0.0.1".parse().unwrap()];
            config.edge_security.browser = Some(GatewayServerBrowserSecurity {
                expected_origin: format!("http://{}", config.edge_security.expected_host),
                expected_csrf_token: Some("synthetic-csrf-token".to_string()),
            });
        },
        move |request| {
            let observed = Arc::clone(&handler_observed);
            async move {
                let has_forwarding_header = request.request.headers().keys().any(|name| {
                    name.as_str() == "forwarded" || name.as_str().starts_with("x-forwarded-")
                });
                observed.lock().unwrap().push((
                    request.peer_addr,
                    request.client_ip,
                    has_forwarding_header,
                ));
                full_response(b"ok")
            }
        },
    )
    .await;
    let trusted = send_raw_request(
        trusted_addr,
        format!(
            "POST /admin/keys HTTP/1.1\r\nHost: {trusted_addr}\r\nX-Forwarded-For: 198.51.100.99, 203.0.113.7, 127.0.0.1\r\nContent-Length: 0\r\n\r\n"
        ),
    )
    .await;
    assert!(trusted.starts_with("HTTP/1.1 200"), "{trusted}");
    let loopback_alias = send_raw_request(
        trusted_addr,
        format!(
            "POST /admin/keys HTTP/1.1\r\nHost: localhost:{}\r\nContent-Length: 0\r\n\r\n",
            trusted_addr.port()
        ),
    )
    .await;
    assert!(
        loopback_alias.starts_with("HTTP/1.1 200"),
        "{loopback_alias}"
    );

    for request in [
        "POST /admin/keys HTTP/1.1\r\nHost: attacker.example.com\r\nContent-Length: 0\r\n\r\n"
            .to_string(),
        format!(
            "POST /admin/keys HTTP/1.1\r\nHost: {trusted_addr}\r\nOrigin: https://attacker.example.com\r\nX-CSRF-Token: synthetic-csrf-token\r\nContent-Length: 0\r\n\r\n"
        ),
        format!(
            "POST /admin/keys HTTP/1.1\r\nHost: {trusted_addr}\r\nOrigin: http://{trusted_addr}\r\nX-CSRF-Token: wrong-token\r\nContent-Length: 0\r\n\r\n"
        ),
    ] {
        let response = send_raw_request(trusted_addr, request).await;
        assert!(response.starts_with("HTTP/1.1 403"), "{response}");
    }
    let browser = send_raw_request(
        trusted_addr,
        format!(
            "POST /admin/keys HTTP/1.1\r\nHost: {trusted_addr}\r\nOrigin: http://{trusted_addr}\r\nX-CSRF-Token: synthetic-csrf-token\r\nContent-Length: 0\r\n\r\n"
        ),
    )
    .await;
    assert!(browser.starts_with("HTTP/1.1 200"), "{browser}");
    {
        let observed = observed.lock().unwrap();
        assert_eq!(observed.len(), 3);
        assert!(
            observed
                .iter()
                .all(|(peer, _, forwarded)| peer.ip().is_loopback() && !forwarded)
        );
        assert_eq!(
            observed[0].1,
            "203.0.113.7".parse::<std::net::IpAddr>().unwrap()
        );
    }
    stop_frontend(trusted_stop, trusted_front).await;
}

#[test]
fn non_loopback_server_requires_explicit_expected_host() {
    let mut config = GatewayServerConfig::production(
        "0.0.0.0:4317".parse().unwrap(),
        GatewayServerMode::DataPlane,
    );
    assert!(config.validate().is_err());
    config.edge_security.expected_host = "gateway.example.com".to_string();
    assert!(config.validate().is_ok());
}

async fn send_raw_request(addr: std::net::SocketAddr, request: String) -> String {
    let mut client = TcpStream::connect(addr).await.unwrap();
    client.write_all(request.as_bytes()).await.unwrap();
    read_headers(&mut client).await
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
) -> TestFrontend {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mut config = GatewayServerConfig::production(addr, mode);
    config.response_header_timeout = Duration::from_secs(1);
    config.drain_timeout = Duration::from_secs(1);
    configure(&mut config);
    let (stop_tx, stop_rx) = oneshot::channel();
    let backend = LoopbackBackend::new(backend_addr).unwrap();
    let task = tokio::spawn(run_with_handler(
        listener,
        config,
        move |request| {
            let backend = backend.clone();
            async move { backend.handle(request).await }
        },
        async move { stop_rx.await.map_err(anyhow::Error::from) },
    ));
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

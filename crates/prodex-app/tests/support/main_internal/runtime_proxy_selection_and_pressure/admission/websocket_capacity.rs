use super::helpers::*;
use super::*;

#[test]
fn idle_websocket_connections_do_not_exhaust_workers_01651() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .build();
    let (paths, state) = {
        let runtime = harness.shared().runtime.lock().unwrap();
        (runtime.paths.clone(), runtime.state.clone())
    };
    let _workers = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT", "2");
    let _queue = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY", "8");
    let _logs = TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", paths.root.to_str().unwrap());
    let backend = RuntimeProxyBackend::start();
    let proxy =
        start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false).unwrap();
    let mut sockets = Vec::new();
    let mut failure = None;
    for index in 0..6 {
        let stream = TcpStream::connect(proxy.listen_addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        match tungstenite::client(
            format!("ws://{}/backend-api/prodex/responses", proxy.listen_addr),
            stream,
        ) {
            Ok((socket, response)) => {
                assert_eq!(response.status().as_u16(), 101);
                sockets.push(socket);
            }
            Err(error) => {
                failure = Some(format!("idle connection {index} blocked a worker: {error}"));
                break;
            }
        }
    }
    if failure.is_none() {
        for (index, socket) in sockets.iter_mut().enumerate() {
            socket
                .send(WsMessage::Ping(vec![index as u8].into()))
                .unwrap();
            assert_eq!(
                socket.read().unwrap(),
                WsMessage::Pong(vec![index as u8].into())
            );
        }
    }
    // Shut down real clients even on the old implementation's failure path.
    // A failing assertion must not strand a server worker or its test process.
    for mut socket in sockets {
        let _ = socket.close(None);
        let _ = socket.get_mut().shutdown(std::net::Shutdown::Both);
    }
    drop(proxy);
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}

fn with_small_websocket_pool(label: &str, body: impl FnOnce(&RuntimeRotationProxy)) {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .build();
    let (paths, state) = {
        let runtime = harness.shared().runtime.lock().unwrap();
        (runtime.paths.clone(), runtime.state.clone())
    };
    let _workers = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT", "2");
    let _queue = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY", "8");
    let _active = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT", "1");
    let _logs = TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", paths.root.to_str().unwrap());
    let backend = RuntimeProxyBackend::start();
    let proxy =
        start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false).unwrap();
    body(&proxy);
    assert!(
        backend.responses_accounts().is_empty(),
        "{label}: local control must not become paid inference"
    );
}

fn connect_local(proxy: &RuntimeRotationProxy) -> tungstenite::WebSocket<TcpStream> {
    let stream = TcpStream::connect(proxy.listen_addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    tungstenite::client(
        format!("ws://{}/backend-api/prodex/responses", proxy.listen_addr),
        stream,
    )
    .unwrap()
    .0
}

#[test]
fn parked_partial_frame_preserves_parser_and_does_not_hold_global_admission_01651() {
    use std::io::Write;
    with_small_websocket_pool("partial-frame", |proxy| {
        let mut first = connect_local(proxy);
        // Start a masked Ping but leave its header incomplete while another
        // connection needs the only global admission slot.
        first.get_mut().write_all(&[0x89]).unwrap();
        let mut second = connect_local(proxy);
        second
            .send(WsMessage::Ping(b"independent".to_vec().into()))
            .unwrap();
        assert_eq!(
            second.read().unwrap(),
            WsMessage::Pong(b"independent".to_vec().into())
        );
        let payload = b"parked";
        let mask = [1_u8, 2, 3, 4];
        let mut rest = vec![0x80 | payload.len() as u8];
        rest.extend_from_slice(&mask);
        rest.extend(
            payload
                .iter()
                .enumerate()
                .map(|(i, byte)| byte ^ mask[i % 4]),
        );
        first.get_mut().write_all(&rest).unwrap();
        assert_eq!(
            first.read().unwrap(),
            WsMessage::Pong(payload.to_vec().into())
        );
        for mut socket in [first, second] {
            let _ = socket.close(None);
            let _ = socket.get_mut().shutdown(std::net::Shutdown::Both);
        }
    });
}

#[test]
fn bounded_idle_connection_cache_evicts_idle_not_new_work_01651() {
    with_small_websocket_pool("bounded-idle-cache", |proxy| {
        let mut sockets = (0..14).map(|_| connect_local(proxy)).collect::<Vec<_>>();
        let last = sockets.last_mut().unwrap();
        last.send(WsMessage::Ping(b"new-work".to_vec().into()))
            .unwrap();
        assert_eq!(
            last.read().unwrap(),
            WsMessage::Pong(b"new-work".to_vec().into())
        );
        // Polling rotates parked sockets. Bound the number retained instead of
        // assuming that FIFO scheduling implements an unrelated LRU policy.
        let mut closed = std::collections::BTreeSet::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while closed.len() < 4 && Instant::now() < deadline {
            for (index, socket) in sockets.iter_mut().take(13).enumerate() {
                if closed.contains(&index) {
                    continue;
                }
                socket.get_mut().set_nonblocking(true).unwrap();
                let read = socket.read();
                socket.get_mut().set_nonblocking(false).unwrap();
                match read {
                    Err(tungstenite::Error::Io(error))
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(_) | Ok(WsMessage::Close(_)) => {
                        closed.insert(index);
                    }
                    other => panic!("unexpected idle fixture frame: {other:?}"),
                }
            }
            if closed.len() < 4 {
                thread::sleep(Duration::from_millis(10));
            }
        }
        assert!(
            closed.len() >= 4,
            "14 connections must not exceed 8 parked + 2 exclusive worker owners; reclaimed {closed:?}"
        );
        for mut socket in sockets {
            let _ = socket.close(None);
            let _ = socket.get_mut().shutdown(std::net::Shutdown::Both);
        }
    });
}

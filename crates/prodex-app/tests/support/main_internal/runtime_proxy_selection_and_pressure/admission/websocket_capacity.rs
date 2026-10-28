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

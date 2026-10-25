use super::*;

#[test]
fn runtime_proxy_two_sessions_with_sixteen_workers_complete_through_capacity_pressure() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();
    // Exercise 34 queued streams with the production watchdog budget, rather
    // than the shortened per-unit fixture timeout. Socket I/O remains bounded
    // to 60 seconds and all identity/exactly-once assertions stay unchanged.
    let _idle_timeout =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "300000");
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_capacity_pressure(),
        "second",
        &["second"],
        &[],
        Vec::new(),
    );
    const PARENTS: usize = 2;
    const WORKERS_PER_PARENT: usize = 16;
    const REQUESTS: usize = PARENTS * (WORKERS_PER_PARENT + 1);
    let start = std::sync::Barrier::new(REQUESTS);
    let completed = thread::scope(|parents| {
        let groups = (0..PARENTS).map(|parent| {
            let fixture = &fixture;
            let start = &start;
            parents.spawn(move || thread::scope(|workers| {
                let handles = (0..=WORKERS_PER_PARENT).map(|worker| {
                    workers.spawn(move || {
                        let session = format!("parallel-{parent}-{worker}");
                        start.wait();
                        let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
                        set_test_websocket_io_timeout(&mut socket, Duration::from_secs(60));
                        send_runtime_websocket_json(&mut socket, serde_json::json!({
                            "session_id": session,
                            "input": [{
                                "type": "message",
                                "role": "user",
                                "content": format!("work for {session}"),
                            }],
                        }));
                        let (frames, terminal) = read_runtime_websocket_until(&mut socket, |text| {
                            let event: serde_json::Value = serde_json::from_str(text).unwrap();
                            matches!(event["type"].as_str(), Some("response.completed" | "error" | "response.failed"))
                        });
                        let _ = socket.close(None);
                        let terminal: serde_json::Value = serde_json::from_str(&terminal).unwrap();
                        assert_eq!(terminal["type"], "response.completed", "{session}: {terminal}");
                        let expected_id = format!("resp-capacity-{session}");
                        assert_eq!(terminal["response"]["id"], expected_id);
                        let events = frames.iter().map(|frame| serde_json::from_str::<serde_json::Value>(frame).unwrap()).collect::<Vec<_>>();
                        assert_eq!(events.iter().filter(|event| event["type"] == "response.output_text.delta").count(), 1,
                            "each client must receive its output exactly once: {session}");
                        assert!(events.iter().all(|event| event["type"] != "error" && event["type"] != "response.failed"),
                            "healthy queued work must not receive a local error: {session}");
                        expected_id
                    })
                }).collect::<Vec<_>>();
                handles.into_iter().map(|handle| handle.join().expect("fanout worker should finish")).collect::<Vec<_>>()
            }))
        }).collect::<Vec<_>>();
        groups
            .into_iter()
            .flat_map(|group| group.join().expect("parent session should finish"))
            .collect::<Vec<_>>()
    });
    assert_eq!(completed.len(), REQUESTS);
    assert_eq!(
        completed.into_iter().collect::<BTreeSet<_>>().len(),
        REQUESTS,
        "response identity must remain isolated across both parent/worker groups"
    );
    assert_eq!(
        fixture.backend.websocket_requests().len(),
        REQUESTS,
        "local queuing must not duplicate upstream dispatch"
    );
    let log = fixture.wait_for_log(|log| log.contains("inflight_wait_finished"));
    assert!(
        log.contains("inflight_wait_started"),
        "fanout must exercise actual profile backpressure: {log}"
    );
    assert!(
        !log.contains("precommit_budget_exhausted"),
        "local queue time must not exhaust upstream budget: {log}"
    );
}

#[test]
fn runtime_proxy_idle_websockets_leave_capacity_for_compaction_on_the_only_ready_profile() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();
    // This backend deliberately holds committed output for 1,050ms, longer
    // than the 250ms unit idle timeout. Use the production stream budget.
    let _idle_timeout =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "300000");
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_capacity_pressure(),
        "second",
        &["second"],
        &[],
        Vec::new(),
    );
    let mut idle_sockets = Vec::new();
    for index in 0..4 {
        let metadata = [runtime_continuation_header(
            "x-codex-turn-metadata",
            codex_0155_compaction_turn_metadata(),
        )];
        let mut socket = if index == 3 {
            fixture.connect_websocket_with_headers("backend-api/prodex/responses", &metadata)
        } else {
            fixture.connect_websocket("backend-api/prodex/responses")
        };
        set_test_websocket_io_timeout(&mut socket, Duration::from_secs(10));
        let session = format!("idle-capacity-{index}");
        send_runtime_websocket_json(
            &mut socket,
            serde_json::json!({
                "session_id": session,
                "input": [{"type":"message", "role":"user", "content":format!("work for {session}")}],
            }),
        );
        let (frames, terminal) = read_runtime_websocket_until(&mut socket, |text| {
            let event: serde_json::Value = serde_json::from_str(text).unwrap();
            matches!(
                event["type"].as_str(),
                Some("response.completed" | "error" | "response.failed")
            )
        });
        let event: serde_json::Value = serde_json::from_str(&terminal).unwrap();
        assert_eq!(event["type"], "response.completed", "{session}: {event}");
        assert_eq!(event["response"]["id"], format!("resp-capacity-{session}"));
        assert_eq!(
            frames
                .iter()
                .filter(|text| text.contains("response.output_text.delta"))
                .count(),
            1
        );
        // Unlike the old fan-out fixture, keep every completed client open.
        // Native parent, side-agent and compaction sessions cache their sockets.
        idle_sockets.push(socket);
    }
    assert_eq!(
        fixture.backend.websocket_requests().len(),
        4,
        "idle clients must not cause duplicate upstream replay or starve compaction"
    );
    for socket in &mut idle_sockets {
        let _ = socket.close(None);
    }
}

#[test]
fn runtime_proxy_saturated_idle_websocket_workers_do_not_block_unary_compact_04391() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let _workers = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT", "2");
    let _acceptors = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_WORKER_COUNT", "1");
    let _queue = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY", "1");
    let _idle = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "300000");
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_capacity_pressure(),
        "second",
        &["second"],
        &[],
        Vec::new(),
    );
    // Native parent and sub-agent clients retain these connections while idle.
    // Fill the entire connection pool, not just the per-profile execution cap.
    let mut sockets = (0..2)
        .map(|_| fixture.connect_websocket("backend-api/prodex/responses"))
        .collect::<Vec<_>>();
    // One retry fills the queue; the next previously blocked the only acceptor.
    let pending = (0..2).map(|_| {
        let mut stream = TcpStream::connect(fixture.listen_addr()).unwrap();
        // WebSocket handshake nonces are ephemeral protocol values, not credentials.
        let nonce = tungstenite::handshake::client::generate_key();
        let handshake = format!("GET /backend-api/prodex/responses HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {nonce}\r\n\r\n");
        stream.write_all(handshake.as_bytes()).unwrap();
        stream
    }).collect::<Vec<_>>();
    let pressure = fixture.wait_for_log(|log| {
        log.contains("runtime_proxy_queue_wait_started")
            || log.contains("runtime_proxy_queue_overloaded")
    });
    let health = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
        .get(format!(
            "http://{}/__prodex/runtime/log/snapshot",
            fixture.listen_addr()
        ))
        .send()
        .map(|response| response.status());
    let result = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            fixture.listen_addr()
        ))
        .json(&serde_json::json!({"model":"gpt-5", "input":[
            {"type":"message", "role":"user", "content":"preserve completed work"}
        ]}))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.text());
    // Release held workers even on the old, failing implementation.
    for socket in &mut sockets {
        let _ = socket.close(None);
    }
    drop(sockets);
    drop(pending);
    assert!(
        pressure.contains("runtime_proxy_queue_wait_started")
            || pressure.contains("runtime_proxy_queue_overloaded"),
        "the regression must exercise an actually full queue"
    );
    assert!(
        health.as_ref().is_ok_and(|status| status.is_client_error()),
        "unauthenticated control must respond without bypassing authentication or waiting for upstream capacity: {health:?}"
    );
    assert!(
        result.is_ok(),
        "idle WebSocket workers must not starve unary compact: {result:?}"
    );
    assert_eq!(
        fixture.backend.responses_accounts().len(),
        1,
        "compaction must be dispatched exactly once on its selected profile"
    );
    assert!(
        fixture.backend.websocket_requests().is_empty(),
        "idle cached sockets must not fabricate or replay upstream work"
    );
}

#[test]
fn runtime_proxy_idle_websocket_pool_preserves_http_fallback_capacity_04391() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let _workers = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT", "2");
    let _idle = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "300000");
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_buffered_json(),
        "second",
        &["second"],
        &[],
        Vec::new(),
    );
    let mut sockets = (0..2)
        .map(|_| fixture.connect_websocket("backend-api/prodex/responses"))
        .collect::<Vec<_>>();
    let result = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap()
        .post(format!(
            "http://{}/backend-api/prodex/responses",
            fixture.listen_addr()
        ))
        .json(&serde_json::json!({"model":"gpt-5", "stream":false, "input":"continue once"}))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.text());
    for socket in &mut sockets {
        let _ = socket.close(None);
    }
    drop(sockets);
    assert!(
        result.is_ok(),
        "HTTP fallback must not share idle WebSocket connection workers: {result:?}"
    );
    assert_eq!(
        fixture.backend.responses_accounts().len(),
        1,
        "a transport fallback must not duplicate accepted input"
    );
    assert!(fixture.backend.websocket_requests().is_empty());
}

use super::*;

#[test]
fn runtime_proxy_two_sessions_with_sixteen_workers_complete_through_capacity_pressure() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();
    // Keep the short synthetic 250ms idle timeout from turning a healthy
    // one-second stream into a transport failure. The retry budget is unchanged.
    let _idle_timeout = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "5000");
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
        groups.into_iter().flat_map(|group| group.join().expect("parent session should finish")).collect::<Vec<_>>()
    });
    assert_eq!(completed.len(), REQUESTS);
    assert_eq!(completed.into_iter().collect::<BTreeSet<_>>().len(), REQUESTS,
        "response identity must remain isolated across both parent/worker groups");
    assert_eq!(fixture.backend.websocket_requests().len(), REQUESTS,
        "local queuing must not duplicate upstream dispatch");
    let log = fixture.wait_for_log(|log| log.contains("inflight_wait_finished"));
    assert!(log.contains("inflight_wait_started"), "fanout must exercise actual profile backpressure: {log}");
    assert!(!log.contains("precommit_budget_exhausted"), "local queue time must not exhaust upstream budget: {log}");
}

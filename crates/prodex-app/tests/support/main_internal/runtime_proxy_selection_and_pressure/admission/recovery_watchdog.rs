use super::helpers::*;
use super::*;
use crate::runtime_proxy::{RuntimeProxyRecoveryTimeout, await_runtime_proxy_async_task};
use std::sync::atomic::{AtomicBool, Ordering};

fn request() -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".into(),
        path_and_query: "/backend-api/codex/responses".into(),
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: br#"{"input":[]}"#.to_vec(),
    }
}

#[test]
fn recovery_watchdog_is_request_local_and_not_reset_by_retry_clones() {
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main")
        .current_profile("main")
        .build();
    let global = harness.shared();
    assert!(global.precommit_deadline.is_none());
    let first = global.for_precommit_request();
    assert!(first.precommit_deadline.is_some());
    let next_attempt = first.for_precommit_request();
    assert_eq!(first.precommit_deadline, next_attempt.precommit_deadline);
    assert!(
        global.precommit_deadline.is_none(),
        "another request must not inherit an expired deadline"
    );
}

#[test]
fn recovery_watchdog_cancels_precommit_async_wait_instead_of_orphaning_it() {
    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main")
        .current_profile("main")
        .build();
    let mut shared = harness.shared().clone();
    shared.precommit_deadline = Some(Instant::now() + Duration::from_millis(80));
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = Dropped(dropped.clone());
    let started = Instant::now();
    let result =
        await_runtime_proxy_async_task(&shared, "synthetic_stalled_precommit", async move {
            let _guard = guard;
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(())
        });
    assert!(result.unwrap_err().is::<RuntimeProxyRecoveryTimeout>());
    assert!(started.elapsed() < Duration::from_secs(2));
    for _ in 0..100 {
        if dropped.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        dropped.load(Ordering::SeqCst),
        "timed out async work must be cancelled, not detached"
    );
}

#[test]
fn saturated_pool_watchdog_leaves_quota_intact_and_next_request_usable() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::sse_success(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
                "healthy-after-capacity-release",
            ),
        ]));
    let ready = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(80, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready)
        .build();
    let mut scoped = harness.shared().clone();
    scoped.precommit_deadline = Some(Instant::now() + Duration::from_millis(100));
    let limit = scoped.runtime_config.tuning.profile_inflight_hard_limit;
    scoped.lane_admission.set_profile_inflight("main", limit);
    let err = match proxy_runtime_responses_request(1200, &request(), &scoped) {
        Err(error) => error,
        Ok(_) => panic!("permanently saturated pool must not occupy the request forever"),
    };
    assert!(err.is::<RuntimeProxyRecoveryTimeout>(), "{err:#}");
    assert!(
        backend.responses_accounts().is_empty(),
        "local waiting is not an upstream attempt"
    );
    scoped.lane_admission.set_profile_inflight("main", 0);
    scoped.lane_admission.record_inflight_release();
    let response = proxy_runtime_responses_request(1201, &request(), harness.shared()).unwrap();
    let status = match response {
        RuntimeResponsesReply::Buffered(parts) => parts.status,
        RuntimeResponsesReply::Streaming(mut reply) => {
            let mut body = String::new();
            reply.body.read_to_string(&mut body).unwrap();
            reply.status
        }
    };
    assert_eq!(status, 200);
    assert_eq!(scoped.lane_admission.profile_inflight_count("main"), 0);
    assert_eq!(backend.responses_accounts(), ["main-account"]);
    let log = read_runtime_proxy_test_log(&scoped.log_path);
    assert!(
        !log.contains("quota_quarantine"),
        "a local watchdog must not exhaust or penalize account quota: {log}"
    );
}

#[test]
fn committed_stream_remains_readable_after_precommit_watchdog_expires() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::sse_success(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
                "committed-survives",
            ),
        ]));
    let ready = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(80, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready)
        .build();
    let mut scoped = harness.shared().clone();
    scoped.precommit_deadline = Some(Instant::now() + Duration::from_secs(1));
    let RuntimeResponsesReply::Streaming(mut response) =
        proxy_runtime_responses_request(1202, &request(), &scoped).unwrap()
    else {
        panic!("fixture must commit a stream");
    };
    let wait = scoped
        .precommit_deadline
        .unwrap()
        .saturating_duration_since(Instant::now());
    thread::sleep(wait + Duration::from_millis(20));
    let mut body = String::new();
    response.body.read_to_string(&mut body).unwrap();
    assert_eq!(response.status, 200);
    assert!(body.contains("committed-survives"), "{body}");
    assert_eq!(backend.responses_accounts(), ["main-account"]);
}

#[test]
fn retryable_profile_cooldown_outliving_one_wait_epoch_does_not_surface_local_503() {
    let ready = runtime_usage_snapshot(quota_window_ready(87, 3600), quota_window_ready(91, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main")
        .current_profile("main")
        .profile_usage_snapshot("main", ready)
        .build();
    let mut scoped = harness.shared().clone();
    scoped.precommit_deadline = Some(Instant::now() + Duration::from_secs(60));
    {
        let mut runtime = scoped.runtime.lock().unwrap();
        runtime
            .profile_retry_backoff_until
            .insert("main".into(), Local::now().timestamp() + 90);
    }
    let (retry_pending, exclusion_cleared, sweeps) =
        crate::runtime_proxy::test_runtime_precommit_retryable_recovery_after_transport(
            &scoped,
            RuntimeRouteKind::Responses,
            "main",
        )
        .unwrap();
    assert!(
        retry_pending,
        "a 30-second wait epoch is not a terminal 90-second account cooldown"
    );
    assert!(
        !exclusion_cleared,
        "must honor the provider cooldown, not retry the blocked account early"
    );
    assert_eq!(sweeps, 1);
}

#[test]
fn expired_recovery_watchdog_does_not_reopen_a_ready_profile_sweep_04381() {
    let ready = runtime_usage_snapshot(quota_window_ready(90, 3600), quota_window_ready(90, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main")
        .current_profile("main")
        .profile_usage_snapshot("main", ready)
        .build();
    let mut scoped = harness.shared().clone();
    scoped.precommit_deadline = Some(Instant::now() - Duration::from_millis(1));
    // A recovered profile is the no-sleep path: it must still honor the
    // original request deadline instead of repeatedly clearing exclusions.
    let result = crate::runtime_proxy::test_runtime_precommit_retryable_recovery_after_transport(
        &scoped,
        RuntimeRouteKind::Responses,
        "main",
    );
    assert!(
        matches!(result, Err(ref error) if error.is::<RuntimeProxyRecoveryTimeout>()),
        "an expired request must not start another recovery sweep: {result:?}"
    );
    let log = read_runtime_proxy_test_log(&scoped.log_path);
    assert!(
        !log.contains("rotation_sweep_start"),
        "no recovery after expiry: {log}"
    );
    assert!(
        !log.contains("profile_transport_failure"),
        "expiry is not an upstream account fault: {log}"
    );
}

#[test]
fn expired_recovery_watchdog_does_not_enter_admission_after_capacity_returns_04381() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .active_request_limit(1)
    .build();
    let shared = harness.shared();
    let held = try_acquire_runtime_proxy_active_request_slot(
        shared,
        "http",
        "/backend-api/codex/responses",
    )
    .unwrap();
    let mut scoped = shared.clone();
    scoped.precommit_deadline = Some(Instant::now() + Duration::from_millis(60));
    let (done, receive) = std::sync::mpsc::channel();
    let waiter = thread::spawn(move || {
        let result = acquire_runtime_proxy_active_request_slot_with_wait_for_request(
            &scoped,
            "http",
            "/backend-api/codex/responses",
            None,
        )
        .map(drop);
        let _ = done.send(result);
    });
    let terminated = receive.recv_timeout(Duration::from_millis(500));
    // Rescue old code before asserting: no stranded waiter on failure.
    drop(held);
    waiter.join().unwrap();
    assert!(
        terminated.is_ok_and(|result| result.is_err()),
        "expired admission must terminate without waiting for an unrelated release"
    );
    assert_eq!(shared.active_request_count.load(Ordering::SeqCst), 0);
}

#[test]
fn expired_recovery_watchdog_does_not_enqueue_after_capacity_returns_04381() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .build();
    let mut scoped = harness.shared().clone();
    scoped.precommit_deadline = Some(Instant::now());
    let mut dispatched = false;
    let result = try_enqueue_runtime_proxy_work(17_u64, &scoped, |_| {
        dispatched = true;
        Ok(())
    });
    assert!(matches!(
        result,
        Err((RuntimeProxyQueueRejection::Full, 17))
    ));
    assert!(
        !dispatched,
        "expired work must not enter even an available queue"
    );
}

#[test]
fn queued_request_keeps_ingress_deadline_and_returns_504_before_upstream_04381() {
    use crate::runtime_proxy::{
        RuntimeProxyQueuedRequest, enqueue_runtime_proxy_request,
        handle_runtime_rotation_proxy_request,
    };
    use std::io::{Read, Write};
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let ready = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(80, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready)
        .build();
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let mut client = std::net::TcpStream::connect(server.server_addr().to_ip().unwrap()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(b"POST /backend-api/codex/responses HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 2\r\n\r\n{}").unwrap();
    let request = server
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    let (sender, receiver) = std::sync::mpsc::sync_channel::<RuntimeProxyQueuedRequest>(1);
    let mut scope = harness.shared().clone();
    let deadline = Instant::now() + Duration::from_millis(80);
    scope.precommit_deadline = Some(deadline);
    assert!(enqueue_runtime_proxy_request(&sender, request, &scope).is_ok());
    let queued = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(queued.deadline, Some(deadline));
    thread::sleep(deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(20));
    let mut worker_scope = harness.shared().clone();
    worker_scope.precommit_deadline = queued.deadline;
    handle_runtime_rotation_proxy_request(queued.request, &worker_scope);
    let mut reply = String::new();
    client.read_to_string(&mut reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 504"), "{reply}");
    assert!(reply.contains("request_timeout"), "{reply}");
    assert!(backend.responses_accounts().is_empty());
    assert_eq!(scope.active_request_count.load(Ordering::SeqCst), 0);
    assert_eq!(scope.lane_admission.profile_inflight_count("main"), 0);
    let log = read_runtime_proxy_test_log(&scope.log_path);
    assert!(
        !log.contains("quota_quarantine") && !log.contains("profile_transport_failure"),
        "{log}"
    );
}

#[test]
fn standard_retry_scope_stops_expired_work_without_penalizing_ready_profile_04381() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let ready = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(80, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready)
        .build();
    let mut scope = harness.shared().clone();
    scope.precommit_deadline = Some(Instant::now() - Duration::from_millis(1));
    let request = RuntimeProxyRequest {
        method: "GET".into(),
        path_and_query: "/backend-api/status".into(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let failed = proxy_runtime_standard_request(4381001, &request, &scope);
    assert!(matches!(failed, Err(ref error) if error.is::<RuntimeProxyRecoveryTimeout>()));
    assert!(backend.responses_accounts().is_empty());
    let response = proxy_runtime_standard_request(4381002, &request, harness.shared()).unwrap();
    let (status, _) = tiny_http_response_status_and_body(response);
    assert_eq!(status, 200);
    assert_eq!(backend.responses_accounts(), ["main-account"]);
    assert!(harness.shared().precommit_deadline.is_none());
}

#[test]
fn ready_profile_notification_interrupts_long_recovery_sleep_04381() {
    let ready = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(80, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main")
        .current_profile("main")
        .profile_usage_snapshot("main", ready)
        .build();
    harness
        .shared()
        .runtime
        .lock()
        .unwrap()
        .profile_retry_backoff_until
        .insert("main".into(), Local::now().timestamp() + 90);
    let mut scoped = harness.shared().clone();
    scoped.precommit_deadline = Some(Instant::now() + Duration::from_secs(2));
    let result = std::thread::scope(|threads| {
        let release = threads.spawn(|| {
            let observed_by = Instant::now() + Duration::from_secs(1);
            while !read_runtime_proxy_test_log(&scoped.log_path)
                .contains("rotation_waiting_for_recovery")
            {
                assert!(
                    Instant::now() < observed_by,
                    "must observe the real cooldown wait"
                );
                thread::sleep(Duration::from_millis(5));
            }
            scoped
                .runtime
                .lock()
                .unwrap()
                .profile_retry_backoff_until
                .remove("main");
            scoped.lane_admission.notify_selection_change();
        });
        let result =
            crate::runtime_proxy::test_runtime_precommit_retryable_recovery_after_transport(
                &scoped,
                RuntimeRouteKind::Responses,
                "main",
            );
        release.join().unwrap();
        result
    });
    assert!(
        matches!(result, Ok((true, true, 1))),
        "a ready-profile notification must interrupt cooldown before the request expires: {result:?}"
    );
    assert!(
        !read_runtime_proxy_test_log(&scoped.log_path)
            .contains("precommit_recovery_deadline_exceeded")
    );
}

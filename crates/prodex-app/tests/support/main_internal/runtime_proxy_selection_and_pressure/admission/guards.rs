use super::helpers::*;
use super::*;

#[test]
fn active_request_guard_drop_records_lane_release() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .active_request_limit(4)
    .build();
    let shared = harness.shared();

    let guard = try_acquire_runtime_proxy_active_request_slot(
        shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("active request slot should be acquired");

    assert_eq!(shared.active_request_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        shared
            .lane_admission
            .active_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        shared
            .lane_admission
            .admissions_total_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        1
    );

    drop(guard);

    assert_eq!(shared.active_request_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        shared
            .lane_admission
            .active_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(
        shared
            .lane_admission
            .releases_total_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        shared
            .lane_admission
            .active_request_release_underflows_total(),
        0
    );
    assert_eq!(
        shared
            .lane_admission
            .release_underflows_total_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        0
    );
}

#[test]
fn active_request_guard_drop_records_underflow_without_wrapping() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .active_request_limit(4)
    .build();
    let shared = harness.shared();

    let guard = try_acquire_runtime_proxy_active_request_slot(
        shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("active request slot should be acquired");
    shared.active_request_count.store(0, Ordering::SeqCst);
    shared
        .lane_admission
        .active_counter(RuntimeRouteKind::Responses)
        .store(0, Ordering::SeqCst);

    drop(guard);

    assert_eq!(shared.active_request_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        shared
            .lane_admission
            .active_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(
        shared
            .lane_admission
            .releases_total_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        shared
            .lane_admission
            .active_request_release_underflows_total(),
        1
    );
    assert_eq!(
        shared
            .lane_admission
            .release_underflows_total_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        1
    );
}

#[test]
fn response_lane_limit_still_rejects_fresh_request() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .active_request_limit(4)
    .build();
    let shared = harness.shared();
    let limit = shared.lane_admission.limit(RuntimeRouteKind::Responses);
    shared
        .lane_admission
        .active_counter(RuntimeRouteKind::Responses)
        .store(limit, Ordering::SeqCst);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: Vec::new(),
        body: br#"{"input":"fresh"}"#.to_vec(),
    };

    assert!(matches!(
        acquire_runtime_proxy_active_request_slot_with_wait_for_request(
            shared,
            "http",
            "/backend-api/codex/responses",
            Some(&request),
        ),
        Err(RuntimeProxyAdmissionRejection::LaneLimit(
            RuntimeRouteKind::Responses
        ))
    ));
    let wait = shared
        .lane_admission
        .admission_wait_metric_counters()
        .snapshot();
    assert_eq!(wait.wait_count, 1);
    assert!(wait.wait_total_ns > 0);
}

#[test]
fn active_request_admission_wait_metric_records_recovery_once() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(100, 250);
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
    .expect("first request should hold admission capacity");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(held);
    });

    let recovered = acquire_runtime_proxy_active_request_slot_with_wait(
        shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("admission should recover after the held request exits");
    release.join().expect("release thread should join");

    let wait = shared
        .lane_admission
        .admission_wait_metric_counters()
        .snapshot();
    assert_eq!(wait.wait_count, 1);
    assert!(wait.wait_total_ns > 0);
    drop(recovered);
}

#[test]
fn response_lane_limit_does_not_override_owned_previous_response_affinity() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .active_request_limit(4)
    .build();
    let shared = harness.shared();
    {
        let mut runtime = shared.runtime.lock().expect("runtime state should lock");
        runtime.state.response_profile_bindings.insert(
            "resp-owned".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
    }
    let limit = shared.lane_admission.limit(RuntimeRouteKind::Responses);
    shared
        .lane_admission
        .active_counter(RuntimeRouteKind::Responses)
        .store(limit, Ordering::SeqCst);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: Vec::new(),
        body: br#"{"previous_response_id":"resp-owned","input":"continue"}"#.to_vec(),
    };

    let guard = acquire_runtime_proxy_active_request_slot_with_wait_for_request(
        shared,
        "http",
        "/backend-api/codex/responses",
        Some(&request),
    )
    .expect("owned previous_response_id should bypass only the lane cap");

    assert_eq!(
        shared.active_request_count.load(Ordering::SeqCst),
        1,
        "global active counter should still be enforced"
    );
    assert_eq!(
        shared
            .lane_admission
            .active_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        limit + 1
    );
    assert_eq!(
        shared
            .lane_admission
            .admission_wait_metric_counters()
            .snapshot()
            .wait_count,
        0
    );
    drop(guard);
}

#[test]
fn compact_lane_limit_does_not_override_owned_turn_state_lineage() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .active_request_limit(4)
    .build();
    let shared = harness.shared();
    shared
        .runtime
        .lock()
        .expect("runtime state should lock")
        .turn_state_bindings
        .insert(
            runtime_compact_turn_state_lineage_key("turn-owned"),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
    let limit = shared.lane_admission.limit(RuntimeRouteKind::Compact);
    shared
        .lane_admission
        .active_counter(RuntimeRouteKind::Compact)
        .store(limit, Ordering::SeqCst);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![(
            "x-codex-turn-state".to_string(),
            "turn-owned".to_string(),
        )],
        body: br#"{"input":"continue"}"#.to_vec(),
    };

    let guard = acquire_runtime_proxy_active_request_slot_with_wait_for_request(
        shared,
        "http",
        "/backend-api/codex/responses/compact",
        Some(&request),
    )
    .expect("owned compact lineage should bypass only the compact lane cap");

    assert_eq!(
        shared
            .lane_admission
            .active_counter(RuntimeRouteKind::Compact)
            .load(Ordering::SeqCst),
        limit + 1
    );
    drop(guard);
}

#[test]
fn websocket_lane_limit_does_not_override_owned_turn_state_affinity() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .active_request_limit(4)
    .build();
    let shared = harness.shared();
    shared
        .runtime
        .lock()
        .expect("runtime state should lock")
        .turn_state_bindings
        .insert(
            "turn-owned".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
    let limit = shared.lane_admission.limit(RuntimeRouteKind::Websocket);
    shared
        .lane_admission
        .active_counter(RuntimeRouteKind::Websocket)
        .store(limit, Ordering::SeqCst);
    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![(
            "x-codex-turn-state".to_string(),
            "turn-owned".to_string(),
        )],
        body: Vec::new(),
    };

    let guard = acquire_runtime_proxy_active_request_slot_with_wait_for_request(
        shared,
        "websocket",
        "/backend-api/codex/responses",
        Some(&request),
    )
    .expect("owned websocket turn state should bypass only the lane cap");

    assert_eq!(
        shared
            .lane_admission
            .active_counter(RuntimeRouteKind::Websocket)
            .load(Ordering::SeqCst),
        limit + 1
    );
    drop(guard);
}

#[test]
fn long_lived_queue_wait_metrics_record_each_started_wait_once() {
    let _budget_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS", "5");
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .build();
    let shared = harness.shared();
    let path = "/backend-api/codex/responses";
    let wait_count = || {
        shared
            .lane_admission
            .long_lived_queue_wait_metric_counters()
            .snapshot()
            .wait_count
    };

    wait_for_runtime_proxy_queue_capacity((), shared, "http", path, |_| Ok(()))
        .expect("immediate queue capacity should not wait");
    assert_eq!(wait_count(), 0);

    let mut attempts = 0;
    wait_for_runtime_proxy_queue_capacity((), shared, "http", path, |item| {
        attempts += 1;
        if attempts == 1 {
            Err((RuntimeProxyQueueRejection::Full, item))
        } else {
            Ok(())
        }
    })
    .expect("second queue probe should recover");
    assert_eq!(wait_count(), 1);

    let mut attempts = 0;
    wait_for_runtime_proxy_queue_capacity((), shared, "http", path, |item| {
        attempts += 1;
        if attempts <= 2 {
            Err((RuntimeProxyQueueRejection::Full, item))
        } else {
            Ok(())
        }
    })
    .expect("queue should recover after its bounded wait");
    assert_eq!(wait_count(), 2);

    let mut attempts = 0;
    assert!(matches!(
        wait_for_runtime_proxy_queue_capacity((), shared, "http", path, |item| {
            attempts += 1;
            if attempts == 1 {
                Err((RuntimeProxyQueueRejection::Full, item))
            } else {
                Err((RuntimeProxyQueueRejection::Disconnected, item))
            }
        }),
        Err((RuntimeProxyQueueRejection::Disconnected, ()))
    ));
    assert_eq!(wait_count(), 3);

    assert!(matches!(
        wait_for_runtime_proxy_queue_capacity((), shared, "http", path, |item| Err((
            RuntimeProxyQueueRejection::Full,
            item,
        ))),
        Err((RuntimeProxyQueueRejection::Full, ()))
    ));
    assert_eq!(wait_count(), 4);

    assert!(matches!(
        wait_for_runtime_proxy_queue_capacity((), shared, "http", path, |item| Err((
            RuntimeProxyQueueRejection::Disconnected,
            item,
        ))),
        Err((RuntimeProxyQueueRejection::Disconnected, ()))
    ));
    assert_eq!(wait_count(), 4);

    let wait = shared
        .lane_admission
        .long_lived_queue_wait_metric_counters()
        .snapshot();
    assert_eq!(wait.wait_count, 4);
    assert!(wait.wait_total_ns > 0);
    assert!(wait.wait_max_ns > 0);
}

#[test]
fn response_lane_limit_retry_bypasses_owned_previous_response_affinity() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .active_request_limit(4)
    .build();
    let shared = harness.shared();
    {
        let mut runtime = shared.runtime.lock().expect("runtime state should lock");
        runtime.state.response_profile_bindings.insert(
            "resp-owned".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
    }
    let limit = shared.lane_admission.limit(RuntimeRouteKind::Responses);
    shared
        .lane_admission
        .active_counter(RuntimeRouteKind::Responses)
        .store(limit, Ordering::SeqCst);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: Vec::new(),
        body: br#"{"previous_response_id":"resp-owned","input":"continue"}"#.to_vec(),
    };

    assert!(matches!(
        acquire_runtime_proxy_active_request_slot_with_wait(
            shared,
            "http",
            "/backend-api/codex/responses",
        ),
        Err(RuntimeProxyAdmissionRejection::LaneLimit(
            RuntimeRouteKind::Responses
        ))
    ));

    let guard = acquire_runtime_proxy_active_request_slot_with_wait_for_request(
        shared,
        "http",
        "/backend-api/codex/responses",
        Some(&request),
    )
    .expect("owned previous_response_id should be retried with request context");

    assert_eq!(
        shared.active_request_count.load(Ordering::SeqCst),
        1,
        "global active counter should still be enforced"
    );
    assert_eq!(
        shared
            .lane_admission
            .active_counter(RuntimeRouteKind::Responses)
            .load(Ordering::SeqCst),
        limit + 1
    );
    drop(guard);
}

#[test]
fn profile_inflight_guard_drop_records_release() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .build();
    let shared = harness.shared();
    let context = runtime_route_kind_inflight_context(RuntimeRouteKind::Responses);
    let weight = runtime_profile_inflight_weight(context);

    let guard = acquire_runtime_profile_inflight_guard(shared, "main", context)
        .expect("profile inflight guard should be acquired");

    assert_eq!(shared.lane_admission.profile_inflight_admissions_total(), 1);
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), weight);

    drop(guard);

    assert_eq!(shared.lane_admission.profile_inflight_releases_total(), 1);
    assert_eq!(
        shared
            .lane_admission
            .profile_inflight_release_underflows_total(),
        0
    );
    assert_eq!(shared.lane_admission.profile_inflight_count("main"), 0);
}

#[test]
fn profile_inflight_guard_drop_records_underflow_and_log_marker() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .build();
    let shared = harness.shared();
    let context = runtime_route_kind_inflight_context(RuntimeRouteKind::Responses);

    let guard = acquire_runtime_profile_inflight_guard(shared, "main", context)
        .expect("profile inflight guard should be acquired");
    shared.lane_admission.set_profile_inflight("main", 0);

    drop(guard);
    runtime_proxy_flush_logs_for_path(&shared.log_path).expect("runtime log should flush");

    assert_eq!(shared.lane_admission.profile_inflight_releases_total(), 1);
    assert_eq!(
        shared
            .lane_admission
            .profile_inflight_release_underflows_total(),
        1
    );
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("profile_inflight_underflow"),
        "profile inflight underflow marker should be logged: {log}"
    );
    assert!(
        log.contains("context=responses_http"),
        "profile inflight underflow marker should include context: {log}"
    );
}

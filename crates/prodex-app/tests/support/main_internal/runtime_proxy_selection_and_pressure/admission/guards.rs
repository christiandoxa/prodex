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
            .responses_active
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        shared
            .lane_admission
            .responses_admissions_total
            .load(Ordering::SeqCst),
        1
    );

    drop(guard);

    assert_eq!(shared.active_request_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        shared
            .lane_admission
            .responses_active
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(
        shared
            .lane_admission
            .responses_releases_total
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        shared
            .lane_admission
            .active_request_release_underflows_total
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(
        shared
            .lane_admission
            .responses_release_underflows_total
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
        .responses_active
        .store(0, Ordering::SeqCst);

    drop(guard);

    assert_eq!(shared.active_request_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        shared
            .lane_admission
            .responses_active
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(
        shared
            .lane_admission
            .responses_releases_total
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        shared
            .lane_admission
            .active_request_release_underflows_total
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        shared
            .lane_admission
            .responses_release_underflows_total
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
        .responses_active
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
        Err(RuntimeProxyAdmissionRejection::LaneLimit(RuntimeRouteKind::Responses))
    ));
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
        .responses_active
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
        shared.lane_admission.responses_active.load(Ordering::SeqCst),
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

    assert_eq!(
        shared
            .lane_admission
            .profile_inflight_admissions_total
            .load(Ordering::SeqCst),
        1
    );
    {
        let runtime = shared.runtime.lock().expect("runtime state should lock");
        assert_eq!(runtime.profile_inflight.get("main"), Some(&weight));
    }

    drop(guard);

    assert_eq!(
        shared
            .lane_admission
            .profile_inflight_releases_total
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        shared
            .lane_admission
            .profile_inflight_release_underflows_total
            .load(Ordering::SeqCst),
        0
    );
    let runtime = shared.runtime.lock().expect("runtime state should lock");
    assert!(!runtime.profile_inflight.contains_key("main"));
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
    {
        let mut runtime = shared.runtime.lock().expect("runtime state should lock");
        runtime.profile_inflight.clear();
    }

    drop(guard);
    runtime_proxy_flush_logs_for_path(&shared.log_path);

    assert_eq!(
        shared
            .lane_admission
            .profile_inflight_releases_total
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        shared
            .lane_admission
            .profile_inflight_release_underflows_total
            .load(Ordering::SeqCst),
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

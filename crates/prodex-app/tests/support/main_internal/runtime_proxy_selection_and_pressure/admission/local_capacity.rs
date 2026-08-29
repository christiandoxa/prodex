use super::helpers::*;
use super::*;

#[test]
fn responses_wait_past_old_admission_window_for_healthy_saturated_profile() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let hard_limit = runtime_proxy_profile_inflight_hard_limit();
    let ready = runtime_usage_snapshot(
        quota_window_ready(100, 3_600),
        quota_window_ready(83, 86_400),
    );
    let mut harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile(
            "exhausted",
            "exhausted-account",
            Some("exhausted@example.com"),
        )
        .openai_profile("ready", "second-account", Some("ready@example.com"))
        .active_profile("exhausted")
        .current_profile("exhausted")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot(
            "exhausted",
            runtime_usage_snapshot(quota_window_exhausted(300), quota_window_exhausted(300)),
        )
        .profile_usage_snapshot("ready", ready)
        .build();
    Arc::make_mut(&mut harness.shared_mut().runtime_config).response_chain_trace = true;
    let mut inflight_guards = (0..(hard_limit / runtime_profile_inflight_weight("responses_http")))
        .map(|_| {
            acquire_runtime_profile_inflight_guard(harness.shared(), "ready", "responses_http")
        })
        .collect::<Result<Vec<_>>>()
        .expect("ready profile should be fully saturated");
    let released_guard = inflight_guards
        .pop()
        .expect("one permit should be releasable");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(900));
        drop(released_guard);
    });

    let response = proxy_runtime_responses_request(
        900,
        &RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: br#"{"input":[]}"#.to_vec(),
        },
        harness.shared(),
    )
    .expect("healthy capacity should recover after a long local wait");
    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered response");
    };
    assert_eq!(
        parts.status,
        200,
        "unexpected saturated-profile recovery response: {}",
        String::from_utf8_lossy(&parts.body)
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(!log.contains("local_capacity_wait_timeout"));
    assert!(
        log.contains("request=900") && log.contains("rotation_generation=0"),
        "local saturation must not consume a provider attempt: {log}"
    );

    release.join().expect("permit release should finish");
    drop(inflight_guards);
}

#[test]
fn responses_wait_for_any_saturated_profile_and_reselect_after_release() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let hard_limit = runtime_proxy_profile_inflight_hard_limit();
    let ready = runtime_usage_snapshot(
        quota_window_ready(100, 3_600),
        quota_window_ready(83, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile(
            "exhausted",
            "exhausted-account",
            Some("exhausted@example.com"),
        )
        .openai_profile("busy-a", "second-account", Some("busy-a@example.com"))
        .openai_profile("busy-b", "third-account", Some("busy-b@example.com"))
        .active_profile("exhausted")
        .current_profile("exhausted")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot(
            "exhausted",
            runtime_usage_snapshot(quota_window_exhausted(300), quota_window_exhausted(300)),
        )
        .profile_usage_snapshot("busy-a", ready.clone())
        .profile_usage_snapshot("busy-b", ready)
        .build();
    let guard_count = hard_limit / runtime_profile_inflight_weight("responses_http");
    let busy_a_guards = (0..guard_count)
        .map(|_| {
            acquire_runtime_profile_inflight_guard(harness.shared(), "busy-a", "responses_http")
        })
        .collect::<Result<Vec<_>>>()
        .expect("first busy profile should be saturated");
    let mut busy_b_guards = (0..guard_count)
        .map(|_| {
            acquire_runtime_profile_inflight_guard(harness.shared(), "busy-b", "responses_http")
        })
        .collect::<Result<Vec<_>>>()
        .expect("second busy profile should be saturated");
    let released_guard = busy_b_guards
        .pop()
        .expect("one second-profile permit should be releasable");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(900));
        drop(released_guard);
    });

    let response = proxy_runtime_responses_request(
        901,
        &RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: br#"{"input":[]}"#.to_vec(),
        },
        harness.shared(),
    )
    .expect("one released eligible profile should continue the request");
    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered response");
    };
    assert_eq!(
        parts.status,
        200,
        "unexpected multi-profile saturation response: {}",
        String::from_utf8_lossy(&parts.body)
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["third-account".to_string()]
    );

    release.join().expect("permit release should finish");
    drop(busy_a_guards);
    drop(busy_b_guards);
}

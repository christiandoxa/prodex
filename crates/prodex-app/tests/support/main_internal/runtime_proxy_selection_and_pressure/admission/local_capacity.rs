use super::helpers::*;
use super::*;

#[test]
fn responses_rotate_around_saturated_current_when_another_profile_is_ready() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let hard_limit = runtime_proxy_profile_inflight_hard_limit();
    let main_ready = runtime_usage_snapshot(
        quota_window_ready(98, 3_600),
        quota_window_ready(98, 86_400),
    );
    let second_ready =
        runtime_usage_snapshot(quota_window_ready(9, 3_600), quota_window_ready(86, 86_400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", main_ready)
        .profile_usage_snapshot("second", second_ready)
        .build();
    harness
        .shared()
        .lane_admission
        .set_profile_inflight("main", hard_limit);

    let response = proxy_runtime_responses_request(
        899,
        &RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: br#"{"input":[]}"#.to_vec(),
        },
        harness.shared(),
    )
    .expect("fresh responses request should rotate around a saturated current profile");
    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered response");
    };

    assert_eq!(
        parts.status,
        200,
        "saturated current profile must not surface local 503 while another profile is ready: {}",
        String::from_utf8_lossy(&parts.body)
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()],
        "the saturated current profile must be skipped before upstream dispatch"
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("selection_skip_current")
            && log.contains("profile=main")
            && log.contains("reason=profile_inflight_saturated"),
        "selection must explain the saturated-current rotation: {log}"
    );
    assert!(
        !log.contains("local_capacity_wait_timeout"),
        "a ready alternative must prevent local capacity timeout: {log}"
    );
    assert!(
        !log.contains("inflight_wait_started"),
        "a ready alternative should be selected immediately instead of waiting on the busy current profile: {log}"
    );
}

#[test]
fn responses_keep_waiting_after_capacity_epoch_while_quota_remains_positive() {
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let hard_limit = runtime_proxy_profile_inflight_hard_limit();
    let ready =
        runtime_usage_snapshot(quota_window_ready(7, 3_600), quota_window_ready(81, 86_400));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready)
        .build();
    let mut inflight_guards = (0..(hard_limit / runtime_profile_inflight_weight("responses_http")))
        .map(|_| acquire_runtime_profile_inflight_guard(harness.shared(), "main", "responses_http"))
        .collect::<Result<Vec<_>>>()
        .expect("positive-quota profile should be fully saturated");
    let released_guard = inflight_guards
        .pop()
        .expect("one permit should be releasable");
    let release = thread::spawn(move || {
        // Test builds use a 1.5s capacity epoch. Releasing after more than two old
        // epochs proves saturation is pure backpressure rather than a request deadline.
        thread::sleep(Duration::from_millis(3_200));
        drop(released_guard);
    });

    let response = proxy_runtime_responses_request(
        898,
        &RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: br#"{"input":[]}"#.to_vec(),
        },
        harness.shared(),
    )
    .expect("positive-quota profile should survive the first capacity wait epoch");
    let (status, body) = match response {
        RuntimeResponsesReply::Buffered(parts) => (
            parts.status,
            String::from_utf8_lossy(&parts.body).into_owned(),
        ),
        RuntimeResponsesReply::Streaming(mut response) => {
            let mut body = String::new();
            response
                .body
                .read_to_string(&mut body)
                .expect("recovered streaming response should be readable");
            (response.status, body)
        }
    };

    assert_eq!(
        status, 200,
        "local capacity timeout must stay internal while usable quota remains: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string()]
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("local_capacity_wait_continued") && log.contains("mode=backpressure"),
        "the request must keep waiting across old capacity timeout boundaries: {log}"
    );
    assert!(
        !log.contains("local_capacity_wait_timeout")
            && !body.contains("local_capacity_timeout")
            && !body.contains("local capacity remained saturated"),
        "positive quota must never surface a local capacity timeout response: {log}"
    );

    release.join().expect("permit release should finish");
    drop(inflight_guards);
}

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

#[test]
fn standard_fresh_request_rotates_around_saturated_current_profile() {
    let backend = RuntimeProxyBackend::start();
    let hard_limit = runtime_proxy_profile_inflight_hard_limit();
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    harness
        .shared()
        .lane_admission
        .set_profile_inflight("main", hard_limit);
    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/status".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let response = proxy_runtime_standard_request(13, &request, harness.shared())
        .expect("fresh standard request should rotate around saturated current profile");
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(
        status, 200,
        "saturated current profile must not surface local 503 while another profile is ready: {body}"
    );
    assert!(
        body.contains("second-account"),
        "ready alternative should serve the request: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()],
        "the saturated current profile must be skipped before upstream dispatch"
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("profile_inflight_saturated") && log.contains("profile=main"),
        "standard selection must record the saturated current profile: {log}"
    );
    assert!(
        !log.contains("local_capacity_wait_timeout"),
        "a ready alternative must prevent local capacity timeout: {log}"
    );
    assert!(
        !log.contains("inflight_wait_started"),
        "a ready alternative should be selected immediately instead of waiting on the busy current profile: {log}"
    );
}

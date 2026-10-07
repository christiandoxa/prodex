use super::helpers::quota_window_exhausted;
use super::{
    Duration, ResponseProfileBinding, RuntimeProxyBackend, RuntimeProxyBackendFaultRoute,
    RuntimeProxyBackendFaultScript, RuntimeProxyBackendFaultStep, RuntimeProxyMarkerGuard,
    RuntimeProxyProfileHarness, RuntimeProxyProfileHarnessBuilder, RuntimeProxyRequest,
    RuntimeRouteKind, TestEnvVarGuard, closed_loopback_backend_base_url,
    proxy_runtime_standard_request, quota_window_ready, read_runtime_proxy_test_log,
    register_runtime_proxy_persistence_mode, runtime_has_route_eligible_quota_fallback_for_model,
    runtime_profile_route_circuit_key, runtime_usage_snapshot,
    test_runtime_compact_quota_fallback_exhausted, tiny_http_response_status_and_body,
};
use chrono::Local;
use std::collections::BTreeSet;
use std::time::Instant;

fn two_ready_profiles(backend: &RuntimeProxyBackend) -> RuntimeProxyProfileHarness {
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build()
}

#[test]
fn compact_quota_fallback_respects_request_local_exclusions() {
    let backend = RuntimeProxyBackend::start();
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
    let excluded = BTreeSet::from(["second".to_string()]);

    assert!(
        runtime_has_route_eligible_quota_fallback_for_model(
            harness.shared(),
            "main",
            &BTreeSet::new(),
            RuntimeRouteKind::Compact,
            None,
        )
        .expect("global quota fallback lookup should succeed"),
        "second profile should be globally eligible before request-local exclusion"
    );
    assert!(
        test_runtime_compact_quota_fallback_exhausted(harness.shared(), "main", &excluded, None,)
            .expect("request-local quota fallback decision should succeed"),
        "a profile already quota-failed in this request must make the compact fallback exhausted"
    );
}

fn compact_request(session_id: Option<&str>) -> RuntimeProxyRequest {
    let mut headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("x-openai-subagent".to_string(), "compact".to_string()),
    ];
    if let Some(session_id) = session_id {
        headers.push(("session_id".to_string(), session_id.to_string()));
    }
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers,
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    }
}

fn compact_request_with_turn_state(turn_state: &str) -> RuntimeProxyRequest {
    let mut request = compact_request(None);
    request
        .headers
        .push(("x-codex-turn-state".to_string(), turn_state.to_string()));
    request
}

fn bind_session(harness: &RuntimeProxyProfileHarness, session_id: &str, profile_name: &str) {
    harness
        .shared()
        .runtime
        .lock()
        .unwrap()
        .session_id_bindings
        .insert(
            session_id.to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: profile_name.to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
}

fn bind_response(harness: &RuntimeProxyProfileHarness, response_id: &str, profile_name: &str) {
    harness
        .shared()
        .runtime
        .lock()
        .unwrap()
        .state
        .response_profile_bindings
        .insert(
            response_id.to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: profile_name.to_string(),
                bound_at: Local::now().timestamp(),
            },
        );
}

fn compact_request_with_previous_response(previous_response_id: &str) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-openai-subagent".to_string(), "compact".to_string()),
        ],
        body: format!(
            r#"{{"input":[],"instructions":"compact","previous_response_id":"{previous_response_id}"}}"#
        )
        .into_bytes(),
    }
}

#[test]
fn fresh_noncompact_transport_failure_rotates_through_ready_profiles() {
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(closed_loopback_backend_base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/status".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let response = proxy_runtime_standard_request(54, &request, harness.shared())
        .expect("fresh transport failures should remain inside the rotation loop");
    let (status, _) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);

    assert_eq!(status, 503, "{log}");
    assert!(
        log.contains("standard_transport_failure profile=main")
            && log.contains("standard_transport_failure profile=second"),
        "every ready profile should receive a bounded precommit attempt: {log}"
    );
}

#[test]
fn fresh_noncompact_cold_start_probe_wait_is_one_shot() {
    let backend = RuntimeProxyBackend::start();
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot(
            "main",
            runtime_usage_snapshot(
                quota_window_exhausted(3_600),
                quota_window_ready(80, 86_400),
            ),
        )
        .build();
    let shared = harness.shared();
    let circuit_key = runtime_profile_route_circuit_key("second", RuntimeRouteKind::Standard);
    shared
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_route_circuit_open_until
        .insert(circuit_key.clone(), Local::now().timestamp() + 60);
    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/status".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let response = std::thread::scope(|scope| {
        let release = scope.spawn(|| {
            // Hold the circuit until the real recovery phase is observable;
            // crossing a wall-clock second must not skip the path under test.
            let deadline = Instant::now() + Duration::from_secs(5);
            let saw_wait = loop {
                let log = std::fs::read_to_string(&shared.log_path).unwrap_or_default();
                if log.lines().any(|line| {
                    line.contains("rotation_waiting_for_recovery")
                        && line.contains("route=standard")
                }) {
                    break true;
                }
                if Instant::now() >= deadline {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            shared
                .runtime
                .lock()
                .unwrap()
                .profile_route_circuit_open_until
                .remove(&circuit_key);
            shared.lane_admission.notify_selection_change();
            saw_wait
        });
        let response = proxy_runtime_standard_request(86, &request, shared)
            .expect("cold-start recovery should reselect the profile after its circuit clears");
        assert!(
            release.join().unwrap(),
            "the real recovery wait must be exercised"
        );
        response
    });
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 200, "{body}\n{log}");
    assert!(
        body.contains("second-account"),
        "recovered cold-start profile should serve the request: {body}"
    );
    let recovery_waits = log
        .lines()
        .filter(|line| {
            line.contains("rotation_waiting_for_recovery") && line.contains("route=standard")
        })
        .count();
    assert_eq!(
        recovery_waits, 1,
        "temporarily unavailable cold-start profile should require exactly one recovery wait: {log}"
    );
    assert!(
        !log.contains("precommit_budget_exhausted"),
        "successful recovery must not leak or record terminal retry-budget exhaustion: {log}"
    );
}

#[test]
fn session_affinity_prefers_bound_profile_for_compact_requests() {
    let backend = RuntimeProxyBackend::start_http_compact_overloaded();
    let harness = two_ready_profiles(&backend);
    bind_session(&harness, "sess-second", "second");

    proxy_runtime_standard_request(1, &compact_request(Some("sess-second")), harness.shared())
        .expect("session-bound compact request should succeed");

    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()]
    );
}

#[test]
fn previous_response_affinity_bypasses_compact_profile_inflight_limit() {
    let backend = RuntimeProxyBackend::start();
    let harness = two_ready_profiles(&backend);
    bind_response(&harness, "resp-main", "main");
    let hard_limit = harness
        .shared()
        .runtime_config
        .tuning
        .profile_inflight_hard_limit;
    harness
        .shared()
        .lane_admission
        .set_profile_inflight("main", hard_limit);

    proxy_runtime_standard_request(
        50,
        &compact_request_with_previous_response("resp-main"),
        harness.shared(),
    )
    .expect("previous-response-bound compact request should succeed");

    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string()]
    );
    let bound_profile = harness
        .shared()
        .runtime
        .lock()
        .unwrap()
        .state
        .response_profile_bindings
        .get("resp-main")
        .map(|binding| binding.profile_name.clone());
    assert_eq!(bound_profile.as_deref(), Some("main"));
}

#[test]
fn previous_response_affined_compact_quota_requests_full_context_replay() {
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let harness = two_ready_profiles(&backend);
    bind_response(&harness, "resp-main", "main");
    let shared = harness.shared();

    let response = proxy_runtime_standard_request(
        51,
        &compact_request_with_previous_response("resp-main"),
        shared,
    )
    .expect("previous-response-affined compact quota should request full-context replay");
    let (status, body) = tiny_http_response_status_and_body(response);
    let accounts = backend.responses_accounts();
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 400, "{body}\n{log}");
    assert!(body.contains("previous_response_not_found"), "{body}");
    assert!(!body.contains("usage limit"), "{body}");
    assert!(!body.contains("service_unavailable"), "{body}");
    assert_eq!(accounts, vec!["main-account".to_string()]);
    assert!(
        log.contains("compact_full_context_retry_signal profile=main reason=compact_quota"),
        "{log}"
    );
    let bound_profile = shared
        .runtime
        .lock()
        .unwrap()
        .state
        .response_profile_bindings
        .get("resp-main")
        .map(|binding| binding.profile_name.clone());
    assert_eq!(bound_profile, None, "replay signal must release exhausted owner");

    let replay = proxy_runtime_standard_request(57, &compact_request(None), shared)
        .expect("full-context compact replay should continue on the healthy profile");
    let (replay_status, replay_body) = tiny_http_response_status_and_body(replay);
    assert_eq!(replay_status, 200, "{replay_body}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    let replay_log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(
        replay_log.contains("compact_committed profile=second"),
        "full-context replay must finish on the healthy profile: {replay_log}"
    );
}

#[test]
fn compact_transport_timeout_rotates_fresh_request_to_next_profile_once() {
    let _compact_timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS", "300");
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::stalled_json(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
                Duration::from_millis(400),
            ),
        ]));
    let harness = two_ready_profiles(&backend);
    let shared = harness.shared();
    let _marker_guard = RuntimeProxyMarkerGuard::new(&shared.log_path);
    register_runtime_proxy_persistence_mode(&shared.log_path, false);

    let response = proxy_runtime_standard_request(45, &compact_request(None), shared)
        .expect("fresh compact transport failure should rotate before returning");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(
        status, 200,
        "unexpected compact response body: {body}; log: {log}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert!(
        log.contains(
            "compact_transport_failure profile=main route=compact stage=compact_forward_response"
        ) && log.contains("profile_transport_backoff")
            && log.contains("route=compact")
            && log.contains("compact_committed profile=second"),
        "compact transport timeout should back off main and commit second: {log}"
    );
    assert_eq!(
        log.matches("compact_committed profile=").count(),
        1,
        "{log}"
    );
}

#[test]
fn compact_candidate_wait_without_selection_does_not_consume_an_attempt() {
    let backend = RuntimeProxyBackend::start();
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .build();
    let shared = harness.shared();
    let hard_limit = shared.runtime_config.tuning.profile_inflight_hard_limit;
    shared
        .lane_admission
        .set_profile_inflight("main", hard_limit);
    shared
        .lane_admission
        .set_profile_inflight("second", hard_limit);

    let lane_admission = shared.lane_admission.clone();
    let release = std::thread::spawn(move || {
        // Cross two historical test capacity epochs. Waiting itself must not
        // spend a provider attempt or surface a local 503.
        std::thread::sleep(Duration::from_millis(3_200));
        lane_admission.set_profile_inflight("second", hard_limit.saturating_sub(1));
        lane_admission.record_inflight_release();
    });

    let response = proxy_runtime_standard_request(53, &compact_request(None), shared)
        .expect("saturated compact selection should backpressure until capacity returns");
    let (status, body) = tiny_http_response_status_and_body(response);
    release.join().expect("capacity release should join");
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(
        status, 200,
        "local saturation must not surface as an error: {body}\n{log}"
    );
    assert_eq!(
        log.matches("transport=http compact_candidate=").count(),
        1,
        "waiting without a selected candidate must not consume an upstream attempt: {log}"
    );
    assert!(
        log.contains("local_capacity_wait_continued") && log.contains("mode=backpressure"),
        "compact request should remain queued across old capacity epochs: {log}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["second-account".to_string()],
        "only the profile released from saturation should receive the upstream attempt"
    );
}

#[test]
fn unbound_turn_state_compact_transport_failure_rotates_to_ready_profile() {
    let _compact_timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS", "300");
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::stalled_json(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
                Duration::from_millis(400),
            ),
        ]));
    let harness = two_ready_profiles(&backend);
    let shared = harness.shared();

    let response = proxy_runtime_standard_request(
        52,
        &compact_request_with_turn_state("turn-unbound"),
        shared,
    )
    .expect("turn-state compact transport failure should rotate transparently");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 200, "{body}\n{log}");
    assert!(!body.contains("service_unavailable"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert!(
        log.contains("compact_affinity_recovered profile=main reason=compact_transport")
            && log.contains("compact_committed profile=second"),
        "{log}"
    );
}

#[test]
fn session_affined_compact_transport_failure_rotates_to_ready_profile() {
    let _compact_timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS", "300");
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::stalled_json(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
                Duration::from_millis(400),
            ),
        ]));
    let harness = two_ready_profiles(&backend);
    bind_session(&harness, "sess-main", "main");
    let shared = harness.shared();

    let response = proxy_runtime_standard_request(46, &compact_request(Some("sess-main")), shared)
        .expect("session-affined compact transport failure should rotate transparently");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 200, "{body}\n{log}");
    assert!(!body.contains("service_unavailable"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert!(
        log.contains("compact_affinity_recovered profile=main reason=compact_transport")
            && log.contains("compact_committed profile=second"),
        "{log}"
    );
    assert_eq!(
        shared
            .runtime
            .lock()
            .unwrap()
            .state
            .session_profile_bindings
            .get("sess-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
}

#[test]
fn session_affined_compact_overload_rotates_to_ready_profile() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::overloaded_503(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
            RuntimeProxyBackendFaultStep::overloaded_503(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
        ]));
    let harness = two_ready_profiles(&backend);
    bind_session(&harness, "sess-main", "main");
    let shared = harness.shared();

    let response = proxy_runtime_standard_request(49, &compact_request(Some("sess-main")), shared)
        .expect("session-affined compact overload should rotate transparently");
    let (status, body) = tiny_http_response_status_and_body(response);
    let accounts = backend.responses_accounts();
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 200, "{body}\n{log}");
    assert!(!body.contains("service_unavailable"), "{body}");
    assert_eq!(
        accounts,
        vec![
            "main-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string()
        ]
    );
    assert!(
        log.contains("compact_affinity_recovered profile=main reason=compact_overload")
            && log.contains("compact_committed profile=second"),
        "{log}"
    );
}

#[test]
fn session_affined_compact_auth_failure_rotates_to_ready_profile() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::unauthorized(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
        ]));
    let harness = two_ready_profiles(&backend);
    bind_session(&harness, "sess-main", "main");
    let shared = harness.shared();

    let response = proxy_runtime_standard_request(47, &compact_request(Some("sess-main")), shared)
        .expect("session-affined compact auth failure should rotate transparently");
    let (status, body) = tiny_http_response_status_and_body(response);
    let accounts = backend.responses_accounts();
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 200, "{body}\n{log}");
    assert!(!body.contains("service_unavailable"), "{body}");
    assert_eq!(
        accounts,
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert!(
        log.contains("compact_affinity_recovered profile=main reason=compact_auth_failure")
            && log.contains("compact_committed profile=second"),
        "{log}"
    );
}

#[test]
fn session_affined_compact_rate_limit_rotates_to_ready_profile() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::rate_limited_429(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
        ]));
    let harness = two_ready_profiles(&backend);
    bind_session(&harness, "sess-main", "main");
    let shared = harness.shared();

    let response = proxy_runtime_standard_request(55, &compact_request(Some("sess-main")), shared)
        .expect("session-affined compact rate limit should rotate transparently");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 200, "{body}\n{log}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert!(
        log.contains("compact_affinity_recovered profile=main reason=compact_rate_limit")
            && log.contains("compact_committed profile=second"),
        "{log}"
    );
}

#[test]
fn session_affined_compact_profile_unavailable_rotates_to_ready_profile() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::profile_unavailable(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
        ]));
    let harness = two_ready_profiles(&backend);
    bind_session(&harness, "sess-main", "main");
    let shared = harness.shared();

    let response = proxy_runtime_standard_request(56, &compact_request(Some("sess-main")), shared)
        .expect("session-affined unavailable compact owner should rotate transparently");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 200, "{body}\n{log}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert!(
        log.contains("compact_affinity_recovered profile=main reason=compact_profile_unavailable")
            && log.contains("compact_committed profile=second"),
        "{log}"
    );
}


fn run_previous_response_compact_retryable_replay_case(
    faults: Vec<RuntimeProxyBackendFaultStep>,
    request_id: u64,
    reason: &str,
) {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new(faults));
    let harness = two_ready_profiles(&backend);
    bind_response(&harness, "resp-main", "main");
    let shared = harness.shared();

    let response = proxy_runtime_standard_request(
        request_id,
        &compact_request_with_previous_response("resp-main"),
        shared,
    )
    .expect("retryable previous-response compact failure should request full-context replay");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 400, "{body}\n{log}");
    assert!(body.contains("previous_response_not_found"), "{body}");
    assert!(!body.contains("service_unavailable"), "{body}");
    assert!(
        log.contains(&format!(
            "compact_full_context_retry_signal profile=main reason={reason}"
        )),
        "{log}"
    );
    assert!(
        !shared
            .runtime
            .lock()
            .unwrap()
            .state
            .response_profile_bindings
            .contains_key("resp-main"),
        "replay signal must release the failed previous-response owner"
    );
    let before_replay = backend.responses_accounts();
    assert!(
        !before_replay.is_empty() && before_replay.iter().all(|account| account == "main-account"),
        "hard-affinity owner must be the only upstream before replay: {before_replay:?}"
    );

    let replay = proxy_runtime_standard_request(request_id + 1, &compact_request(None), shared)
        .expect("full-context compact replay should finish on a healthy profile");
    let (replay_status, replay_body) = tiny_http_response_status_and_body(replay);
    assert_eq!(replay_status, 200, "{replay_body}");
    let accounts = backend.responses_accounts();
    assert_eq!(accounts.last().map(String::as_str), Some("second-account"), "{accounts:?}");
    assert!(
        read_runtime_proxy_test_log(&shared.log_path).contains("compact_committed profile=second"),
        "full-context replay must commit the healthy profile"
    );
}

#[test]
fn previous_response_affined_compact_overload_requests_full_context_replay() {
    run_previous_response_compact_retryable_replay_case(
        vec![
            RuntimeProxyBackendFaultStep::overloaded_503(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
            RuntimeProxyBackendFaultStep::overloaded_503(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
        ],
        58,
        "compact_overload",
    );
}

#[test]
fn previous_response_affined_compact_auth_failure_requests_full_context_replay() {
    run_previous_response_compact_retryable_replay_case(
        vec![RuntimeProxyBackendFaultStep::unauthorized(
            RuntimeProxyBackendFaultRoute::Compact,
            "main-account",
        )],
        60,
        "compact_auth_failure",
    );
}

#[test]
fn previous_response_affined_compact_rate_limit_requests_full_context_replay() {
    run_previous_response_compact_retryable_replay_case(
        vec![RuntimeProxyBackendFaultStep::rate_limited_429(
            RuntimeProxyBackendFaultRoute::Compact,
            "main-account",
        )],
        62,
        "compact_rate_limit",
    );
}

#[test]
fn previous_response_affined_compact_profile_unavailable_requests_full_context_replay() {
    run_previous_response_compact_retryable_replay_case(
        vec![RuntimeProxyBackendFaultStep::profile_unavailable(
            RuntimeProxyBackendFaultRoute::Compact,
            "main-account",
        )],
        64,
        "compact_profile_unavailable",
    );
}

#[test]
fn previous_response_affined_compact_transport_failure_requests_full_context_replay() {
    let _compact_timeout_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS", "300");
    run_previous_response_compact_retryable_replay_case(
        vec![RuntimeProxyBackendFaultStep::stalled_json(
            RuntimeProxyBackendFaultRoute::Compact,
            "main-account",
            Duration::from_millis(400),
        )],
        66,
        "compact_transport",
    );
}

#[test]
fn generic_compact_429_rotates_before_commit() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::plain_429(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
        ]));
    let harness = two_ready_profiles(&backend);
    let shared = harness.shared();

    let response = proxy_runtime_standard_request(48, &compact_request(None), shared)
        .expect("generic compact 429 should rotate before commit");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 200, "{body}\n{log}");
    assert!(!body.contains("Too Many Requests"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    assert!(
        log.contains("compact_rate_limited profile=main")
            && log.contains("compact_committed profile=second"),
        "{log}"
    );
}

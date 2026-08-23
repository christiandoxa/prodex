use super::{
    Duration, ResponseProfileBinding, RuntimeProxyBackend, RuntimeProxyBackendFaultRoute,
    RuntimeProxyBackendFaultScript, RuntimeProxyBackendFaultStep, RuntimeProxyProfileHarness,
    RuntimeProxyMarkerGuard, RuntimeProxyProfileHarnessBuilder, RuntimeProxyRequest,
    TestEnvVarGuard, closed_loopback_backend_base_url, proxy_runtime_standard_request,
    quota_window_ready, read_runtime_proxy_test_log, register_runtime_proxy_persistence_mode,
    runtime_usage_snapshot,
    tiny_http_response_status_and_body,
};
use chrono::Local;

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
    request.headers.push((
        "x-codex-turn-state".to_string(),
        turn_state.to_string(),
    ));
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
    let hard_limit = harness.shared().runtime_config.tuning.profile_inflight_hard_limit;
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
fn previous_response_affined_compact_quota_does_not_rotate_or_release_binding() {
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let harness = two_ready_profiles(&backend);
    bind_response(&harness, "resp-main", "main");
    let shared = harness.shared();

    let response = proxy_runtime_standard_request(
        51,
        &compact_request_with_previous_response("resp-main"),
        shared,
    )
    .expect("previous-response-affined compact quota should pass through");
    let (status, _) = tiny_http_response_status_and_body(response);
    let accounts = backend.responses_accounts();
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 429, "{log}");
    assert_eq!(accounts, vec!["main-account".to_string()]);
    assert!(
        log.contains("compact_final_failure exit=hard_affinity_retryable_failure reason=quota"),
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
    assert_eq!(bound_profile.as_deref(), Some("main"));
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

    let response = proxy_runtime_standard_request(53, &compact_request(None), shared)
        .expect("saturated compact selection should fail locally");
    let (status, _) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 503, "{log}");
    let selected_candidates = log.matches("transport=http compact_candidate=").count();
    assert!(
        log.contains("compact_final_failure exit=candidate_exhausted")
            && log.contains(&format!("attempts={selected_candidates} ")),
        "only the selected saturated candidate may spend an attempt: {log}"
    );
    assert!(backend.responses_accounts().is_empty(), "{log}");
}

#[test]
fn unbound_turn_state_compact_transport_failure_does_not_rotate() {
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
    .expect("unbound turn-state transport failure should fail closed");
    let (status, _) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 503, "{log}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string()]
    );
    assert!(
        log.contains("compact_final_failure exit=hard_affinity_transport_failure"),
        "{log}"
    );
}

#[test]
fn session_affined_compact_transport_failure_does_not_rotate() {
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
        .expect("session-affined compact transport failure should return locally");
    let (status, _) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 503, "{log}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string()]
    );
    assert!(
        log.contains("compact_final_failure exit=hard_affinity_transport_failure"),
        "{log}"
    );
    assert!(!log.contains("compact_committed profile=second"), "{log}");
}

#[test]
fn session_affined_compact_overload_does_not_rotate() {
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
        .expect("session-affined compact overload should pass through");
    let (status, _) = tiny_http_response_status_and_body(response);
    let accounts = backend.responses_accounts();
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 503, "{log}");
    assert_eq!(
        accounts,
        vec!["main-account".to_string(), "main-account".to_string()]
    );
    assert!(
        log.contains(
            "compact_final_failure exit=hard_affinity_retryable_failure reason=overload"
        ),
        "{log}"
    );
    assert!(!log.contains("compact_committed profile=second"), "{log}");
}

#[test]
fn session_affined_compact_auth_failure_does_not_rotate() {
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
        .expect("session-affined compact auth failure should pass through");
    let (status, _) = tiny_http_response_status_and_body(response);
    let accounts = backend.responses_accounts();
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 401, "{log}");
    assert!(!accounts.is_empty(), "{log}");
    assert!(
        accounts.iter().all(|account| account == "main-account"),
        "{accounts:?}"
    );
    assert!(
        log.contains("compact_final_failure exit=hard_affinity_auth_failure"),
        "{log}"
    );
    assert!(!log.contains("compact_committed profile=second"), "{log}");
}

#[test]
fn generic_compact_429_passes_through_without_rotation() {
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
        .expect("generic compact 429 should pass through");
    let (status, body) = tiny_http_response_status_and_body(response);
    let log = read_runtime_proxy_test_log(&shared.log_path);

    assert_eq!(status, 429, "{log}");
    assert_eq!(body, "Too Many Requests");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string()]
    );
    assert_eq!(
        log.matches("compact_committed profile=").count(),
        1,
        "{log}"
    );
}

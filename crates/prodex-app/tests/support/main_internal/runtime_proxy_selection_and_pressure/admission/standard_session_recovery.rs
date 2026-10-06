use super::helpers::*;
use super::*;

fn bind_standard_session(harness: &RuntimeProxyProfileHarness, session_id: &str) {
    let binding = ResponseProfileBinding {
        binding_identity: None,
        profile_name: "main".to_string(),
        bound_at: Local::now().timestamp(),
    };
    let mut runtime = harness.shared().runtime.lock().expect("runtime lock");
    runtime
        .session_id_bindings
        .insert(session_id.to_string(), binding.clone());
    runtime
        .state
        .session_profile_bindings
        .insert(session_id.to_string(), binding);
}

fn run_standard_session_recovery_case(
    fault: RuntimeProxyBackendFaultStep,
    request_id: u64,
    release_marker: &str,
) {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([fault]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("second")
        .current_profile("second")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    bind_standard_session(&harness, "sess-bound");

    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/status".to_string(),
        headers: vec![("session_id".to_string(), "sess-bound".to_string())],
        body: Vec::new(),
    };
    let response = proxy_runtime_standard_request(request_id, &request, harness.shared())
        .expect("retryable session-owner failure should rotate transparently");
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(
        status, 200,
        "retryable owner failure leaked to user: {body}"
    );
    assert!(body.contains("second-account"), "{body}");
    assert!(!body.contains("service_unavailable"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()],
        "bound owner must fail first, then the healthy profile must finish the request"
    );

    let runtime = harness.shared().runtime.lock().expect("runtime lock");
    assert_eq!(
        runtime
            .session_id_bindings
            .get("sess-bound")
            .map(|binding| binding.profile_name.as_str()),
        Some("second"),
        "successful retry must rebind the live session to the healthy profile"
    );
    assert_eq!(
        runtime
            .state
            .session_profile_bindings
            .get("sess-bound")
            .map(|binding| binding.profile_name.as_str()),
        Some("second"),
        "persisted session affinity must follow the recovered profile"
    );
    drop(runtime);

    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains(release_marker) && log.contains("session_id=Some(\"sess-bound\")"),
        "failed owner affinity must be released before retry: {log}"
    );
}

#[test]
fn standard_session_quota_block_rotates_to_ready_profile() {
    run_standard_session_recovery_case(
        RuntimeProxyBackendFaultStep::usage_limit_429(
            RuntimeProxyBackendFaultRoute::Status,
            "main-account",
        ),
        171,
        "quota_release_affinity",
    );
}

#[test]
fn standard_session_rate_limit_rotates_to_ready_profile() {
    run_standard_session_recovery_case(
        RuntimeProxyBackendFaultStep::rate_limited_429(
            RuntimeProxyBackendFaultRoute::Status,
            "main-account",
        ),
        172,
        "standard_rate_limit_release_affinity",
    );
}

#[test]
fn standard_session_generic_429_rotates_to_ready_profile() {
    run_standard_session_recovery_case(
        RuntimeProxyBackendFaultStep::plain_429(
            RuntimeProxyBackendFaultRoute::Status,
            "main-account",
        ),
        176,
        "standard_rate_limit_release_affinity",
    );
}

#[test]
fn standard_session_overload_rotates_to_ready_profile() {
    run_standard_session_recovery_case(
        RuntimeProxyBackendFaultStep::overloaded_503(
            RuntimeProxyBackendFaultRoute::Status,
            "main-account",
        ),
        173,
        "standard_overload_release_affinity",
    );
}

#[test]
fn standard_session_profile_unavailable_rotates_to_ready_profile() {
    run_standard_session_recovery_case(
        RuntimeProxyBackendFaultStep::profile_unavailable(
            RuntimeProxyBackendFaultRoute::Status,
            "main-account",
        ),
        174,
        "standard_profile_unavailable_release_affinity",
    );
}

#[test]
fn standard_session_auth_failure_rotates_to_ready_profile() {
    run_standard_session_recovery_case(
        RuntimeProxyBackendFaultStep::unauthorized(
            RuntimeProxyBackendFaultRoute::Status,
            "main-account",
        ),
        175,
        "auth_failed_release_affinity",
    );
}

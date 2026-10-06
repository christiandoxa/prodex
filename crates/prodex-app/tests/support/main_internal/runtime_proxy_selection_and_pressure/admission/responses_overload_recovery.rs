use super::helpers::*;
use super::*;
use std::io::Read;

#[test]
fn fresh_responses_keep_recovering_after_multiple_provider_overload_sweeps() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::sse_overloaded(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
            RuntimeProxyBackendFaultStep::sse_overloaded(
                RuntimeProxyBackendFaultRoute::Responses,
                "second-account",
            ),
            RuntimeProxyBackendFaultStep::sse_success(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
                "recovered-after-outage",
            ),
        ]));
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
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":[]}"#.to_vec(),
    };

    let response = proxy_runtime_responses_request(903, &request, harness.shared())
        .expect("provider-wide overload should remain recoverable");
    let RuntimeResponsesReply::Streaming(mut response) = response else {
        panic!("recovered response should remain streaming");
    };
    let mut body = String::new();
    response
        .body
        .read_to_string(&mut body)
        .expect("recovered response should be readable");

    assert_eq!(response.status, 200, "{body}");
    assert!(!body.contains("server_is_overloaded"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        ["main-account", "second-account", "main-account"]
    );
}

#[test]
fn fresh_responses_keep_recovering_past_old_two_sweep_limit() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::sse_overloaded(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
            RuntimeProxyBackendFaultStep::sse_overloaded(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
            RuntimeProxyBackendFaultStep::sse_overloaded(
                RuntimeProxyBackendFaultRoute::Responses,
                "second-account",
            ),
            RuntimeProxyBackendFaultStep::sse_overloaded(
                RuntimeProxyBackendFaultRoute::Responses,
                "second-account",
            ),
            RuntimeProxyBackendFaultStep::sse_success(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
                "recovered-after-two-sweeps",
            ),
        ]));
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
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":[]}"#.to_vec(),
    };

    let response = proxy_runtime_responses_request(904, &request, harness.shared())
        .expect("retryable provider overload must not leak while quota-positive profiles remain");
    let RuntimeResponsesReply::Streaming(mut response) = response else {
        panic!("recovered response should remain streaming");
    };
    let mut body = String::new();
    response
        .body
        .read_to_string(&mut body)
        .expect("recovered response should be readable");

    assert_eq!(
        response.status, 200,
        "retryable provider 503 must remain internal until recovery: {body}"
    );
    assert!(!body.contains("server_is_overloaded"), "{body}");
    assert!(
        backend.responses_accounts().len() >= 5,
        "the request should survive more than the historical two recovery sweeps: {:?}",
        backend.responses_accounts()
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("sweep=2"),
        "the test must cross the old recovery sweep boundary: {log}"
    );
}

#[test]
fn fresh_responses_wait_for_rate_limit_pool_recovery_instead_of_leaking_429() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::rate_limited_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
            RuntimeProxyBackendFaultStep::rate_limited_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "second-account",
            ),
            RuntimeProxyBackendFaultStep::sse_success(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
                "recovered-after-rate-limit",
            ),
        ]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(15, 3_600),
        quota_window_ready(75, 86_400),
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
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":[]}"#.to_vec(),
    };

    let response = proxy_runtime_responses_request(905, &request, harness.shared())
        .expect("temporary rate limiting across the pool should remain recoverable");
    let RuntimeResponsesReply::Streaming(mut response) = response else {
        panic!("recovered response should remain streaming");
    };
    let mut body = String::new();
    response
        .body
        .read_to_string(&mut body)
        .expect("recovered response should be readable");

    assert_eq!(
        response.status, 200,
        "temporary upstream 429 must remain internal while quota-positive accounts remain: {body}"
    );
    assert!(!body.contains("rate_limit_exceeded"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        ["main-account", "second-account", "main-account"]
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("rotation_waiting_for_recovery") || log.contains("rotation_sweep_start"),
        "pool-wide temporary rate limiting should enter recovery instead of terminating: {log}"
    );
}

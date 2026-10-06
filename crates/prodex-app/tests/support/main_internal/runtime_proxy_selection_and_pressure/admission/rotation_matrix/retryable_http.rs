use super::*;

#[test]
fn fresh_responses_generic_429_rotates_before_commit() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::plain_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let harness = ready_profiles(&backend);

    let reply = proxy_runtime_responses_request(
        104,
        &responses_request(br#"{"input":[]}"#),
        harness.shared(),
    )
    .expect("generic 429 should rotate before commit");
    let (status, body, profile) = consume_responses_reply(reply);

    assert_eq!(status, 200, "{body}");
    assert!(!body.contains("Too Many Requests"), "{body}");
    assert_eq!(profile.as_deref(), Some("second"));
    assert_eq!(backend.responses_accounts(), ["main-account", "second-account"]);
}

#[test]
fn fresh_responses_rate_limit_rotates_without_overload_penalty() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::rate_limited_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let harness = ready_profiles(&backend);

    let reply = proxy_runtime_responses_request(
        114,
        &responses_request(br#"{"input":[]}"#),
        harness.shared(),
    )
    .expect("explicit rate limit should rotate to the next profile");
    let (status, body, profile) = consume_responses_reply(reply);

    assert_eq!(status, 200, "{body}");
    assert_eq!(profile.as_deref(), Some("second"));
    assert_eq!(
        backend.responses_accounts(),
        ["main-account", "second-account"]
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("rate_limited"),
        "rate-limit classification should be visible: {log}"
    );
    assert!(
        !log.contains("upstream_overloaded route=responses profile=main"),
        "rate limits must not receive overload health treatment: {log}"
    );
}

#[test]
fn fresh_responses_sse_rate_limit_rotates_without_overload_penalty() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::sse_rate_limited(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let harness = ready_profiles(&backend);

    let reply = proxy_runtime_responses_request(
        115,
        &responses_request(br#"{"input":[]}"#),
        harness.shared(),
    )
    .expect("SSE rate limit should rotate to the next profile");
    let (status, body, profile) = consume_responses_reply(reply);

    assert_eq!(status, 200, "{body}");
    assert_eq!(profile.as_deref(), Some("second"));
    assert_eq!(
        backend.responses_accounts(),
        ["main-account", "second-account"]
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("sse_rate_limited"),
        "SSE rate-limit classification should be visible: {log}"
    );
    assert!(
        !log.contains("upstream_overloaded route=responses profile=main"),
        "SSE rate limits must not receive overload health treatment: {log}"
    );
}

#[test]
fn compact_rate_limit_rotates_without_overload_penalty() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::rate_limited_429(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
        ]));
    let harness = ready_profiles(&backend);

    let response = proxy_runtime_standard_request(116, &compact_request(), harness.shared())
        .expect("compact rate limit should rotate to the next profile");
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(status, 200, "{body}");
    assert_eq!(
        backend.responses_accounts(),
        ["main-account", "second-account"]
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("compact_rate_limited"),
        "compact rate-limit classification should be visible: {log}"
    );
    assert!(
        !log.contains("compact_retryable_failure profile=main reason=overload"),
        "compact rate limits must not receive overload handling: {log}"
    );
}

#[test]
fn compact_rate_limit_pool_recovery_retries_until_success() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::rate_limited_429(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
            RuntimeProxyBackendFaultStep::rate_limited_429(
                RuntimeProxyBackendFaultRoute::Compact,
                "second-account",
            ),
            RuntimeProxyBackendFaultStep::success(
                RuntimeProxyBackendFaultRoute::Compact,
                "main-account",
            ),
        ]));
    let harness = ready_profiles(&backend);

    let response = proxy_runtime_standard_request(117, &compact_request(), harness.shared())
        .expect(
            "compact temporary rate limits should recover while quota-positive profiles remain",
        );
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(status, 200, "{body}");
    assert_eq!(
        backend.responses_accounts(),
        ["main-account", "second-account", "main-account"]
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("compact_rate_limited"),
        "rate-limit classification must remain visible during recovery: {log}"
    );
    assert!(
        log.contains("rotation_waiting_for_recovery") || log.contains("rotation_sweep_start"),
        "compact must wait for a retryable profile instead of surfacing temporary 429: {log}"
    );
}

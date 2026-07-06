use super::helpers::*;
use super::*;

#[test]
fn attempt_runtime_responses_request_allows_weekly_exhausted_profile_before_send() {
    let backend = RuntimeProxyBackend::start();
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .upstream_base_url(backend.base_url())
    .profile_usage_snapshot(
        "main",
        runtime_usage_snapshot(quota_window_ready(81, 3600), quota_window_exhausted(300)),
    )
    .build();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: Vec::new(),
        body: br#"{"input":[]}"#.to_vec(),
    };

    match attempt_runtime_responses_request(1, &request, harness.shared(), "main", None, None)
        .expect("responses attempt should succeed")
    {
        RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name,
            reason,
        } => panic!(
            "weekly exhausted should not pre-send block profile {profile_name}: {reason}"
        ),
        RuntimeResponsesAttempt::Success { profile_name, .. } => {
            assert_eq!(profile_name, "main");
        }
        RuntimeResponsesAttempt::QuotaBlocked { profile_name, .. } => {
            assert_eq!(profile_name, "main");
        }
        RuntimeResponsesAttempt::AuthFailed { profile_name, .. } => {
            assert_eq!(profile_name, "main");
        }
        RuntimeResponsesAttempt::PreviousResponseNotFound { profile_name, .. } => {
            assert_eq!(profile_name, "main");
        }
    }
    assert_eq!(backend.responses_accounts(), vec!["main-account".to_string()]);
}

#[test]
fn scripted_backend_fault_plain_429_passes_through_without_rotation() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new(
        [RuntimeProxyBackendFaultStep::plain_429(
            RuntimeProxyBackendFaultRoute::Responses,
            "main-account",
        )],
    ));
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .upstream_base_url(backend.base_url())
    .build();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":[]}"#.to_vec(),
    };

    let response = proxy_runtime_responses_request(11, &request, harness.shared())
        .expect("scripted plain 429 should pass through");
    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("plain 429 should be returned as a buffered upstream response");
    };
    let body = String::from_utf8(parts.body.into_vec()).expect("plain 429 body should decode");

    assert_eq!(parts.status, 429);
    assert_eq!(body, "Too Many Requests");
    assert_eq!(backend.responses_accounts(), vec!["main-account".to_string()]);
}

#[test]
fn usage_workspace_credits_error_rotates_to_ready_profile() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new(
        [RuntimeProxyBackendFaultStep::workspace_credits_exhausted(
            RuntimeProxyBackendFaultRoute::Usage,
            "main-account",
        )],
    ));
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .build();
    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/wham/usage".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let response = proxy_runtime_standard_request(12, &request, harness.shared())
        .expect("usage quota failure should rotate to another ready profile");
    let (status, body) = tiny_http_response_status_and_body(response);

    assert_eq!(status, 200, "usage retry should succeed on second profile: {body}");
    assert!(
        body.contains("second@example.com"),
        "ready second profile usage response should be returned: {body}"
    );
    assert!(
        !body.contains("workspace is out of credits"),
        "workspace credits error must not leak while a ready profile succeeds: {body}"
    );
    let usage_accounts = backend.usage_accounts();
    assert_eq!(
        usage_accounts.first().map(String::as_str),
        Some("main-account"),
        "usage route should try main first: {usage_accounts:?}"
    );
    assert!(
        usage_accounts.iter().any(|account| account == "second-account"),
        "usage route should rotate to the ready second profile: {usage_accounts:?}"
    );
}

#[test]
fn precommit_quota_gate_allows_weekly_exhausted_continuation_from_persisted_snapshot() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .profile_usage_snapshot(
        "main",
        runtime_usage_snapshot(quota_window_ready(81, 3600), quota_window_exhausted(300)),
    )
    .build();

    match runtime_precommit_quota_gate(RuntimePrecommitQuotaGateRequest {
        shared: harness.shared(),
        profile_name: "main",
        route_kind: RuntimeRouteKind::Websocket,
        has_continuation_context: true,
        reprobe_context: "websocket_precommit_reprobe",
    })
    .expect("websocket quota gate should succeed")
    {
        RuntimePrecommitQuotaGateDecision::Proceed => {}
        RuntimePrecommitQuotaGateDecision::Block { reason, .. } => {
            panic!("weekly exhausted snapshot should not pre-send block: {reason:?}")
        }
    }
}

#[test]
fn attempt_runtime_standard_request_skips_exhausted_profile_before_send() {
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .profile_usage_snapshot(
        "main",
        runtime_usage_snapshot(quota_window_exhausted(300), quota_window_ready(90, 86_400)),
    )
    .build();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses/compact".to_string(),
        headers: vec![("session_id".to_string(), "sess-123".to_string())],
        body: br#"{"input":[],"instructions":"compact"}"#.to_vec(),
    };

    match attempt_runtime_standard_request(1, &request, harness.shared(), "main", false)
        .expect("standard attempt should succeed")
    {
        RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
            assert_eq!(profile_name, "main");
        }
        _ => panic!("expected exhausted pre-send compact skip"),
    }
}

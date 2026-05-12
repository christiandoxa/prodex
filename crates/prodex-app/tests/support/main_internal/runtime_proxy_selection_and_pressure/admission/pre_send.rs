use super::helpers::*;
use super::*;

#[test]
fn attempt_runtime_responses_request_skips_exhausted_profile_before_send() {
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
        } => {
            assert_eq!(profile_name, "main");
            assert_eq!(reason, "quota_exhausted_before_send");
        }
        _ => panic!("expected exhausted pre-send responses skip"),
    }
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
fn precommit_quota_gate_skips_websocket_continuation_from_persisted_snapshot() {
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
        RuntimePrecommitQuotaGateDecision::Block {
            reason,
            summary,
            source,
        } => {
            assert_eq!(
                reason,
                RuntimePrecommitQuotaBlockReason::ExhaustedBeforeSend
            );
            assert_eq!(summary.weekly.status, RuntimeQuotaWindowStatus::Exhausted);
            assert_eq!(source, Some(RuntimeQuotaSource::PersistedSnapshot));
        }
        RuntimePrecommitQuotaGateDecision::Proceed => {
            panic!("expected websocket precommit gate to block exhausted snapshot")
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

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

    match attempt_runtime_responses_request(1, &request, harness.shared(), "main", None)
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

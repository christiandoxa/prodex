use super::super::test_support::{
    test_runtime_local_websocket_pair, test_runtime_shared, test_runtime_websocket_flow,
};
use super::*;

#[test]
fn candidate_usage_not_included_with_ready_fallback_rotates() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-usage-not-included");
    {
        let mut runtime = shared
            .runtime
            .lock()
            .expect("runtime state should not be poisoned");
        let root = runtime.paths.root.clone();
        for name in ["alpha", "beta"] {
            runtime.state.profiles.insert(
                name.to_string(),
                prodex_state::ProfileEntry {
                    codex_home: root.join(format!("{name}-home")),
                    managed: true,
                    email: None,
                    provider: prodex_state::ProfileProvider::Openai,
                },
            );
        }
        runtime.profile_probe_cache.insert(
            "beta".to_string(),
            prodex_shared_types::RuntimeProfileProbeCacheEntry {
                checked_at: chrono::Local::now().timestamp(),
                auth: prodex_quota::AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Err("cached-ready-profile".to_string()),
            },
        );
    }
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_attempt(
            RuntimeWebsocketAttempt::QuotaBlocked {
                profile_name: "alpha".to_string(),
                payload: RuntimeWebsocketErrorPayload::Text(
                    runtime_proxy_websocket_error_payload_text(
                        429,
                        "usage_not_included",
                        "Your workspace is out of credits.",
                    ),
                ),
            },
            None,
        )
        .expect("usage_not_included should rotate when another profile is ready");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
    assert!(matches!(
        flow.last_failure,
        Some((RuntimeUpstreamFailureResponse::Websocket(_), true))
    ));
}

#[test]
fn workspace_credit_continuation_uses_fresh_retry_when_ready_profile_exists() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-workspace-credit-fresh-retry");
    {
        let mut runtime = shared
            .runtime
            .lock()
            .expect("runtime state should not be poisoned");
        let root = runtime.paths.root.clone();
        for name in ["alpha", "beta"] {
            runtime.state.profiles.insert(
                name.to_string(),
                prodex_state::ProfileEntry {
                    codex_home: root.join(format!("{name}-home")),
                    managed: true,
                    email: None,
                    provider: prodex_state::ProfileProvider::Openai,
                },
            );
        }
        runtime.profile_probe_cache.insert(
            "beta".to_string(),
            prodex_shared_types::RuntimeProfileProbeCacheEntry {
                checked_at: chrono::Local::now().timestamp(),
                auth: prodex_quota::AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                },
                result: Err("cached-ready-profile".to_string()),
            },
        );
    }
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
    flow.request_text = r#"{"type":"response.create","previous_response_id":"resp_alpha","input":[{"type":"message","content":"again"}]}"#.to_string();
    flow.previous_response_id = Some("resp_alpha".to_string());
    flow.previous_response_fresh_fallback_shape =
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation);
    flow.pinned_profile = Some("alpha".to_string());
    flow.bound_profile = Some("alpha".to_string());
    flow.trusted_previous_response_affinity = true;

    let action = flow
        .handle_candidate_quota_blocked(
            "alpha".to_string(),
            RuntimeWebsocketErrorPayload::Text(
                r#"{"type":"error","status_code":429,"headers":{"X-Codex-Rate-Limit-Reached-Type":"workspace_member_credits_depleted"},"error":{"type":"usage_limit_reached","message":"The usage limit has been reached"}}"#.to_string(),
            ),
        )
        .expect("workspace-credit continuation should become a fresh retry");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
    assert!(flow.previous_response_id.is_none());
    assert!(flow.previous_response_fresh_fallback_shape.is_none());
    assert!(flow.pinned_profile.is_none());
    assert!(flow.bound_profile.is_none());
    assert!(!flow.trusted_previous_response_affinity);
    assert!(!flow.request_text.contains("previous_response_id"));
    assert!(matches!(
        &flow.last_failure,
        Some((RuntimeUpstreamFailureResponse::Websocket(_), true))
    ));
}

#[test]
fn candidate_attempt_reuse_watchdog_tripped_excludes_profile_and_continues() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-reuse-watchdog");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_attempt(
            RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name: "alpha".to_string(),
                event: "timeout",
            },
            None,
        )
        .expect("reuse watchdog handling should succeed");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
}

#[test]
fn candidate_attempt_transport_failed_excludes_profile_and_continues() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-transport");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_attempt(
            RuntimeWebsocketAttempt::TransportFailed {
                profile_name: "alpha".to_string(),
                stage: "connect",
            },
            None,
        )
        .expect("transport failure handling should succeed");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
    assert!(matches!(
        flow.last_failure,
        Some((RuntimeUpstreamFailureResponse::Websocket(_), true))
    ));
}

#[test]
fn candidate_overloaded_releasable_affinity_rotates() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-overloaded");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_overloaded(
            "alpha".to_string(),
            RuntimeWebsocketErrorPayload::Text(runtime_proxy_websocket_error_payload_text(
                503,
                "server_is_overloaded",
                "Upstream Codex backend is currently overloaded.",
            )),
        )
        .expect("overload handling should succeed");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
    assert!(matches!(
        flow.last_failure,
        Some((RuntimeUpstreamFailureResponse::Websocket(_), false))
    ));
}

#[test]
fn candidate_local_selection_blocked_releasable_affinity_rotates() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("failure-local-selection");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_candidate_local_selection_blocked(
            "alpha".to_string(),
            "quota_windows_unavailable_after_reprobe",
        )
        .expect("local selection block handling should succeed");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.excluded_profiles.contains("alpha"));
}

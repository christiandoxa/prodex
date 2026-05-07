use super::super::test_support::{
    test_runtime_local_websocket_pair, test_runtime_shared, test_runtime_websocket_flow,
};
use super::*;

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

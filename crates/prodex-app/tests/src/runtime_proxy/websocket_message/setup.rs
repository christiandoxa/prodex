use super::super::test_support::{
    test_runtime_local_websocket_pair, test_runtime_shared, test_runtime_websocket_flow,
};
use super::*;

#[test]
fn turn_state_override_prefers_retry_value_for_matching_candidate() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("setup-turn-state-override");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
    flow.request_turn_state = Some("turn-original".to_string());
    flow.candidate_turn_state_retry_profile = Some("alpha".to_string());
    flow.candidate_turn_state_retry_value = Some("turn-retry".to_string());

    assert_eq!(
        flow.turn_state_override_for("alpha"),
        Some("turn-retry".to_string())
    );
    assert_eq!(
        flow.turn_state_override_for("beta"),
        Some("turn-original".to_string())
    );
}

#[test]
fn should_promote_committed_profile_only_for_fresh_requests() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("setup-promote-committed");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    assert!(flow.should_promote_committed_profile());

    flow.request_session_id_header_present = true;
    assert!(
        flow.should_promote_committed_profile(),
        "a fresh session_id header should not hide runtime auto-rotation"
    );

    flow.bound_session_profile = Some("alpha".to_string());
    assert!(!flow.should_promote_committed_profile());
}

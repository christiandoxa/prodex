use super::super::test_support::{
    read_runtime_websocket_text, test_runtime_local_websocket_pair, test_runtime_shared,
    test_runtime_websocket_flow,
};
use super::*;

#[test]
fn previous_response_not_found_rotate_stashes_last_failure() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("continuation-rotate");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .apply_previous_response_not_found_action(
            RuntimePreviousResponseNotFoundAction::Rotate,
            RuntimeWebsocketErrorPayload::Text("upstream error".to_string()),
        )
        .expect("rotate handling should succeed");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(matches!(
        flow.last_failure,
        Some((RuntimeUpstreamFailureResponse::Websocket(_), false))
    ));
}

#[test]
fn stale_continuation_action_sends_error_frame_and_finishes() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("continuation-stale");
    let (mut local_socket, mut client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .apply_previous_response_not_found_action(
            RuntimePreviousResponseNotFoundAction::StaleContinuation,
            RuntimeWebsocketErrorPayload::Empty,
        )
        .expect("stale continuation handling should succeed");
    let frame = read_runtime_websocket_text(&mut client_socket);

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Finished
    ));
    assert!(frame.contains("\"code\":\"stale_continuation\""));
}

#[test]
fn connection_limit_retries_same_profile_with_fresh_socket() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("continuation-connection-limit");
    let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    let mut flow = test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);

    let action = flow
        .handle_reuse_watchdog_tripped(
            "main".to_string(),
            "connection_limit_reached",
            /*turn_state_override*/ None,
        )
        .expect("connection limit should retry a fresh websocket once");

    assert!(matches!(
        action,
        RuntimeWebsocketMessageLoopAction::Continue
    ));
    assert!(flow.websocket_reuse_fresh_retry_profiles.contains("main"));
    assert!(
        !flow.excluded_profiles.contains("main"),
        "connection limit is socket expiry, not profile failure"
    );
}

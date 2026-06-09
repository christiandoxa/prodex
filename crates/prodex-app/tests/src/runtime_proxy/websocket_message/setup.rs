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

#[test]
fn response_processed_is_absorbed_without_touching_existing_upstream_session() {
    let _guard = acquire_test_runtime_lock();
    let shared = test_runtime_shared("setup-response-processed");
    let (mut local_socket, _local_peer) = test_runtime_local_websocket_pair();
    let (_upstream_peer, upstream_socket) = test_runtime_local_websocket_pair();
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    websocket_session.store(
        upstream_socket,
        "alpha",
        Some("turn-alpha".to_string()),
        None,
    );

    let request_text = r#"{"type":"response.processed","response_id":"resp-alpha"}"#;
    proxy_runtime_websocket_text_message(RuntimeWebsocketTextMessageInput {
        session_id: 77,
        request_id: 88,
        local_socket: &mut local_socket,
        handshake_request: &RuntimeProxyRequest {
            method: "GET".to_string(),
            path_and_query: "/backend-api/prodex/responses".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        },
        request_text,
        request_metadata: &RuntimeWebsocketRequestMetadata::default(),
        shared: &shared,
        websocket_session: &mut websocket_session,
    })
    .expect("response.processed should be absorbed as a removed legacy request");

    assert_eq!(websocket_session.profile_name.as_deref(), Some("alpha"));
    assert_eq!(websocket_session.turn_state.as_deref(), Some("turn-alpha"));
}

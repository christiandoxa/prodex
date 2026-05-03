use super::super::test_support::{
    test_runtime_local_websocket_pair, test_runtime_shared, test_runtime_websocket_flow,
};
use super::*;

#[test]
fn direct_current_profile_fallback_requires_fresh_request_context() {
    let _guard = acquire_test_runtime_lock();
    let cases = [
        (Some("resp-1"), None, None, None, None, false, false, false),
        (None, Some("alpha"), None, None, None, false, false, false),
        (None, None, Some("ts-1"), None, None, false, false, false),
        (None, None, None, Some("alpha"), None, false, false, false),
        (None, None, None, None, Some("alpha"), false, false, false),
        (None, None, None, None, None, true, false, false),
        (None, None, None, None, None, false, true, false),
        (None, None, None, None, None, false, false, true),
    ];

    for (
        previous_response_id,
        pinned_profile,
        request_turn_state,
        turn_state_profile,
        session_profile,
        saw_inflight_saturation,
        saw_failure,
        expected,
    ) in cases
    {
        let shared = test_runtime_shared("loop-direct-current");
        let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        let mut flow =
            test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
        flow.previous_response_id = previous_response_id.map(str::to_string);
        flow.pinned_profile = pinned_profile.map(str::to_string);
        flow.request_turn_state = request_turn_state.map(str::to_string);
        flow.turn_state_profile = turn_state_profile.map(str::to_string);
        flow.session_profile = session_profile.map(str::to_string);
        flow.saw_inflight_saturation = saw_inflight_saturation;
        if saw_failure {
            flow.last_failure = Some((
                RuntimeUpstreamFailureResponse::Websocket(RuntimeWebsocketErrorPayload::Empty),
                false,
            ));
        }

        assert_eq!(flow.allows_direct_current_profile_fallback(), expected);
    }
}

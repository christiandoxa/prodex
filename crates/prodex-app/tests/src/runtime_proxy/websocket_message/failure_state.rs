use super::super::test_support::{
    test_runtime_local_websocket_pair, test_runtime_shared, test_runtime_websocket_flow,
};
use super::*;

#[test]
fn failure_state_plan_reaches_the_live_websocket_consumer_for_each_class() {
    let _guard = acquire_test_runtime_lock();
    let mut confirmed = 0;
    for class in ["rate", "auth", "overload", "local"] {
        let shared = test_runtime_shared("failure-state-consumer");
        let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        let mut flow =
            test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
        let action = match class {
            "rate" => flow.handle_candidate_rate_limited(
                "alpha".to_string(),
                RuntimeWebsocketErrorPayload::Text(runtime_proxy_websocket_error_payload_text(
                    429,
                    "rate_limit_exceeded",
                    "retry later",
                )),
                None,
            ),
            "auth" => {
                flow.bound_profile = Some("owner".to_string());
                flow.handle_candidate_auth_failed(
                    "alpha".to_string(),
                    RuntimeWebsocketErrorPayload::Text(runtime_proxy_websocket_error_payload_text(
                        401,
                        "unauthorized",
                        "credentials rejected",
                    )),
                )
            }
            "overload" => flow.handle_candidate_overloaded(
                "alpha".to_string(),
                RuntimeWebsocketErrorPayload::Text(runtime_proxy_websocket_error_payload_text(
                    503,
                    "server_is_overloaded",
                    "try another profile",
                )),
            ),
            "local" => flow.handle_candidate_local_selection_blocked(
                "alpha".to_string(),
                "quota_windows_unavailable_after_reprobe",
            ),
            _ => unreachable!(),
        }
        .expect("live WebSocket failure consumer should apply the Mojo plan");
        assert!(matches!(
            action,
            RuntimeWebsocketMessageLoopAction::Continue
        ));
        assert!(flow.excluded_profiles.contains("alpha"));
        if matches!(class, "rate" | "overload") {
            assert!(flow.last_failure.is_some());
        }
        if class == "auth" {
            assert_eq!(flow.bound_profile.as_deref(), Some("owner"));
        }
        confirmed += 1;
    }
    assert_eq!(confirmed, 4);
}

#[test]
fn malformed_and_empty_rejected_errors_finish_without_rotation() {
    let _guard = acquire_test_runtime_lock();
    for payload in [
        RuntimeWebsocketErrorPayload::Empty,
        RuntimeWebsocketErrorPayload::Text(String::new()),
        RuntimeWebsocketErrorPayload::Text("{".to_string()),
    ] {
        let shared = test_runtime_shared("failure-malformed-rejected");
        let (mut local_socket, _client_socket) = test_runtime_local_websocket_pair();
        let mut websocket_session = RuntimeWebsocketSessionState::default();
        let mut flow =
            test_runtime_websocket_flow(&mut local_socket, &shared, &mut websocket_session);
        let action = flow
            .handle_candidate_attempt(
                RuntimeWebsocketAttempt::Rejected {
                    profile_name: "alpha".to_string(),
                    payload,
                },
                None,
            )
            .expect("malformed upstream errors should pass through safely");
        assert!(matches!(
            action,
            RuntimeWebsocketMessageLoopAction::Finished
        ));
        assert!(flow.excluded_profiles.is_empty());
    }
}

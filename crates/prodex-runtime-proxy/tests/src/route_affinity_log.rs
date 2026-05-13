use super::*;

#[test]
fn route_affinity_recompute_message_uses_http_prefix_and_presence_flags() {
    let message = runtime_response_route_affinity_recompute_log_message(
        RuntimeResponseRouteAffinityLogContext {
            request_id: 42,
            websocket_session_id: None,
            reason: "request_start",
        },
        RuntimeResponseRouteAffinityPresence {
            previous_response_id_present: true,
            request_turn_state_present: false,
            request_session_id_present: true,
            explicit_request_session_id_present: false,
        },
    );

    assert_eq!(
        message,
        "request=42 transport=http route_affinity_recompute reason=request_start previous_response_id_present=true request_turn_state_present=false request_session_id_present=true explicit_session_id_present=false"
    );
}

#[test]
fn route_affinity_result_message_formats_owned_profiles_like_debug_state() {
    let message = runtime_response_route_affinity_recompute_result_log_message(
        RuntimeResponseRouteAffinityLogContext {
            request_id: 7,
            websocket_session_id: Some(99),
            reason: "message",
        },
        RuntimeResponseRouteAffinityPresence {
            previous_response_id_present: false,
            request_turn_state_present: true,
            request_session_id_present: true,
            explicit_request_session_id_present: true,
        },
        RuntimeResponseRouteAffinityLogState {
            bound_session_profile: Some("alpha"),
            compact_followup_profile: Some(("beta", "turn_state")),
            compact_session_profile: Some("gamma"),
            session_profile: Some("alpha"),
            pinned_profile: Some("beta"),
        },
    );

    assert_eq!(
        message,
        "request=7 websocket_session=99 route_affinity_recompute_result reason=message previous_response_id_present=false request_turn_state_present=true request_session_id_present=true explicit_session_id_present=true bound_session_profile=Some(\"alpha\") compact_followup_profile=Some((\"beta\", \"turn_state\")) compact_session_profile=Some(\"gamma\") session_profile=Some(\"alpha\") pinned_profile=Some(\"beta\")"
    );
}

#[test]
fn route_affinity_owner_messages_preserve_followup_and_session_sources() {
    let messages = runtime_response_route_affinity_compact_followup_owner_log_messages(
        RuntimeResponseRouteAffinityLogContext {
            request_id: 12,
            websocket_session_id: Some(5),
            reason: "ignored",
        },
        RuntimeResponseRouteAffinityLogState {
            compact_followup_profile: Some(("beta", "turn_state")),
            compact_session_profile: Some("gamma"),
            ..RuntimeResponseRouteAffinityLogState::default()
        },
    );

    assert_eq!(
        messages,
        vec![
            "request=12 websocket_session=5 compact_followup_owner profile=beta source=turn_state",
            "request=12 websocket_session=5 compact_followup_owner profile=gamma source=session_id",
        ]
    );
}

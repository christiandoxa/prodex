use crate::RuntimeInspectedWebsocketTextFrame;

pub fn runtime_websocket_precommit_hold_promotion_allowed(
    reuse_existing_session: bool,
    request_previous_response_id: Option<&str>,
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_override: Option<&str>,
    promote_committed_profile: bool,
) -> bool {
    prodex_mojo_core::runtime::websocket_response_plan(
        prodex_mojo_core::runtime::WebsocketResponsePlanInput {
            reuse_existing_session,
            request_previous_response_present: request_previous_response_id.is_some(),
            request_session_present: request_session_id.is_some(),
            request_turn_state_present: request_turn_state.is_some(),
            turn_state_override_present: turn_state_override.is_some(),
            promote_committed_profile,
            ..Default::default()
        },
    )
    .expect("Mojo websocket response planning returned an invalid result")
    .hold_promotion_allowed
}

pub fn runtime_websocket_precommit_transport_retry_allowed(
    reuse_existing_session: bool,
    request_previous_response_id: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_override: Option<&str>,
    promote_committed_profile: bool,
) -> bool {
    prodex_mojo_core::runtime::websocket_response_plan(
        prodex_mojo_core::runtime::WebsocketResponsePlanInput {
            reuse_existing_session,
            request_previous_response_present: request_previous_response_id.is_some(),
            request_turn_state_present: request_turn_state.is_some(),
            turn_state_override_present: turn_state_override.is_some(),
            promote_committed_profile,
            ..Default::default()
        },
    )
    .expect("Mojo websocket retry planning returned an invalid result")
    .transport_retry_allowed
}

pub fn runtime_websocket_precommit_hold_promotion_event_seen(
    inspected: &RuntimeInspectedWebsocketTextFrame,
) -> bool {
    let _ = inspected;
    false
}

#[cfg(test)]
#[path = "../tests/src/websocket_response_tracking.rs"]
mod tests;

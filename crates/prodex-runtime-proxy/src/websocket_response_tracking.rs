use crate::RuntimeInspectedWebsocketTextFrame;

pub fn runtime_websocket_precommit_hold_promotion_allowed(
    reuse_existing_session: bool,
    request_previous_response_id: Option<&str>,
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_override: Option<&str>,
    promote_committed_profile: bool,
) -> bool {
    !reuse_existing_session
        && request_previous_response_id.is_none()
        && request_session_id.is_none()
        && request_turn_state.is_none()
        && turn_state_override.is_none()
        && promote_committed_profile
}

pub fn runtime_websocket_precommit_transport_retry_allowed(
    reuse_existing_session: bool,
    request_previous_response_id: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_override: Option<&str>,
    promote_committed_profile: bool,
) -> bool {
    !reuse_existing_session
        && request_previous_response_id.is_none()
        && request_turn_state.is_none()
        && turn_state_override.is_none()
        && promote_committed_profile
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

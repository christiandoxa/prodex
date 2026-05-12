use super::*;

#[test]
fn precommit_hold_promotion_requires_fresh_promotable_websocket_request() {
    assert!(runtime_websocket_precommit_hold_promotion_allowed(
        false, None, None, None, None, true,
    ));
    assert!(!runtime_websocket_precommit_hold_promotion_allowed(
        true, None, None, None, None, true,
    ));
    assert!(!runtime_websocket_precommit_hold_promotion_allowed(
        false,
        Some("resp_1"),
        None,
        None,
        None,
        true,
    ));
    assert!(!runtime_websocket_precommit_hold_promotion_allowed(
        false,
        None,
        Some("session_1"),
        None,
        None,
        true,
    ));
    assert!(!runtime_websocket_precommit_hold_promotion_allowed(
        false,
        None,
        None,
        Some("turn_1"),
        None,
        true,
    ));
    assert!(!runtime_websocket_precommit_hold_promotion_allowed(
        false,
        None,
        None,
        None,
        Some("turn_1"),
        true,
    ));
    assert!(!runtime_websocket_precommit_hold_promotion_allowed(
        false, None, None, None, None, false,
    ));
}

#[test]
fn precommit_transport_retry_requires_replayable_fresh_request() {
    assert!(runtime_websocket_precommit_transport_retry_allowed(
        false, None, None, None, true,
    ));
    assert!(!runtime_websocket_precommit_transport_retry_allowed(
        true, None, None, None, true,
    ));
    assert!(!runtime_websocket_precommit_transport_retry_allowed(
        false,
        Some("resp_1"),
        None,
        None,
        true,
    ));
    assert!(!runtime_websocket_precommit_transport_retry_allowed(
        false,
        None,
        Some("turn_1"),
        None,
        true,
    ));
    assert!(!runtime_websocket_precommit_transport_retry_allowed(
        false,
        None,
        None,
        Some("turn_1"),
        true,
    ));
    assert!(!runtime_websocket_precommit_transport_retry_allowed(
        false, None, None, None, false,
    ));
}

#[test]
fn precommit_hold_promotion_event_requires_response_created_with_id() {
    let mut inspected = RuntimeInspectedWebsocketTextFrame {
        event_type: Some("response.created".to_string()),
        response_ids: vec!["resp_1".to_string()],
        ..RuntimeInspectedWebsocketTextFrame::default()
    };
    assert!(runtime_websocket_precommit_hold_promotion_event_seen(
        &inspected
    ));

    inspected.response_ids.clear();
    assert!(!runtime_websocket_precommit_hold_promotion_event_seen(
        &inspected
    ));

    inspected.response_ids.push("resp_1".to_string());
    inspected.event_type = Some("response.in_progress".to_string());
    assert!(!runtime_websocket_precommit_hold_promotion_event_seen(
        &inspected
    ));
}

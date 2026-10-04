#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::log::{
    CHAIN_LOG_DEAD_UPSTREAM, CHAIN_LOG_RETRIED_OWNER, ChainLogRenderInput,
    ROUTE_AFFINITY_LOG_RECOMPUTE, RouteAffinityLogRenderInput, render_chain_log,
    render_route_affinity_log, render_route_affinity_owner_logs,
};

#[test]
fn route_affinity_mojo_abi_preserves_http_flags_and_owner_order() {
    let input = RouteAffinityLogRenderInput {
        request_id: 42,
        websocket_session: None,
        reason: "request_start",
        previous_response_id_present: true,
        request_turn_state_present: false,
        request_session_id_present: true,
        explicit_request_session_id_present: false,
        bound_session_profile_debug: "",
        compact_followup_profile_debug: "",
        compact_session_profile_debug: "",
        session_profile_debug: "",
        pinned_profile_debug: "",
        compact_followup_profile: None,
        compact_session_profile: None,
    };
    assert_eq!(
        render_route_affinity_log(ROUTE_AFFINITY_LOG_RECOMPUTE, input).unwrap(),
        "request=42 transport=http route_affinity_recompute reason=request_start previous_response_id_present=true request_turn_state_present=false request_session_id_present=true explicit_session_id_present=false"
    );

    let owners = render_route_affinity_owner_logs(RouteAffinityLogRenderInput {
        request_id: 12,
        websocket_session: Some(5),
        compact_followup_profile: Some(("beta", "turn_state")),
        compact_session_profile: Some("gamma"),
        ..input
    })
    .unwrap();
    assert_eq!(
        owners,
        [
            "request=12 websocket_session=5 compact_followup_owner profile=beta source=turn_state",
            "request=12 websocket_session=5 compact_followup_owner profile=gamma source=session_id",
        ]
    );
    assert!(render_route_affinity_owner_logs(input).unwrap().is_empty());
}

#[test]
fn chain_mojo_abi_preserves_retry_and_missing_value_branches() {
    assert_eq!(
        render_chain_log(ChainLogRenderInput {
            operation: CHAIN_LOG_RETRIED_OWNER,
            request_id: 7,
            transport: "websocket",
            route: "responses",
            websocket_session: Some(42),
            profile: "work",
            previous_response_id: Some("resp_123"),
            reason: "transport_backoff",
            via: Some("previous_response"),
            detail: "125",
            detail_present: true,
        })
        .unwrap(),
        "request=7 transport=websocket route=responses websocket_session=42 chain_retried_owner profile=work previous_response_id=resp_123 delay_ms=125 reason=transport_backoff via=previous_response"
    );
    assert_eq!(
        render_chain_log(ChainLogRenderInput {
            operation: CHAIN_LOG_DEAD_UPSTREAM,
            request_id: 9,
            transport: "http",
            route: "compact",
            websocket_session: None,
            profile: "default",
            previous_response_id: None,
            reason: "previous_response_not_found",
            via: None,
            detail: "",
            detail_present: false,
        })
        .unwrap(),
        "request=9 transport=http route=compact websocket_session=- chain_dead_upstream_confirmed profile=default previous_response_id=- reason=previous_response_not_found via=- event=-"
    );
}

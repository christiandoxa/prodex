use super::*;

#[test]
fn formats_chain_retried_owner_with_optional_fields() {
    let message = runtime_proxy_chain_retried_owner_log_message(
        RuntimeProxyChainLog {
            request_id: 7,
            transport: "websocket",
            route: "responses",
            websocket_session: Some(42),
            profile_name: "work",
            previous_response_id: Some("resp_123"),
            reason: "transport_backoff",
            via: Some("previous_response"),
        },
        125,
    );

    assert_eq!(
        message,
        "request=7 transport=websocket route=responses websocket_session=42 chain_retried_owner profile=work previous_response_id=resp_123 delay_ms=125 reason=transport_backoff via=previous_response"
    );
}

#[test]
fn formats_chain_dead_upstream_confirmed_defaults() {
    let message = runtime_proxy_chain_dead_upstream_confirmed_log_message(
        RuntimeProxyChainLog {
            request_id: 9,
            transport: "http",
            route: "compact",
            websocket_session: None,
            profile_name: "default",
            previous_response_id: None,
            reason: "previous_response_not_found",
            via: None,
        },
        None,
    );

    assert_eq!(
        message,
        "request=9 transport=http route=compact websocket_session=- chain_dead_upstream_confirmed profile=default previous_response_id=- reason=previous_response_not_found via=- event=-"
    );
}

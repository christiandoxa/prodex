use super::*;

#[derive(Clone, Copy)]
pub(crate) struct RuntimeProxyChainLog<'a> {
    pub(crate) request_id: u64,
    pub(crate) transport: &'a str,
    pub(crate) route: &'a str,
    pub(crate) websocket_session: Option<u64>,
    pub(crate) profile_name: &'a str,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) reason: &'a str,
    pub(crate) via: Option<&'a str>,
}

pub(crate) fn runtime_proxy_log_chain_retried_owner(
    shared: &RuntimeRotationProxyShared,
    log: RuntimeProxyChainLog<'_>,
    delay_ms: u128,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={} transport={} route={} websocket_session={} chain_retried_owner profile={} previous_response_id={} delay_ms={delay_ms} reason={} via={}",
            log.request_id,
            log.transport,
            log.route,
            log.websocket_session
                .map(|session_id| session_id.to_string())
                .as_deref()
                .unwrap_or("-"),
            log.profile_name,
            log.previous_response_id.unwrap_or("-"),
            log.reason,
            log.via.unwrap_or("-"),
        ),
    );
}

pub(crate) fn runtime_proxy_log_chain_dead_upstream_confirmed(
    shared: &RuntimeRotationProxyShared,
    log: RuntimeProxyChainLog<'_>,
    event: Option<&str>,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={} transport={} route={} websocket_session={} chain_dead_upstream_confirmed profile={} previous_response_id={} reason={} via={} event={}",
            log.request_id,
            log.transport,
            log.route,
            log.websocket_session
                .map(|session_id| session_id.to_string())
                .as_deref()
                .unwrap_or("-"),
            log.profile_name,
            log.previous_response_id.unwrap_or("-"),
            log.reason,
            log.via.unwrap_or("-"),
            event.unwrap_or("-"),
        ),
    );
}

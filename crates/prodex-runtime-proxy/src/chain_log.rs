#[derive(Clone, Copy)]
pub struct RuntimeProxyChainLog<'a> {
    pub request_id: u64,
    pub transport: &'a str,
    pub route: &'a str,
    pub websocket_session: Option<u64>,
    pub profile_name: &'a str,
    pub previous_response_id: Option<&'a str>,
    pub reason: &'a str,
    pub via: Option<&'a str>,
}

fn runtime_proxy_chain_log_websocket_session(log: RuntimeProxyChainLog<'_>) -> String {
    log.websocket_session
        .map(|session_id| session_id.to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub fn runtime_proxy_chain_retried_owner_log_message(
    log: RuntimeProxyChainLog<'_>,
    delay_ms: u128,
) -> String {
    format!(
        "request={} transport={} route={} websocket_session={} chain_retried_owner profile={} previous_response_id={} delay_ms={delay_ms} reason={} via={}",
        log.request_id,
        log.transport,
        log.route,
        runtime_proxy_chain_log_websocket_session(log),
        log.profile_name,
        log.previous_response_id.unwrap_or("-"),
        log.reason,
        log.via.unwrap_or("-"),
    )
}

pub fn runtime_proxy_chain_dead_upstream_confirmed_log_message(
    log: RuntimeProxyChainLog<'_>,
    event: Option<&str>,
) -> String {
    format!(
        "request={} transport={} route={} websocket_session={} chain_dead_upstream_confirmed profile={} previous_response_id={} reason={} via={} event={}",
        log.request_id,
        log.transport,
        log.route,
        runtime_proxy_chain_log_websocket_session(log),
        log.profile_name,
        log.previous_response_id.unwrap_or("-"),
        log.reason,
        log.via.unwrap_or("-"),
        event.unwrap_or("-"),
    )
}

#[cfg(test)]
#[path = "../tests/src/chain_log.rs"]
mod tests;

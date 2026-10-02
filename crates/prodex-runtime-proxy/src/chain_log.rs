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

pub fn runtime_proxy_chain_retried_owner_log_message(
    log: RuntimeProxyChainLog<'_>,
    delay_ms: u128,
) -> String {
    let delay_ms = delay_ms.to_string();
    prodex_mojo_core::log::render_chain_log(prodex_mojo_core::log::ChainLogRenderInput {
        operation: prodex_mojo_core::log::CHAIN_LOG_RETRIED_OWNER,
        request_id: log.request_id,
        transport: log.transport,
        route: log.route,
        websocket_session: log.websocket_session,
        profile: log.profile_name,
        previous_response_id: log.previous_response_id,
        reason: log.reason,
        via: log.via,
        detail: &delay_ms,
        detail_present: true,
    })
    .expect("Mojo chain retry log renderer returned invalid output")
}

pub fn runtime_proxy_chain_dead_upstream_confirmed_log_message(
    log: RuntimeProxyChainLog<'_>,
    event: Option<&str>,
) -> String {
    prodex_mojo_core::log::render_chain_log(prodex_mojo_core::log::ChainLogRenderInput {
        operation: prodex_mojo_core::log::CHAIN_LOG_DEAD_UPSTREAM,
        request_id: log.request_id,
        transport: log.transport,
        route: log.route,
        websocket_session: log.websocket_session,
        profile: log.profile_name,
        previous_response_id: log.previous_response_id,
        reason: log.reason,
        via: log.via,
        detail: event.unwrap_or_default(),
        detail_present: event.is_some(),
    })
    .expect("Mojo chain dead-upstream log renderer returned invalid output")
}

#[cfg(test)]
#[path = "../tests/src/chain_log.rs"]
mod tests;

use crate::{RuntimeRotationProxyShared, runtime_proxy_log};

pub(crate) use prodex_runtime_broker_log::{
    clear_runtime_proxy_continuity_failure_reason_metrics,
    runtime_proxy_continuity_failure_reason_metrics_snapshot,
};

pub(crate) fn runtime_proxy_record_continuity_failure_reason(
    shared: &RuntimeRotationProxyShared,
    event: &str,
    reason: &str,
) {
    prodex_runtime_broker_log::runtime_proxy_record_continuity_failure_reason_for_log_path(
        &shared.log_path,
        event,
        reason,
    );
}

#[cfg(test)]
pub(crate) fn clear_all_runtime_proxy_continuity_failure_reason_metrics() {
    prodex_runtime_broker_log::clear_all_runtime_proxy_continuity_failure_reason_metrics_for_test();
}

#[cfg(test)]
pub(crate) fn runtime_proxy_continuity_failure_reason_metrics_store_entry_count() -> usize {
    prodex_runtime_broker_log::runtime_proxy_continuity_failure_reason_metrics_store_entry_count_for_test()
}

pub(crate) use runtime_proxy_crate::RuntimeProxyChainLog;

pub(crate) fn runtime_proxy_log_chain_retried_owner(
    shared: &RuntimeRotationProxyShared,
    log: RuntimeProxyChainLog<'_>,
    delay_ms: u128,
) {
    runtime_proxy_record_continuity_failure_reason(shared, "chain_retried_owner", log.reason);
    runtime_proxy_log(
        shared,
        runtime_proxy_crate::runtime_proxy_chain_retried_owner_log_message(log, delay_ms),
    );
}

pub(crate) fn runtime_proxy_log_chain_dead_upstream_confirmed(
    shared: &RuntimeRotationProxyShared,
    log: RuntimeProxyChainLog<'_>,
    event: Option<&str>,
) {
    runtime_proxy_record_continuity_failure_reason(
        shared,
        "chain_dead_upstream_confirmed",
        log.reason,
    );
    runtime_proxy_log(
        shared,
        runtime_proxy_crate::runtime_proxy_chain_dead_upstream_confirmed_log_message(log, event),
    );
}

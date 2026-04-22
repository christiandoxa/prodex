use super::*;

static RUNTIME_PROXY_CONTINUITY_FAILURE_REASON_METRICS: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimeBrokerContinuityFailureReasonMetrics>>,
> = OnceLock::new();

fn runtime_proxy_continuity_failure_reason_metrics_store()
-> &'static Mutex<BTreeMap<PathBuf, RuntimeBrokerContinuityFailureReasonMetrics>> {
    RUNTIME_PROXY_CONTINUITY_FAILURE_REASON_METRICS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn increment_runtime_proxy_reason_metric(map: &mut BTreeMap<String, usize>, reason: &str) {
    *map.entry(reason.to_string()).or_insert(0) += 1;
}

pub(crate) fn runtime_proxy_record_continuity_failure_reason(
    shared: &RuntimeRotationProxyShared,
    event: &str,
    reason: &str,
) {
    let mut store = runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let metrics = store.entry(shared.log_path.clone()).or_default();
    match event {
        "chain_retried_owner" => {
            increment_runtime_proxy_reason_metric(&mut metrics.chain_retried_owner, reason);
        }
        "chain_dead_upstream_confirmed" => {
            increment_runtime_proxy_reason_metric(
                &mut metrics.chain_dead_upstream_confirmed,
                reason,
            );
        }
        "stale_continuation" => {
            increment_runtime_proxy_reason_metric(&mut metrics.stale_continuation, reason);
        }
        _ => {}
    }
}

pub(crate) fn runtime_proxy_continuity_failure_reason_metrics(
    log_path: &Path,
) -> RuntimeBrokerContinuityFailureReasonMetrics {
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(log_path)
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn clear_runtime_proxy_continuity_failure_reason_metrics(log_path: &Path) {
    runtime_proxy_continuity_failure_reason_metrics_store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(log_path);
}

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
    runtime_proxy_record_continuity_failure_reason(shared, "chain_retried_owner", log.reason);
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
    runtime_proxy_record_continuity_failure_reason(
        shared,
        "chain_dead_upstream_confirmed",
        log.reason,
    );
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

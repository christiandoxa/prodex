use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;
use crate::RuntimeRotationProxyShared;

pub(super) fn runtime_gateway_audit_usage_reconciliation(
    runtime_shared: &RuntimeRotationProxyShared,
    state_store: &RuntimeGatewayStateStore,
    event: &RuntimeProviderGatewaySpendEvent,
    outcome: &str,
) {
    let default_log_dir = runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    let reason = match event
        .reconciliation_reason
        .unwrap_or(prodex_domain::ReservationReconciliationReason::Completed)
    {
        prodex_domain::ReservationReconciliationReason::Completed => "completed",
        prodex_domain::ReservationReconciliationReason::Cancelled => "cancelled",
        prodex_domain::ReservationReconciliationReason::StreamInterrupted => "stream_interrupted",
    };
    let _ = prodex_audit_log::append_audit_event(
        &path,
        "gateway_data_plane",
        "usage_reconciliation",
        outcome,
        serde_json::json!({
            "state_backend": state_store.label(),
            "details": {"reason": reason},
        }),
    );
}

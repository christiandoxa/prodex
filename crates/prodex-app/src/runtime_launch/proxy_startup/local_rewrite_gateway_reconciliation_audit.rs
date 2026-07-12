use crate::RuntimeRotationProxyShared;
use prodex_application::ApplicationUsageReconciliationAuditPlan;

pub(super) fn runtime_gateway_audit_usage_reconciliation(
    runtime_shared: &RuntimeRotationProxyShared,
    plan: ApplicationUsageReconciliationAuditPlan,
) -> anyhow::Result<()> {
    let default_log_dir = runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    prodex_audit_log::append_audit_event(
        &path,
        plan.component(),
        plan.action(),
        plan.outcome(),
        serde_json::json!({
            "state_backend": plan.backend(),
            "details": {"reason": plan.reason()},
        }),
    )
}

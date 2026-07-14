use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use runtime_proxy_crate::path_without_query;
use std::path::Path;

pub(super) fn runtime_gateway_audit_admin_auth_event(
    shared: &RuntimeLocalRewriteProxyShared,
    action: &'static str,
    outcome: &'static str,
    details: serde_json::Value,
) {
    let payload = serde_json::json!({
        "state_backend": shared.gateway_state_store.label(),
        "details": details,
    });
    let default_log_dir = shared
        .runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    let _ = prodex_audit_log::append_audit_event(&path, "gateway_admin", action, outcome, payload);
}

pub(super) fn runtime_gateway_audit_admin_request_denied_event(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    reason: &'static str,
    method: &str,
    path: &str,
) -> bool {
    runtime_gateway_audit_admin_denial(
        shared,
        admin_auth,
        "request_denied",
        reason,
        serde_json::json!({
            "actor": admin_auth.name,
            "role": admin_auth.role.as_str(),
            "method": method,
            "path": path_without_query(path),
        }),
    )
}

pub(super) fn runtime_gateway_audit_admin_role_denied_event(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    method: &str,
    path: &str,
) -> bool {
    runtime_gateway_audit_admin_denial(
        shared,
        admin_auth,
        "authorization_denied",
        "role_forbidden",
        serde_json::json!({
            "actor": admin_auth.name,
            "role": admin_auth.role.as_str(),
            "method": method,
            "path": path_without_query(path),
        }),
    )
}

pub(super) fn runtime_gateway_audit_admin_authorization_denied_event(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    resource: &'static str,
    action: &'static str,
    resource_name: &str,
) -> bool {
    runtime_gateway_audit_admin_denial(
        shared,
        admin_auth,
        "authorization_denied",
        "scope_forbidden",
        serde_json::json!({
            "actor": admin_auth.name,
            "role": admin_auth.role.as_str(),
            "resource": resource,
            "action": action,
            "resource_name": resource_name,
        }),
    )
}

pub(super) fn runtime_gateway_audit_admin_key_mutation_denied_event(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    reason: &'static str,
    action: &'static str,
    key_name: &str,
) -> bool {
    runtime_gateway_audit_admin_denial(
        shared,
        admin_auth,
        "authorization_denied",
        reason,
        serde_json::json!({
            "actor": admin_auth.name,
            "role": admin_auth.role.as_str(),
            "resource": "gateway_key",
            "action": action,
            "resource_name": key_name,
        }),
    )
}

fn runtime_gateway_audit_admin_denial(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    operational_action: &'static str,
    reason: &'static str,
    mut operational_details: serde_json::Value,
) -> bool {
    if !super::local_rewrite_governance_audit::runtime_governance_audit_is_durable(shared) {
        if let Some(details) = operational_details.as_object_mut() {
            details.insert("reason".to_string(), serde_json::json!(reason));
        }
        runtime_gateway_audit_admin_auth_event(
            shared,
            operational_action,
            "failure",
            operational_details,
        );
        return true;
    }
    let principal =
        super::local_rewrite_application_boundary::runtime_gateway_admin_principal(admin_auth);
    let tenant = prodex_domain::TenantContext {
        tenant_id: principal
            .tenant_id
            .expect("gateway admin principal is tenant-bound"),
    };
    let context = super::local_rewrite_governance_audit::RuntimeGovernanceAuditContext::new(
        tenant, principal,
    );
    super::local_rewrite_governance_audit::persist_runtime_material_governance_audit(
        shared,
        &context,
        0,
        "admin_denied",
        prodex_domain::AuditOutcome::Denied,
        reason,
    )
    .is_ok()
}

pub(super) struct RuntimeGatewayAdminRouteExplainAudit<'a> {
    pub(super) control_plane_action: &'a str,
    pub(super) endpoint: &'a str,
    pub(super) requested_model: &'a str,
    pub(super) result_category: &'a str,
    pub(super) selected_route_id: Option<&'a str>,
    pub(super) candidate_count: usize,
    pub(super) diagnostic_seed: u64,
    pub(super) current_load_included: bool,
    pub(super) health_quota_included: bool,
    pub(super) hard_affinity_required: bool,
    pub(super) hard_affinity_applied: bool,
    pub(super) truncated: bool,
}

pub(super) fn runtime_gateway_audit_admin_route_explain_event(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    details: RuntimeGatewayAdminRouteExplainAudit<'_>,
) {
    let payload = serde_json::json!({
        "state_backend": shared.gateway_state_store.label(),
        "details": {
            "operation": "route_explain",
            "control_plane_action": details.control_plane_action,
            "actor": admin_auth.name,
            "role": admin_auth.role.as_str(),
            "tenant_id": admin_auth.tenant_id,
            "endpoint": details.endpoint,
            "requested_model": details.requested_model,
            "result_category": details.result_category,
            "selected_route_id": details.selected_route_id,
            "candidate_count": details.candidate_count,
            "diagnostic_seed": details.diagnostic_seed,
            "current_load_included": details.current_load_included,
            "health_quota_included": details.health_quota_included,
            "hard_affinity_required": details.hard_affinity_required,
            "hard_affinity_applied": details.hard_affinity_applied,
            "truncated": details.truncated,
        },
    });
    let default_log_dir = shared
        .runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    let _ = prodex_audit_log::append_audit_event(
        &path,
        "gateway_admin",
        "route_explain",
        "success",
        payload,
    );
}

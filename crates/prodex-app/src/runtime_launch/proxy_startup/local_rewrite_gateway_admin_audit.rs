use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_store_types::RuntimeGatewayScimUser;
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
    actor: &str,
    role: &str,
    reason: &'static str,
    method: &str,
    path_value: &str,
) {
    let payload = serde_json::json!({
        "state_backend": shared.gateway_state_store.label(),
        "details": {
            "reason": reason,
            "actor": actor,
            "role": role,
            "method": method,
            "path": path_value,
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
        "request_denied",
        "failure",
        payload,
    );
}

pub(super) fn runtime_gateway_audit_admin_role_denied_event(
    shared: &RuntimeLocalRewriteProxyShared,
    actor: &str,
    role: &str,
    method: &str,
    path_value: &str,
) {
    let payload = serde_json::json!({
        "state_backend": shared.gateway_state_store.label(),
        "details": {
            "reason": "role_forbidden",
            "actor": actor,
            "role": role,
            "method": method,
            "path": path_value,
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
        "authorization_denied",
        "failure",
        payload,
    );
}

pub(super) fn runtime_gateway_audit_admin_authorization_denied_event(
    shared: &RuntimeLocalRewriteProxyShared,
    actor: &str,
    role: &str,
    resource: &'static str,
    action: &'static str,
    resource_name: &str,
) {
    let payload = serde_json::json!({
        "state_backend": shared.gateway_state_store.label(),
        "details": {
            "reason": "scope_forbidden",
            "actor": actor,
            "role": role,
            "resource": resource,
            "action": action,
            "resource_name": resource_name,
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
        "authorization_denied",
        "failure",
        payload,
    );
}

pub(super) fn runtime_gateway_audit_admin_key_mutation_denied_event(
    shared: &RuntimeLocalRewriteProxyShared,
    actor: &str,
    role: &str,
    reason: &'static str,
    action: &'static str,
    key_name: &str,
) {
    let payload = serde_json::json!({
        "state_backend": shared.gateway_state_store.label(),
        "details": {
            "reason": reason,
            "actor": actor,
            "role": role,
            "resource": "gateway_key",
            "action": action,
            "resource_name": key_name,
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
        "authorization_denied",
        "failure",
        payload,
    );
}

pub(super) fn runtime_gateway_audit_admin_key_event(
    shared: &RuntimeLocalRewriteProxyShared,
    action: &'static str,
    outcome: &'static str,
    key_name: &str,
    details: serde_json::Value,
) {
    let payload = serde_json::json!({
        "key_name": key_name,
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

pub(super) fn runtime_gateway_audit_admin_scim_user_event(
    shared: &RuntimeLocalRewriteProxyShared,
    action: &'static str,
    outcome: &'static str,
    user: &RuntimeGatewayScimUser,
) {
    let payload = serde_json::json!({
        "user_id": user.id,
        "user_name": user.user_name,
        "state_backend": shared.gateway_state_store.label(),
        "details": {
            "active": user.active,
            "tenant_id": user.tenant_id,
            "team_id": user.team_id,
            "project_id": user.project_id,
            "user_id": user.user_id,
            "budget_id": user.budget_id,
            "role": user.role,
            "allowed_key_prefixes": user.allowed_key_prefixes,
        },
    });
    let default_log_dir = shared
        .runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    let _ = prodex_audit_log::append_audit_event(&path, "gateway_admin", action, outcome, payload);
}

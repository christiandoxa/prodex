use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use serde_json::Value;
use std::path::Path;

pub(super) fn runtime_gateway_audit_data_plane_auth_failed(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &str,
) {
    append_failure(
        shared,
        "auth_failed",
        serde_json::json!({
            "state_backend": shared.gateway_state_store.label(),
            "details": {
                "reason": "missing_or_invalid_gateway_bearer_token",
                "path": path,
            },
        }),
    );
}

pub(super) fn runtime_gateway_audit_data_plane_request_capture_failed(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &str,
) {
    append_failure(
        shared,
        "request_capture_failed",
        serde_json::json!({
            "state_backend": shared.gateway_state_store.label(),
            "details": { "path": path, "reason": "request_capture_failed" },
        }),
    );
}

pub(super) fn runtime_gateway_audit_data_plane_presidio_redaction_failed(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &str,
) {
    append_failure(
        shared,
        "presidio_redaction_failed",
        serde_json::json!({
            "state_backend": shared.gateway_state_store.label(),
            "details": { "path": path, "reason": "presidio_redaction_failed" },
        }),
    );
}

pub(super) fn runtime_gateway_audit_data_plane_request_body_too_large(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &str,
) {
    append_failure(
        shared,
        "request_body_too_large",
        serde_json::json!({
            "state_backend": shared.gateway_state_store.label(),
            "details": { "path": path, "reason": "request_body_too_large" },
        }),
    );
}

pub(super) fn runtime_gateway_audit_data_plane_virtual_key_rejected(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &str,
    reason: &str,
) {
    append_failure(
        shared,
        if reason == "invalid_gateway_key" {
            "auth_failed"
        } else {
            "authorization_denied"
        },
        serde_json::json!({
            "state_backend": shared.gateway_state_store.label(),
            "details": { "reason": reason, "path": path },
        }),
    );
}

pub(super) fn runtime_gateway_audit_data_plane_guardrail_blocked(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &str,
    reason: &str,
) {
    append_failure(
        shared,
        "guardrail_blocked",
        serde_json::json!({
            "state_backend": shared.gateway_state_store.label(),
            "details": { "reason": reason, "path": path },
        }),
    );
}

pub(super) fn runtime_gateway_audit_data_plane_guardrail_webhook_blocked(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &str,
    phase: &str,
    reason: &str,
) {
    append_failure(
        shared,
        "guardrail_webhook_blocked",
        serde_json::json!({
            "state_backend": shared.gateway_state_store.label(),
            "details": { "path": path, "phase": phase, "reason": reason },
        }),
    );
}

fn append_failure(shared: &RuntimeLocalRewriteProxyShared, action: &str, payload: Value) {
    let default_log_dir = shared
        .runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    let _ = prodex_audit_log::append_audit_event(
        &path,
        "gateway_data_plane",
        action,
        "failure",
        payload,
    );
}

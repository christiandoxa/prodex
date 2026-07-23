use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use runtime_proxy_crate::path_without_query;
use serde_json::Value;

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
                "path": path_without_query(path),
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
            "details": { "path": path_without_query(path), "reason": "request_capture_failed" },
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
            "details": { "path": path_without_query(path), "reason": "presidio_redaction_failed" },
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
            "details": { "path": path_without_query(path), "reason": "request_body_too_large" },
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
            "details": { "reason": reason, "path": path_without_query(path) },
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
            "details": { "reason": reason, "path": path_without_query(path) },
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
            "details": { "path": path_without_query(path), "phase": phase, "reason": reason },
        }),
    );
}

fn append_failure(shared: &RuntimeLocalRewriteProxyShared, action: &str, payload: Value) {
    crate::audit_log::append_runtime_audit_event_best_effort(
        &shared.runtime_shared,
        "gateway_data_plane",
        action,
        "failure",
        payload,
    );
}

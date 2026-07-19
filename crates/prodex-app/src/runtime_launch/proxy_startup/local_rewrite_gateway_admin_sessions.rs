use prodex_control_plane::{ControlPlaneActionPlan, ControlPlaneOperation};
use prodex_domain::{CredentialScope, Principal, PrincipalKind, Role, TenantContext};
use prodex_storage::{GovernanceRepositoryError, GovernanceWriteOutcome};

use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_admin_execution::runtime_gateway_admin_mutation_execution;
use super::local_rewrite_gateway_admin_response::runtime_gateway_admin_json_body;
use super::*;

pub(super) fn runtime_gateway_self_service_session_response(
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    principal: &Principal,
) -> tiny_http::ResponseBox {
    if !request.method.eq_ignore_ascii_case("POST") {
        return session_error(
            405,
            "method_not_allowed",
            "current-session revocation requires POST",
        );
    }
    if !request.body.is_empty()
        || request.headers.iter().any(|(name, value)| {
            (name.eq_ignore_ascii_case("content-length") && value.trim() != "0")
                || name.eq_ignore_ascii_case("transfer-encoding")
        })
    {
        return session_error(
            400,
            "governance_session_body_invalid",
            "current-session self-revocation does not accept a request body",
        );
    }
    let Some(tenant_id) = principal.tenant_id else {
        return session_error(
            401,
            "governance_session_identity_invalid",
            "current-session revocation requires a tenant-bound identity",
        );
    };
    let now_seconds =
        super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis() / 1_000;
    match shared.governance_sessions.revoke_current(
        request,
        TenantContext { tenant_id },
        principal,
        now_seconds,
    ) {
        Ok(GovernanceWriteOutcome::Applied | GovernanceWriteOutcome::Replayed) => {
            build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
                status: 204,
                headers: vec![("cache-control".to_string(), b"no-store".to_vec())],
                body: Vec::new().into(),
            })
        }
        Err(GovernanceRepositoryError::NotFound) => {
            session_error(404, "governance_session_not_found", "session was not found")
        }
        Err(GovernanceRepositoryError::InvalidInput) => session_error(
            400,
            "governance_session_revoke_invalid",
            "session revocation request is invalid",
        ),
        Err(_) => session_error(
            503,
            "governance_session_storage_unavailable",
            "session governance storage is unavailable",
        ),
    }
}

pub(super) fn runtime_gateway_admin_session_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    admin_prefix: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> Option<tiny_http::ResponseBox> {
    let session_resource = path
        .strip_prefix(&format!("{admin_prefix}/sessions/"))?
        .strip_suffix("/revoke")?
        .trim();
    if session_resource.is_empty() || session_resource.contains('/') {
        return Some(session_error(
            400,
            "governance_session_hash_invalid",
            "session hash must be 64 lowercase hexadecimal characters",
        ));
    }
    if !captured.method.eq_ignore_ascii_case("POST") {
        return Some(session_error(
            405,
            "method_not_allowed",
            "session revocation endpoint requires POST",
        ));
    }
    let session_id_hash = if session_resource == "current" {
        match super::local_rewrite_governance_session::runtime_gateway_governance_session_hash(
            captured,
        ) {
            Some(hash) => hash,
            None => {
                return Some(session_error(
                    400,
                    "governance_session_id_missing",
                    "current-session revocation requires a session_id header",
                ));
            }
        }
    } else {
        session_resource.to_string()
    };
    let execution = match runtime_gateway_admin_mutation_execution(
        captured,
        path,
        admin_auth,
        base_action,
        ControlPlaneOperation::PolicyPublish,
    ) {
        Ok(execution) => execution,
        Err(response) => return Some(response),
    };
    let reason_code = if captured.body.is_empty() {
        "session.admin_revoke".to_string()
    } else {
        let body = match runtime_gateway_admin_json_body(captured) {
            Ok(body) => body,
            Err(response) => return Some(response),
        };
        match body.get("reason_code").and_then(serde_json::Value::as_str) {
            Some(reason) => reason.to_string(),
            None => {
                return Some(session_error(
                    400,
                    "governance_session_reason_invalid",
                    "reason_code is required",
                ));
            }
        }
    };
    let tenant = execution.authorized_action.tenant;
    let actor = Principal::new(
        execution.authorized_action.audit_event.principal_id,
        Some(tenant.tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    Some(
        match shared.governance_sessions.revoke_by_hash(
            TenantContext {
                tenant_id: tenant.tenant_id,
            },
            &session_id_hash,
            &actor,
            &reason_code,
        ) {
            Ok(GovernanceWriteOutcome::Applied | GovernanceWriteOutcome::Replayed) => {
                build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
                    status: 204,
                    headers: vec![("cache-control".to_string(), b"no-store".to_vec())],
                    body: Vec::new().into(),
                })
            }
            Err(GovernanceRepositoryError::InvalidInput) => session_error(
                400,
                "governance_session_revoke_invalid",
                "session revocation request is invalid",
            ),
            Err(GovernanceRepositoryError::NotFound) => {
                session_error(404, "governance_session_not_found", "session was not found")
            }
            Err(GovernanceRepositoryError::Conflict) => session_error(
                409,
                "governance_session_revoke_conflict",
                "session revocation conflicted with current state",
            ),
            Err(_) => session_error(
                503,
                "governance_session_storage_unavailable",
                "session governance storage is unavailable",
            ),
        },
    )
}

fn session_error(status: u16, code: &'static str, message: &'static str) -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(status, code, message)
}

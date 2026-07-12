use prodex_application::{
    ApplicationControlPlaneAuditErrorStatus, ApplicationControlPlaneGovernanceScope,
    ApplicationControlPlaneIdempotencyErrorStatus, ApplicationControlPlanePreconditionErrorStatus,
    plan_application_control_plane, plan_application_control_plane_audit_error_response,
    plan_application_control_plane_audit_from_http,
    plan_application_control_plane_idempotency_error_response,
    plan_application_control_plane_idempotency_from_http_digest,
    plan_application_control_plane_precondition_error_response,
    plan_application_control_plane_precondition_from_http,
};
use prodex_control_plane::{
    ControlPlaneActionPlan, ControlPlaneDecision, ControlPlaneOperation,
    plan_control_plane_authorization_error_response,
};
use sha2::{Digest, Sha256};

use super::local_rewrite_application_boundary::{
    runtime_gateway_admin_control_plane_action_for_operation,
    runtime_gateway_admin_governance_scope, runtime_gateway_now_unix_ms,
};
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_admin_router::runtime_gateway_http_request_meta;
use super::local_rewrite_gateway_admin_store_mutation::RuntimeGatewayAdminAtomicWrite;
use super::*;

pub(super) struct RuntimeGatewayAdminMutationExecution {
    pub(super) authorized_action: ControlPlaneActionPlan,
    pub(super) governance: ApplicationControlPlaneGovernanceScope,
    pub(super) entity_tag: Option<prodex_domain::EntityTag>,
    pub(super) atomic_write: RuntimeGatewayAdminAtomicWrite,
}

impl std::fmt::Debug for RuntimeGatewayAdminMutationExecution {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeGatewayAdminMutationExecution")
            .field("authorized_action", &"<redacted>")
            .field("governance", &"<redacted>")
            .field("entity_tag", &"<redacted>")
            .field("atomic_write", &"<redacted>")
            .finish()
    }
}

pub(super) fn runtime_gateway_admin_mutation_execution(
    captured: &RuntimeProxyRequest,
    path: &str,
    admin_auth: &RuntimeGatewayAdminAuth,
    base: &ControlPlaneActionPlan,
    operation: ControlPlaneOperation,
) -> Result<RuntimeGatewayAdminMutationExecution, tiny_http::ResponseBox> {
    let alias = matches!(
        (base.operation, operation),
        (
            ControlPlaneOperation::VirtualKeyUpdate,
            ControlPlaneOperation::VirtualKeyRotateSecret,
        )
    );
    if base.operation != operation && !alias {
        return Err(runtime_gateway_admin_execution_error(
            400,
            "control_plane_route_invalid",
            "control-plane route is invalid",
        ));
    }

    let http = runtime_gateway_http_request_meta(captured, path);
    let action =
        runtime_gateway_admin_control_plane_action_for_operation(&http, admin_auth, operation)
            .filter(|action| {
                action.principal.id == base.audit_event.principal_id
                    && action.resource.tenant_id == base.tenant.tenant_id
                    && action.resource.id == base.audit_event.resource.id
            })
            .ok_or_else(|| {
                runtime_gateway_admin_execution_error(
                    400,
                    "control_plane_route_invalid",
                    "control-plane route is invalid",
                )
            })?;

    let authorized_action = if alias {
        match plan_application_control_plane(action.clone()).decision {
            ControlPlaneDecision::Authorized(action) => action,
            ControlPlaneDecision::Denied { error, .. } => {
                let response = plan_control_plane_authorization_error_response(&error);
                return Err(runtime_gateway_admin_execution_error(
                    403,
                    response.code,
                    response.message,
                ));
            }
        }
    } else {
        base.clone()
    };

    plan_application_control_plane_audit_from_http(action.clone(), &http).map_err(|error| {
        let response = plan_application_control_plane_audit_error_response(&error);
        runtime_gateway_admin_execution_error(
            match response.status {
                ApplicationControlPlaneAuditErrorStatus::BadRequest => 400,
                ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => 405,
                ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => 503,
            },
            response.code,
            response.message,
        )
    })?;
    let operation = plan_application_control_plane_idempotency_from_http_digest(
        action.clone(),
        &http,
        runtime_gateway_request_body_sha256(&captured.body),
    )
    .map_err(|error| {
        let response = plan_application_control_plane_idempotency_error_response(&error);
        runtime_gateway_admin_execution_error(
            match response.status {
                ApplicationControlPlaneIdempotencyErrorStatus::BadRequest => 400,
                ApplicationControlPlaneIdempotencyErrorStatus::Conflict => 409,
                ApplicationControlPlaneIdempotencyErrorStatus::MethodNotAllowed => 405,
                ApplicationControlPlaneIdempotencyErrorStatus::ServiceUnavailable => 503,
            },
            response.code,
            response.message,
        )
    })?
    .operation
    .ok_or_else(|| {
        runtime_gateway_admin_execution_error(
            500,
            "control_plane_idempotency_plan_invalid",
            "control-plane idempotency planning failed",
        )
    })?;
    let entity_tag = plan_application_control_plane_precondition_from_http(action, &http)
        .map_err(|error| {
            let response = plan_application_control_plane_precondition_error_response(&error);
            runtime_gateway_admin_execution_error(
                match response.status {
                    ApplicationControlPlanePreconditionErrorStatus::BadRequest => 400,
                    ApplicationControlPlanePreconditionErrorStatus::MethodNotAllowed => 405,
                },
                response.code,
                response.message,
            )
        })?
        .entity_tag;

    let started_at_unix_ms = authorized_action.audit_event.occurred_at_unix_ms;
    Ok(RuntimeGatewayAdminMutationExecution {
        governance: runtime_gateway_admin_governance_scope(admin_auth),
        entity_tag,
        atomic_write: RuntimeGatewayAdminAtomicWrite {
            operation,
            started_at_unix_ms,
            completed_at_unix_ms: runtime_gateway_now_unix_ms().max(started_at_unix_ms),
            audit_event: authorized_action.audit_event.clone(),
        },
        authorized_action,
    })
}

fn runtime_gateway_request_body_sha256(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

fn runtime_gateway_admin_execution_error(
    status: u16,
    code: &'static str,
    message: &'static str,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(status, code, message)
}

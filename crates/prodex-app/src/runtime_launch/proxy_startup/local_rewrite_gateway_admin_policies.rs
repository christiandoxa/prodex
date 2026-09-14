mod activation;
mod audit_retention;
mod break_glass;
mod operations;
mod reporting;
mod repository;
mod response;
mod route;
mod validation;

use activation::activation_response;
use break_glass::{
    break_glass_approval_response, execution_approval_audit_command, execution_approval_json,
    execution_approval_snapshot_json, vote_response,
};
use operations::{
    actor, append_control_plane_audit_command, approval_state, artifact_fingerprint, audit_command,
    control_plane_audit_command, execution, revision_json,
};
use reporting::{
    audit_export_response, audit_integrity_response, outbox_response, status_response,
};
pub(in crate::runtime_launch::proxy_startup) use repository::{
    RuntimeGovernanceRepository, repository, runtime_governance_repository, storage_unavailable,
};
use response::{
    invalid_request, json_response_with_etag, lifecycle_error, lifecycle_repository_error,
    repository_error,
};
use route::{
    RuntimeGatewayAdminPolicyRoute, RuntimeGatewayAdminResourceRoute,
    runtime_gateway_admin_policy_route, runtime_gateway_admin_resource_route,
};
use validation::{governance_artifact_validation_is_valid, validate_response};

use prodex_application::{
    ApplicationAuditRetentionPurgeRequest, ApplicationBreakGlassAuditRequest,
    ApplicationExecutionApprovalService, ApplicationGovernanceLifecycleError,
    ApplicationGovernanceLifecycleService, ApplicationGovernanceRepository,
    plan_application_audit_retention_purge, plan_application_break_glass_with_audit_storage,
};
use prodex_control_plane::{
    BreakGlassAuthorization, ControlPlaneActionPlan, ControlPlaneDecision, ControlPlaneOperation,
};
use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalReasonCode,
    ApprovalRecord, ApprovalScope, AuditAction, AuditEventId, AuditQueryScope, AuditReasonCode,
    AuditResource, AuditRetentionBatchLimit, AuditRetentionHold, AuditRetentionPolicy,
    AuditRetentionPurgeBatch, AuditRetentionPurgeKey, AuditTimestamp, CredentialScope, Principal,
    PrincipalKind, ResourceKind, Role, TenantContext, compute_audit_chain_digest,
};
use prodex_observability::{PolicyLifecycleOperation, PolicyLifecycleResult};
use prodex_storage::{
    AppendOnlyAuditCommand, ApprovalVoteIdempotency, ApprovalVoteMutationOutcome,
    ApprovalVoteRequest, ApprovalVoteSnapshot, AuditOutboxWriteCommand, AuditRetentionPurgeCommand,
    DurableStoreKind, GovernanceActivationAction, GovernanceActivationRequest,
    GovernanceActivationResult, GovernanceArtifactKind, GovernanceArtifactValidationInput,
    GovernanceAuditExportRecord, GovernanceAuditIntegrityHealth, GovernanceMutationIdempotency,
    GovernanceOutboxHealth, GovernanceRepositoryError, GovernanceRevisionSummary,
    GovernanceRevisionWriteCommand, GovernanceStatus, GovernanceWriteOutcome, TenantStorageKey,
};
use prodex_storage_sqlite_runtime::GovernanceSqliteRepository;
use sha2::{Digest, Sha256};
use std::str::FromStr;

use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_application_boundary::{
    runtime_gateway_admin_control_plane_action_for_operation, runtime_gateway_now_unix_ms,
};
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_admin_execution::{
    RuntimeGatewayAdminMutationExecution, runtime_gateway_admin_mutation_execution,
};
use super::local_rewrite_gateway_admin_policy_resource::{
    RuntimeGovernanceResource, policy_activation_operation, record_policy_lifecycle,
};
use super::local_rewrite_gateway_admin_response::{
    runtime_gateway_admin_json_body, runtime_gateway_admin_json_response,
};
use super::local_rewrite_gateway_admin_router::runtime_gateway_http_request_meta;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_governance_artifact_authenticity::{
    governance_artifact_signature_payload_base64, parse_governance_artifact_authenticity,
    runtime_governance_artifact_authenticity_is_valid,
};
use super::*;

pub(super) fn runtime_gateway_admin_policy_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    admin_prefix: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> Option<tiny_http::ResponseBox> {
    let response = match runtime_gateway_admin_policy_route(path, admin_prefix) {
        RuntimeGatewayAdminPolicyRoute::None => return None,
        RuntimeGatewayAdminPolicyRoute::AuditExport => {
            audit_export_response(captured, shared, base_action)
        }
        RuntimeGatewayAdminPolicyRoute::AuditIntegrity => {
            audit_integrity_response(captured, shared, base_action)
        }
        RuntimeGatewayAdminPolicyRoute::Outbox => outbox_response(
            captured,
            path,
            &format!("{admin_prefix}/governance/outbox"),
            shared,
            admin_auth,
            base_action,
        ),
        RuntimeGatewayAdminPolicyRoute::AuditRetention => {
            let repository = match repository(shared) {
                Ok(repository) => repository,
                Err(response) => return Some(response),
            };
            audit_retention::audit_retention_response(
                captured,
                path,
                &format!("{admin_prefix}/audit/retention"),
                shared,
                admin_auth,
                base_action,
                &repository,
            )
        }
        RuntimeGatewayAdminPolicyRoute::ExecutionApprovals => {
            let repository = match repository(shared) {
                Ok(repository) => repository,
                Err(response) => return Some(response),
            };
            execution_approval_response(
                captured,
                path,
                &format!("{admin_prefix}/execution-approvals"),
                admin_auth,
                base_action,
                &repository,
            )
        }
        RuntimeGatewayAdminPolicyRoute::BreakGlassApprovals => {
            let repository = match repository(shared) {
                Ok(repository) => repository,
                Err(response) => return Some(response),
            };
            break_glass_approval_response(
                captured,
                path,
                &format!("{admin_prefix}/break-glass-approvals"),
                admin_auth,
                base_action,
                &repository,
            )
        }
        RuntimeGatewayAdminPolicyRoute::Resource { resource_code } => {
            let resource = *RuntimeGovernanceResource::ALL.get(usize::from(resource_code))?;
            return runtime_gateway_admin_resource_response(
                captured,
                path,
                admin_prefix,
                resource,
                shared,
                admin_auth,
                base_action,
            );
        }
    };
    Some(response)
}

fn runtime_gateway_admin_resource_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    admin_prefix: &str,
    resource: RuntimeGovernanceResource,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> Option<tiny_http::ResponseBox> {
    let resource_path = format!("{admin_prefix}{}", resource.prefix_suffix());
    let suffix = path.strip_prefix(&(resource_path + "/"));
    let tenant_id = base_action.tenant.tenant_id;
    let method = captured.method.as_str();
    let segments = suffix
        .map(|value| {
            value
                .split('/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let route = runtime_gateway_admin_resource_route(method, &segments);
    if route == RuntimeGatewayAdminResourceRoute::Validate {
        return Some(validate_response(captured, shared, tenant_id, resource));
    }
    let repository = match repository(shared) {
        Ok(repository) => repository,
        Err(response) => return Some(response),
    };
    Some(match route {
        RuntimeGatewayAdminResourceRoute::Create => create_response(
            captured,
            path,
            shared,
            resource,
            admin_auth,
            base_action,
            &repository,
        ),
        RuntimeGatewayAdminResourceRoute::List => {
            match repository.list_revisions(tenant_id, resource.kind()) {
                Ok(revisions) => runtime_gateway_admin_json_response(
                    200,
                    serde_json::json!({
                        "object": format!("governance.{}_revision.list", resource.label()),
                        "data": revisions
                            .into_iter()
                            .map(|revision| revision_json(revision, resource))
                            .collect::<Vec<_>>()
                    }),
                ),
                Err(error) => repository_error(error),
            }
        }
        RuntimeGatewayAdminResourceRoute::Status => {
            status_response(&repository, tenant_id, resource)
        }
        RuntimeGatewayAdminResourceRoute::Get { revision_id } => {
            match repository.get_revision(tenant_id, resource.kind(), revision_id) {
                Ok(revision) => {
                    runtime_gateway_admin_json_response(200, revision_json(revision, resource))
                }
                Err(error) => repository_error(error),
            }
        }
        RuntimeGatewayAdminResourceRoute::Submit { revision_id } => submit_response(
            captured,
            path,
            revision_id,
            resource,
            admin_auth,
            base_action,
            &repository,
        ),
        RuntimeGatewayAdminResourceRoute::Vote {
            revision_id,
            approval_id,
        } => vote_response(
            captured,
            path,
            revision_id,
            approval_id,
            resource,
            admin_auth,
            base_action,
            &repository,
        ),
        RuntimeGatewayAdminResourceRoute::Activate {
            revision_id,
            action,
        } => activation_response(
            captured,
            path,
            revision_id,
            action,
            resource,
            shared,
            admin_auth,
            base_action,
            &repository,
        ),
        RuntimeGatewayAdminResourceRoute::NotFound => build_runtime_proxy_json_error_response(
            404,
            "governance_policy_not_found",
            "governance resource was not found",
        ),
        RuntimeGatewayAdminResourceRoute::MethodNotAllowed => {
            build_runtime_proxy_json_error_response(
                405,
                "control_plane_method_not_allowed",
                "HTTP method is not allowed for this governance route",
            )
        }
        RuntimeGatewayAdminResourceRoute::Validate => {
            validate_response(captured, shared, tenant_id, resource)
        }
    })
}

fn create_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    resource: RuntimeGovernanceResource,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let Some(revision_id) = body.get("revision_id").and_then(serde_json::Value::as_str) else {
        return invalid_request();
    };
    let Some(artifact) = body.get("artifact") else {
        return invalid_request();
    };
    let Ok(compiled_artifact) = serde_json::to_vec(artifact) else {
        return invalid_request();
    };
    let authenticity = match parse_governance_artifact_authenticity(&body) {
        Ok(authenticity) => authenticity,
        Err(()) => return invalid_request(),
    };
    let validation = GovernanceArtifactValidationInput {
        tenant_id: base_action.tenant.tenant_id,
        kind: resource.kind(),
        revision_id,
        compiled_artifact: &compiled_artifact,
        authenticity: authenticity.as_ref(),
    };
    if !governance_artifact_validation_is_valid(shared, resource, &validation) {
        return invalid_request();
    }
    let execution = match execution(captured, path, admin_auth, base_action) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let tenant_id = execution.authorized_action.tenant.tenant_id;
    let fingerprint = artifact_fingerprint(&compiled_artifact);
    let Ok(fingerprint_value) = ApprovalFingerprint::new(fingerprint.clone()) else {
        return invalid_request();
    };
    let command = GovernanceRevisionWriteCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        kind: resource.kind(),
        revision_id: revision_id.to_string(),
        fingerprint: fingerprint_value,
        compiled_artifact,
        authenticity,
        created_by: execution.authorized_action.audit_event.principal_id,
        created_at_unix_ms: execution.atomic_write.completed_at_unix_ms,
    };
    let audit = match audit_command(
        repository,
        &execution.authorized_action,
        resource,
        &format!("governance.{}.revision.write", resource.label()),
        Some(revision_id),
    ) {
        Ok(audit) => audit,
        Err(error) => {
            record_policy_lifecycle(
                resource,
                PolicyLifecycleOperation::Create,
                PolicyLifecycleResult::Failed,
            );
            return repository_error(error);
        }
    };
    match ApplicationGovernanceLifecycleService::new(repository).write_revision(
        &execution.authorized_action,
        command,
        GovernanceMutationIdempotency {
            operation: execution.atomic_write.operation.clone(),
            started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
        },
        audit,
    ) {
        Ok(outcome) => {
            record_policy_lifecycle(
                resource,
                PolicyLifecycleOperation::Create,
                PolicyLifecycleResult::Persisted,
            );
            runtime_gateway_admin_json_response(
                if outcome == GovernanceWriteOutcome::Applied {
                    201
                } else {
                    200
                },
                serde_json::json!({
                    "object": format!("governance.{}_revision", resource.label()),
                    "revision_id": revision_id,
                    "fingerprint": fingerprint,
                    "state": "draft",
                    "replayed": outcome == GovernanceWriteOutcome::Replayed,
                }),
            )
        }
        Err(error) => {
            record_policy_lifecycle(
                resource,
                PolicyLifecycleOperation::Create,
                PolicyLifecycleResult::Failed,
            );
            lifecycle_error(error)
        }
    }
}

fn submit_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    revision_id: &str,
    resource: RuntimeGovernanceResource,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let execution = match execution(captured, path, admin_auth, base_action) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let tenant_id = execution.authorized_action.tenant.tenant_id;
    let revision = match repository.get_revision(tenant_id, resource.kind(), revision_id) {
        Ok(revision) => revision,
        Err(error) => return repository_error(error),
    };
    let approval_id = body
        .get("approval_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "approval:{}",
                artifact_fingerprint(execution.atomic_write.operation.key.as_str().as_bytes())
            )
        });
    let required_quorum = body
        .get("required_quorum")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(1);
    let expires_at = body
        .get("expires_at_unix_ms")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| runtime_gateway_now_unix_ms().saturating_add(86_400_000));
    let Ok(approval) = ApprovalRecord::pending(
        match ApprovalId::new(approval_id.clone()) {
            Ok(value) => value,
            Err(_) => return invalid_request(),
        },
        tenant_id,
        resource.approval_kind(),
        match ApprovalScope::new(
            body.get("scope")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{}/{revision_id}/{approval_id}", resource.label())),
        ) {
            Ok(value) => value,
            Err(_) => return invalid_request(),
        },
        match ApprovalFingerprint::new(revision.fingerprint) {
            Ok(value) => value,
            Err(_) => return invalid_request(),
        },
        execution.authorized_action.audit_event.principal_id,
        required_quorum,
        expires_at,
    ) else {
        return invalid_request();
    };
    let audit = match audit_command(
        repository,
        &execution.authorized_action,
        resource,
        &format!("governance.{}.approval.create", resource.label()),
        Some(&approval_id),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    match ApplicationGovernanceLifecycleService::new(repository).create_approval(
        &execution.authorized_action,
        approval,
        GovernanceMutationIdempotency {
            operation: execution.atomic_write.operation.clone(),
            started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
        },
        audit,
    ) {
        Ok(outcome) => runtime_gateway_admin_json_response(
            if outcome == GovernanceWriteOutcome::Applied {
                201
            } else {
                200
            },
            serde_json::json!({
                "object": format!("governance.{}_approval", resource.label()),
                "approval_id": approval_id,
                "revision_id": revision_id,
                "state": "pending_approval",
                "version": 1,
                "replayed": outcome == GovernanceWriteOutcome::Replayed,
            }),
        ),
        Err(error) => lifecycle_error(error),
    }
}

fn execution_approval_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    base_path: &str,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let method = captured.method.to_ascii_uppercase();
    let segments = path
        .strip_prefix(&(base_path.to_string() + "/"))
        .map(|suffix| {
            suffix
                .split('/')
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let tenant_id = base_action.tenant.tenant_id;
    match (method.as_str(), segments.as_slice()) {
        ("GET", []) => execution_approval_list_response(repository, tenant_id),
        ("GET", [approval_id]) => {
            execution_approval_get_response(repository, tenant_id, approval_id)
        }
        ("POST", [approval_id, "votes"]) => execution_approval_vote_response(
            captured,
            path,
            approval_id,
            admin_auth,
            base_action,
            tenant_id,
            repository,
        ),
        ("GET" | "POST", _) => build_runtime_proxy_json_error_response(
            404,
            "execution_approval_not_found",
            "execution approval was not found",
        ),
        _ => build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this governance route",
        ),
    }
}

fn execution_approval_list_response(
    repository: &RuntimeGovernanceRepository<'_>,
    tenant_id: prodex_domain::TenantId,
) -> tiny_http::ResponseBox {
    match repository.list_execution_approvals(tenant_id) {
        Ok(approvals) => runtime_gateway_admin_json_response(
            200,
            serde_json::json!({
                "object": "governance.execution_approval.list",
                "data": approvals.into_iter().map(execution_approval_json).collect::<Vec<_>>(),
            }),
        ),
        Err(error) => repository_error(error),
    }
}

fn execution_approval_get_response(
    repository: &RuntimeGovernanceRepository<'_>,
    tenant_id: prodex_domain::TenantId,
    approval_id: &str,
) -> tiny_http::ResponseBox {
    let approval_id = match ApprovalId::new(approval_id.to_string()) {
        Ok(approval_id) => approval_id,
        Err(_) => return invalid_request(),
    };
    match repository.get_approval(tenant_id, &approval_id) {
        Ok(approval) if approval.kind == ApprovalKind::Execution => {
            runtime_gateway_admin_json_response(200, execution_approval_json(approval))
        }
        Ok(_) => repository_error(GovernanceRepositoryError::NotFound),
        Err(error) => repository_error(error),
    }
}

fn execution_approval_vote_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    approval_id: &str,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    tenant_id: prodex_domain::TenantId,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let Some(expected_version) = body
        .get("expected_version")
        .and_then(serde_json::Value::as_u64)
    else {
        return invalid_request();
    };
    let action = match body.get("decision").and_then(serde_json::Value::as_str) {
        Some("approve") => ApprovalAction::Approve,
        Some("reject") => ApprovalAction::Reject,
        Some("cancel") => ApprovalAction::Cancel,
        _ => return invalid_request(),
    };
    let approval_id = match ApprovalId::new(approval_id.to_string()) {
        Ok(approval_id) => approval_id,
        Err(_) => return invalid_request(),
    };
    let execution = match execution(captured, path, admin_auth, base_action) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let audit_action = match action {
        ApprovalAction::Approve => "governance.execution_approval.approve",
        ApprovalAction::Reject => "governance.execution_approval.reject",
        ApprovalAction::Cancel => "governance.execution_approval.cancel",
        _ => unreachable!(),
    };
    let audit = match execution_approval_audit_command(
        repository,
        &execution.authorized_action,
        &approval_id,
        audit_action,
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    let reason = match action {
        ApprovalAction::Reject => Some(ApprovalReasonCode::new("approval.rejected").unwrap()),
        ApprovalAction::Cancel => Some(ApprovalReasonCode::new("approval.cancelled").unwrap()),
        _ => None,
    };
    match ApplicationExecutionApprovalService::new(repository).review_idempotent(
        ApprovalVoteRequest {
            tenant_id,
            approval_id: approval_id.clone(),
            actor: actor(&execution.authorized_action),
            expected_version,
            now_unix_ms: execution.atomic_write.completed_at_unix_ms,
            reason,
            audit_outbox: audit,
        },
        action,
        ApprovalVoteIdempotency {
            operation: execution.atomic_write.operation.clone(),
            started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
        },
    ) {
        Ok(ApprovalVoteMutationOutcome::Applied(approval)) => {
            runtime_gateway_admin_json_response(200, execution_approval_json(approval))
        }
        Ok(ApprovalVoteMutationOutcome::Replayed(snapshot)) => runtime_gateway_admin_json_response(
            200,
            execution_approval_snapshot_json(&approval_id, snapshot),
        ),
        Err(ApplicationGovernanceLifecycleError::Repository(error)) => repository_error(error),
        Err(_) => repository_error(GovernanceRepositoryError::InvalidInput),
    }
}

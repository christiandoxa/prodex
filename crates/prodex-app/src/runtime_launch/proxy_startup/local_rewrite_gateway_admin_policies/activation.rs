use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn activation_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    revision_id: &str,
    action: &str,
    resource: RuntimeGovernanceResource,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let approval_id = match runtime_gateway_activation_approval_id(captured, action) {
        Ok(approval_id) => approval_id,
        Err(response) => return response,
    };
    let execution = match execution(captured, path, admin_auth, base_action) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let Some(entity_tag) = execution.entity_tag.as_ref() else {
        return build_runtime_proxy_json_error_response(
            428,
            "control_plane_if_match_required",
            "If-Match is required for governance lifecycle mutation",
        );
    };
    let activation_action = match action {
        "activate" => GovernanceActivationAction::Activate,
        "rollback" => GovernanceActivationAction::Rollback,
        "revoke" => GovernanceActivationAction::Revoke,
        _ => return invalid_request(),
    };
    let audit_action = format!(
        "governance.{}.revision.{}",
        resource.label(),
        activation_action.as_str()
    );
    let audit = match audit_command(
        repository,
        &execution.authorized_action,
        resource,
        &audit_action,
        Some(revision_id),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    let expected_etag = (entity_tag.as_str() != "*").then(|| entity_tag.as_str().to_string());
    let activation = runtime_gateway_activation_result(RuntimeGatewayActivationContext {
        shared,
        repository,
        execution: &execution,
        resource,
        revision_id,
        action,
        approval_id,
        activation_action,
        expected_etag,
        audit,
    });
    runtime_gateway_activation_outcome_response(resource, action, activation)
}

fn runtime_gateway_activation_approval_id(
    captured: &RuntimeProxyRequest,
    action: &str,
) -> Result<Option<ApprovalId>, tiny_http::ResponseBox> {
    if action == "revoke" {
        return Ok(None);
    }
    let body = runtime_gateway_admin_json_body(captured)?;
    let Some(approval_id) = body.get("approval_id").and_then(serde_json::Value::as_str) else {
        return Err(invalid_request());
    };
    ApprovalId::new(approval_id.to_string())
        .map(Some)
        .map_err(|_| invalid_request())
}

fn runtime_gateway_activation_result(
    activation_context: RuntimeGatewayActivationContext<'_>,
) -> Result<GovernanceActivationResult, GovernanceRepositoryError> {
    let tenant_id = activation_context
        .execution
        .authorized_action
        .tenant
        .tenant_id;
    let shared = activation_context.shared;
    let activate = || runtime_gateway_activate_revision(&activation_context);
    let activation = match shared.governance_authority.as_ref() {
        Some(authority) => authority.commit_for_tenant(tenant_id, activate),
        None => activate(),
    };
    activation.and_then(|result| {
        match shared.refresh_committed_governance_artifact_kind(
            tenant_id,
            activation_context.resource.kind(),
        ) {
            Ok(_) => Ok(result),
            Err(_) if activation_context.action == "revoke" => Ok(result),
            Err(error) => Err(error),
        }
    })
}

struct RuntimeGatewayActivationContext<'a> {
    shared: &'a RuntimeLocalRewriteProxyShared,
    repository: &'a RuntimeGovernanceRepository<'a>,
    execution: &'a RuntimeGatewayAdminMutationExecution,
    resource: RuntimeGovernanceResource,
    revision_id: &'a str,
    action: &'a str,
    approval_id: Option<ApprovalId>,
    activation_action: GovernanceActivationAction,
    expected_etag: Option<String>,
    audit: AuditOutboxWriteCommand,
}

fn runtime_gateway_activate_revision(
    context: &RuntimeGatewayActivationContext<'_>,
) -> Result<GovernanceActivationResult, GovernanceRepositoryError> {
    ApplicationGovernanceLifecycleService::new(context.repository)
        .activate_revision(
            &context.execution.authorized_action,
            GovernanceActivationRequest {
                tenant_id: context.execution.authorized_action.tenant.tenant_id,
                kind: context.resource.kind(),
                revision_id: context.revision_id.to_string(),
                approval_id: context.approval_id.clone(),
                actor: actor(&context.execution.authorized_action),
                action: context.activation_action,
                expected_etag: context.expected_etag.clone(),
                idempotency_key: context.execution.atomic_write.operation.key.clone(),
                request_fingerprint: context
                    .execution
                    .atomic_write
                    .operation
                    .request_fingerprint
                    .clone(),
                audit_outbox: context.audit.clone(),
                activated_at_unix_ms: context.execution.atomic_write.completed_at_unix_ms,
            },
            |input| {
                governance_artifact_validation_is_valid(context.shared, context.resource, input)
            },
        )
        .map_err(lifecycle_repository_error)
}

fn runtime_gateway_activation_outcome_response(
    resource: RuntimeGovernanceResource,
    action: &str,
    activation: Result<GovernanceActivationResult, GovernanceRepositoryError>,
) -> tiny_http::ResponseBox {
    match activation {
        Ok(result) => {
            record_policy_lifecycle(
                resource,
                policy_activation_operation(action),
                PolicyLifecycleResult::Published,
            );
            json_response_with_etag(
                200,
                serde_json::json!({
                    "object": format!(
                        "governance.{}_{}",
                        resource.label(),
                        if action == "revoke" { "revocation" } else { "activation" }
                    ),
                    "revision_id": result.revision_id,
                    "active_revision_id": result.active_revision_id,
                    "last_known_good_revision_id": result.last_known_good_revision_id,
                    "etag": result.etag,
                    "replayed": result.outcome == GovernanceWriteOutcome::Replayed,
                }),
                &result.etag,
            )
        }
        Err(error) => {
            record_policy_lifecycle(
                resource,
                policy_activation_operation(action),
                PolicyLifecycleResult::Failed,
            );
            repository_error(error)
        }
    }
}

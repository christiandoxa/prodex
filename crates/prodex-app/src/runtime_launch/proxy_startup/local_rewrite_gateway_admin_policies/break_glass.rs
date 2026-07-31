use super::{
    AppendOnlyAuditCommand, ApplicationGovernanceLifecycleService, ApprovalAction,
    ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalReasonCode, ApprovalRecord,
    ApprovalScope, ApprovalVoteIdempotency, ApprovalVoteMutationOutcome, ApprovalVoteRequest,
    ApprovalVoteSnapshot, AuditAction, AuditEventId, AuditOutboxWriteCommand, AuditResource,
    ControlPlaneActionPlan, GovernanceMutationIdempotency, GovernanceRepositoryError,
    GovernanceWriteOutcome, RuntimeGatewayAdminAuth, RuntimeGovernanceRepository,
    RuntimeGovernanceResource, RuntimeProxyRequest, TenantStorageKey, actor, approval_state,
    artifact_fingerprint, audit_command, build_runtime_proxy_json_error_response,
    compute_audit_chain_digest, control_plane_audit_command, execution, invalid_request,
    lifecycle_error, repository_error, runtime_gateway_admin_json_body,
    runtime_gateway_admin_json_response, runtime_gateway_now_unix_ms,
};

pub(super) fn break_glass_approval_response(
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
    let context = RuntimeBreakGlassResponseContext {
        captured,
        path,
        admin_auth,
        base_action,
        repository,
    };
    match (method.as_str(), segments.as_slice()) {
        ("GET", []) => break_glass_list_response(repository, tenant_id),
        ("POST", []) => break_glass_create_response(&context, tenant_id),
        ("GET", [approval_id]) => break_glass_get_response(repository, tenant_id, approval_id),
        ("POST", [approval_id, action @ ("votes" | "activate" | "revoke")]) => {
            break_glass_transition_response(
                captured,
                path,
                approval_id,
                action,
                admin_auth,
                base_action,
                repository,
            )
        }
        ("GET" | "POST", _) => build_runtime_proxy_json_error_response(
            404,
            "break_glass_approval_not_found",
            "break-glass approval was not found",
        ),
        _ => build_runtime_proxy_json_error_response(
            405,
            "control_plane_method_not_allowed",
            "HTTP method is not allowed for this break-glass route",
        ),
    }
}

struct RuntimeBreakGlassResponseContext<'a> {
    captured: &'a RuntimeProxyRequest,
    path: &'a str,
    admin_auth: &'a RuntimeGatewayAdminAuth,
    base_action: &'a ControlPlaneActionPlan,
    repository: &'a RuntimeGovernanceRepository<'a>,
}

fn break_glass_list_response(
    repository: &RuntimeGovernanceRepository<'_>,
    tenant_id: prodex_domain::TenantId,
) -> tiny_http::ResponseBox {
    match repository.list_approvals(tenant_id, ApprovalKind::BreakGlass) {
        Ok(approvals) => runtime_gateway_admin_json_response(
            200,
            serde_json::json!({
                "object": "governance.break_glass_approval.list",
                "data": approvals.into_iter().map(break_glass_approval_json).collect::<Vec<_>>(),
            }),
        ),
        Err(error) => repository_error(error),
    }
}

fn break_glass_get_response(
    repository: &RuntimeGovernanceRepository<'_>,
    tenant_id: prodex_domain::TenantId,
    approval_id: &str,
) -> tiny_http::ResponseBox {
    let approval_id = match ApprovalId::new(approval_id.to_string()) {
        Ok(approval_id) => approval_id,
        Err(_) => return invalid_request(),
    };
    match repository.get_approval(tenant_id, &approval_id) {
        Ok(approval) if approval.kind == ApprovalKind::BreakGlass => {
            runtime_gateway_admin_json_response(200, break_glass_approval_json(approval))
        }
        Ok(_) => repository_error(GovernanceRepositoryError::NotFound),
        Err(error) => repository_error(error),
    }
}

fn break_glass_create_response(
    context: &RuntimeBreakGlassResponseContext<'_>,
    tenant_id: prodex_domain::TenantId,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(context.captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let Some(approval_id) = body.get("approval_id").and_then(serde_json::Value::as_str) else {
        return invalid_request();
    };
    let reason = match body
        .get("reason_code")
        .and_then(serde_json::Value::as_str)
        .map(ApprovalReasonCode::new)
    {
        Some(Ok(reason)) => reason,
        _ => return invalid_request(),
    };
    let Some(expires_at_unix_ms) = body
        .get("expires_at_unix_ms")
        .and_then(serde_json::Value::as_u64)
    else {
        return invalid_request();
    };
    let required_quorum = body
        .get("required_quorum")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(ApprovalKind::BreakGlass.minimum_quorum());
    let now = runtime_gateway_now_unix_ms();
    if expires_at_unix_ms <= now || expires_at_unix_ms > now.saturating_add(3_600_000) {
        return invalid_request();
    }
    let approval_id = match ApprovalId::new(approval_id.to_string()) {
        Ok(approval_id) => approval_id,
        Err(_) => return invalid_request(),
    };
    let scope = match ApprovalScope::new(format!("audit_retention:{}", reason.as_str())) {
        Ok(scope) => scope,
        Err(_) => return invalid_request(),
    };
    let fingerprint = match ApprovalFingerprint::new(artifact_fingerprint(
        format!("{}:{}:{}", tenant_id, scope.as_str(), expires_at_unix_ms).as_bytes(),
    )) {
        Ok(fingerprint) => fingerprint,
        Err(_) => return invalid_request(),
    };
    let execution = match execution(
        context.captured,
        context.path,
        context.admin_auth,
        context.base_action,
    ) {
        Ok(execution) => execution,
        Err(response) => return response,
    };
    let approval = match ApprovalRecord::pending(
        approval_id.clone(),
        tenant_id,
        ApprovalKind::BreakGlass,
        scope,
        fingerprint,
        execution.authorized_action.audit_event.principal_id,
        required_quorum,
        expires_at_unix_ms,
    ) {
        Ok(approval) => approval,
        Err(_) => return invalid_request(),
    };
    let audit = match control_plane_audit_command(
        context.repository,
        &execution.authorized_action,
        "governance.break_glass_approval.create",
        "break_glass_approval",
        Some(approval_id.as_str()),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    match context.repository.create_approval_idempotent(
        approval.clone(),
        audit,
        GovernanceMutationIdempotency {
            operation: execution.atomic_write.operation,
            started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
        },
    ) {
        Ok(outcome) => runtime_gateway_admin_json_response(
            if outcome == GovernanceWriteOutcome::Applied {
                201
            } else {
                200
            },
            serde_json::json!({
                "approval": break_glass_approval_json(approval),
                "replayed": outcome == GovernanceWriteOutcome::Replayed,
            }),
        ),
        Err(error) => repository_error(error),
    }
}

#[allow(clippy::too_many_arguments)]
fn break_glass_transition_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    approval_id: &str,
    transition: &str,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
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
    let action = match transition {
        "votes" => match body.get("decision").and_then(serde_json::Value::as_str) {
            Some("approve") => ApprovalAction::Approve,
            Some("reject") => ApprovalAction::Reject,
            Some("cancel") => ApprovalAction::Cancel,
            _ => return invalid_request(),
        },
        "activate" => ApprovalAction::Activate,
        "revoke" => ApprovalAction::Supersede,
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
    let tenant_id = execution.authorized_action.tenant.tenant_id;
    match repository.get_approval(tenant_id, &approval_id) {
        Ok(approval) if approval.kind == ApprovalKind::BreakGlass => {}
        Ok(_) => return repository_error(GovernanceRepositoryError::NotFound),
        Err(error) => return repository_error(error),
    }
    let action_label = match action {
        ApprovalAction::Approve => "approve",
        ApprovalAction::Reject => "reject",
        ApprovalAction::Cancel => "cancel",
        ApprovalAction::Activate => "activate",
        ApprovalAction::Supersede => "revoke",
        ApprovalAction::RollBack => return invalid_request(),
    };
    let audit = match control_plane_audit_command(
        repository,
        &execution.authorized_action,
        &format!("governance.break_glass_approval.{action_label}"),
        "break_glass_approval",
        Some(approval_id.as_str()),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    let reason = match action {
        ApprovalAction::Reject => Some(ApprovalReasonCode::new("approval.rejected").unwrap()),
        ApprovalAction::Cancel => Some(ApprovalReasonCode::new("approval.cancelled").unwrap()),
        _ => None,
    };
    match repository.transition_approval_idempotent(
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
            operation: execution.atomic_write.operation,
            started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
        },
    ) {
        Ok(ApprovalVoteMutationOutcome::Applied(approval)) => {
            runtime_gateway_admin_json_response(200, break_glass_approval_json(approval))
        }
        Ok(ApprovalVoteMutationOutcome::Replayed(snapshot)) => runtime_gateway_admin_json_response(
            200,
            break_glass_approval_snapshot_json(&approval_id, snapshot),
        ),
        Err(error) => repository_error(error),
    }
}

fn break_glass_approval_json(approval: ApprovalRecord) -> serde_json::Value {
    let reason_code = approval.scope.as_str().strip_prefix("audit_retention:");
    serde_json::json!({
        "object": "governance.break_glass_approval",
        "approval_id": approval.id.as_str(),
        "scope": "audit_retention",
        "reason_code": reason_code,
        "state": approval_state(approval.state),
        "version": approval.version,
        "required_quorum": approval.effective_required_quorum(),
        "vote_count": approval.votes.len(),
        "expires_at_unix_ms": approval.expires_at_unix_ms,
        "activated_at_unix_ms": approval.activated_at_unix_ms,
    })
}

fn break_glass_approval_snapshot_json(
    approval_id: &ApprovalId,
    snapshot: ApprovalVoteSnapshot,
) -> serde_json::Value {
    serde_json::json!({
        "object": "governance.break_glass_approval",
        "approval_id": approval_id.as_str(),
        "state": approval_state(snapshot.state),
        "version": snapshot.version,
        "required_quorum": snapshot.required_quorum,
        "vote_count": snapshot.vote_count,
        "expires_at_unix_ms": snapshot.expires_at_unix_ms,
        "activated_at_unix_ms": snapshot.activated_at_unix_ms,
    })
}

pub(super) fn execution_approval_json(approval: ApprovalRecord) -> serde_json::Value {
    serde_json::json!({
        "object": "governance.execution_approval",
        "approval_id": approval.id.as_str(),
        "state": approval_state(approval.state),
        "version": approval.version,
        "required_quorum": approval.effective_required_quorum(),
        "vote_count": approval.votes.len(),
        "expires_at_unix_ms": approval.expires_at_unix_ms,
        "activated_at_unix_ms": approval.activated_at_unix_ms,
    })
}

pub(super) fn execution_approval_snapshot_json(
    approval_id: &ApprovalId,
    snapshot: ApprovalVoteSnapshot,
) -> serde_json::Value {
    serde_json::json!({
        "object": "governance.execution_approval",
        "approval_id": approval_id.as_str(),
        "state": approval_state(snapshot.state),
        "version": snapshot.version,
        "required_quorum": snapshot.required_quorum,
        "vote_count": snapshot.vote_count,
        "expires_at_unix_ms": snapshot.expires_at_unix_ms,
        "activated_at_unix_ms": snapshot.activated_at_unix_ms,
    })
}

pub(super) fn execution_approval_audit_command(
    repository: &RuntimeGovernanceRepository<'_>,
    action: &ControlPlaneActionPlan,
    approval_id: &ApprovalId,
    audit_action: &str,
) -> Result<AuditOutboxWriteCommand, GovernanceRepositoryError> {
    let mut event = action.audit_event.clone();
    event.action = AuditAction::new(audit_action);
    event.resource = AuditResource::new(
        "execution_approval",
        Some(approval_id.as_str().to_string()),
        Some(event.tenant_id),
    );
    let previous_digest = repository.latest_audit_digest(event.tenant_id)?;
    let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), &event);
    Ok(AuditOutboxWriteCommand {
        outbox_event_id: AuditEventId::new(),
        audit: AppendOnlyAuditCommand {
            storage_key: TenantStorageKey::tenant(event.tenant_id),
            event,
            previous_digest,
            event_digest,
        },
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn vote_response(
    captured: &RuntimeProxyRequest,
    path: &str,
    revision_id: &str,
    approval_id: &str,
    resource: RuntimeGovernanceResource,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
    repository: &RuntimeGovernanceRepository<'_>,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let decision = match body.get("decision").and_then(serde_json::Value::as_str) {
        Some("approve") => ApprovalAction::Approve,
        Some("reject") => ApprovalAction::Reject,
        _ => return invalid_request(),
    };
    let Some(expected_version) = body
        .get("expected_version")
        .and_then(serde_json::Value::as_u64)
    else {
        return invalid_request();
    };
    let approval_id = match ApprovalId::new(approval_id.to_string()) {
        Ok(value) => value,
        Err(_) => return invalid_request(),
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
    let approval = match repository.get_approval(tenant_id, &approval_id) {
        Ok(approval) => approval,
        Err(error) => return repository_error(error),
    };
    if approval.kind != resource.approval_kind()
        || approval.fingerprint.as_str() != revision.fingerprint
    {
        return repository_error(GovernanceRepositoryError::NotFound);
    }
    let audit_action = match decision {
        ApprovalAction::Approve => {
            format!("governance.{}.approval.approve", resource.label())
        }
        _ => format!("governance.{}.approval.reject", resource.label()),
    };
    let audit = match audit_command(
        repository,
        &execution.authorized_action,
        resource,
        &audit_action,
        Some(approval_id.as_str()),
    ) {
        Ok(audit) => audit,
        Err(error) => return repository_error(error),
    };
    let actor = actor(&execution.authorized_action);
    match ApplicationGovernanceLifecycleService::new(repository).transition_approval(
        &execution.authorized_action,
        resource.kind(),
        ApprovalVoteRequest {
            tenant_id,
            approval_id: approval_id.clone(),
            actor,
            expected_version,
            now_unix_ms: execution.atomic_write.completed_at_unix_ms,
            reason: None,
            audit_outbox: audit,
        },
        decision,
        GovernanceMutationIdempotency {
            operation: execution.atomic_write.operation.clone(),
            started_at_unix_ms: execution.atomic_write.started_at_unix_ms,
        },
    ) {
        Ok(ApprovalVoteMutationOutcome::Applied(approval)) => runtime_gateway_admin_json_response(
            200,
            serde_json::json!({
                "object": format!("governance.{}_approval", resource.label()),
                "state": approval_state(approval.state),
                "version": approval.version,
                "vote_count": approval.votes.len(),
            }),
        ),
        Ok(ApprovalVoteMutationOutcome::Replayed(snapshot)) => runtime_gateway_admin_json_response(
            200,
            serde_json::json!({
                "object": format!("governance.{}_approval", resource.label()),
                "state": approval_state(snapshot.state),
                "version": snapshot.version,
                "vote_count": snapshot.vote_count,
            }),
        ),
        Err(error) => lifecycle_error(error),
    }
}

use super::*;

pub(super) fn execution(
    captured: &RuntimeProxyRequest,
    path: &str,
    admin_auth: &RuntimeGatewayAdminAuth,
    base_action: &ControlPlaneActionPlan,
) -> Result<
    super::local_rewrite_gateway_admin_execution::RuntimeGatewayAdminMutationExecution,
    tiny_http::ResponseBox,
> {
    runtime_gateway_admin_mutation_execution(
        captured,
        path,
        admin_auth,
        base_action,
        base_action.operation,
    )
}

pub(super) fn audit_command(
    repository: &RuntimeGovernanceRepository<'_>,
    action: &ControlPlaneActionPlan,
    resource: RuntimeGovernanceResource,
    audit_action: &str,
    resource_id: Option<&str>,
) -> Result<AuditOutboxWriteCommand, GovernanceRepositoryError> {
    let mut event = action.audit_event.clone();
    event.action = AuditAction::new(audit_action);
    event.resource = AuditResource::new(
        format!("governance_{}_revision", resource.label()),
        resource_id.map(str::to_string),
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

pub(super) fn control_plane_audit_command(
    repository: &RuntimeGovernanceRepository<'_>,
    action: &ControlPlaneActionPlan,
    audit_action: &str,
    resource_kind: &str,
    resource_id: Option<&str>,
) -> Result<AuditOutboxWriteCommand, GovernanceRepositoryError> {
    let mut event = action.audit_event.clone();
    event.action =
        AuditAction::try_new(audit_action).map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    event.resource = AuditResource::new(
        resource_kind,
        resource_id.map(str::to_string),
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

pub(super) fn append_control_plane_audit_command(
    repository: &RuntimeGovernanceRepository<'_>,
    action: &ControlPlaneActionPlan,
    audit_action: &str,
    resource_kind: &str,
    resource_id: Option<&str>,
) -> Result<(), GovernanceRepositoryError> {
    let mut result = Err(GovernanceRepositoryError::AuditChainConflict);
    for _ in 0..3 {
        result = repository.append_audit_outbox(control_plane_audit_command(
            repository,
            action,
            audit_action,
            resource_kind,
            resource_id,
        )?);
        if !matches!(result, Err(GovernanceRepositoryError::AuditChainConflict)) {
            break;
        }
    }
    result
}

pub(super) fn actor(action: &ControlPlaneActionPlan) -> Principal {
    Principal::new(
        action.audit_event.principal_id,
        Some(action.tenant.tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    )
}

pub(super) fn artifact_fingerprint(artifact: &[u8]) -> String {
    let digest = Sha256::digest(artifact);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

pub(super) fn revision_json(
    revision: GovernanceRevisionSummary,
    resource: RuntimeGovernanceResource,
) -> serde_json::Value {
    serde_json::json!({
        "object": format!("governance.{}_revision", resource.label()),
        "revision_id": revision.revision_id,
        "fingerprint": revision.fingerprint,
        "state": revision.lifecycle_state,
        "signature_key_id": revision.signature_key_id,
        "created_at_unix_ms": revision.created_at_unix_ms,
    })
}

pub(super) fn approval_state(state: prodex_domain::ApprovalState) -> &'static str {
    match state {
        prodex_domain::ApprovalState::Draft => "draft",
        prodex_domain::ApprovalState::PendingApproval => "pending_approval",
        prodex_domain::ApprovalState::Approved => "approved",
        prodex_domain::ApprovalState::Rejected => "rejected",
        prodex_domain::ApprovalState::Expired => "expired",
        prodex_domain::ApprovalState::Cancelled => "cancelled",
        prodex_domain::ApprovalState::Active => "active",
        prodex_domain::ApprovalState::Superseded => "superseded",
        prodex_domain::ApprovalState::RolledBack => "rolled_back",
    }
}

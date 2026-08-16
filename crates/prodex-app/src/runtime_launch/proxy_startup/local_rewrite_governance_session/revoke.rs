use super::RuntimeGovernanceAuthority;
use prodex_domain::{
    AuditAction, AuditEvent, AuditEventId, AuditOutcome, AuditResource, Principal, TenantContext,
    compute_audit_chain_digest,
};
use prodex_storage::{
    AppendOnlyAuditCommand, AuditOutboxWriteCommand, GovernanceMutationIdempotency,
    GovernanceRepositoryError, GovernanceSessionRevokeCommand, GovernanceWriteOutcome,
    TenantStorageKey,
};

pub(super) fn runtime_gateway_governance_session_revoke(
    authority: &RuntimeGovernanceAuthority,
    sqlite: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    tenant: TenantContext,
    session_id_hash: String,
    actor: Principal,
    reason_code: String,
    idempotency: Option<GovernanceMutationIdempotency>,
) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
    for _ in 0..3 {
        let previous_digest = match authority {
            RuntimeGovernanceAuthority::Sqlite { .. } => sqlite
                .ok_or(GovernanceRepositoryError::Database)?
                .latest_audit_digest(tenant.tenant_id)?,
            RuntimeGovernanceAuthority::Postgres {
                repository,
                runtime,
                ..
            } => runtime.block_on(repository.governance_latest_audit_digest(tenant.tenant_id))?,
        };
        let occurred_at_unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
        let event = AuditEvent::new(
            occurred_at_unix_ms,
            tenant,
            &actor,
            AuditAction::try_new("governance.session.revoke")
                .map_err(|_| GovernanceRepositoryError::InvalidInput)?,
            AuditResource::new(
                "governance_session",
                Some(session_id_hash.clone()),
                Some(tenant.tenant_id),
            ),
            AuditOutcome::Success,
            Some(reason_code.clone()),
        );
        let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), &event);
        let command = GovernanceSessionRevokeCommand {
            tenant_id: tenant.tenant_id,
            session_id_hash: session_id_hash.clone(),
            revoked_at_unix_ms: occurred_at_unix_ms,
            reason_code: reason_code.clone(),
            audit_outbox: AuditOutboxWriteCommand {
                outbox_event_id: AuditEventId::new(),
                audit: AppendOnlyAuditCommand {
                    storage_key: TenantStorageKey::tenant(tenant.tenant_id),
                    event,
                    previous_digest,
                    event_digest,
                },
            },
        };
        let result = match authority {
            RuntimeGovernanceAuthority::Sqlite { .. } => {
                let repository = sqlite.ok_or(GovernanceRepositoryError::Database)?;
                match idempotency.as_ref() {
                    Some(idempotency) => repository
                        .governance_revoke_session_idempotent(command, idempotency.clone()),
                    None => repository.governance_revoke_session(command),
                }
            }
            RuntimeGovernanceAuthority::Postgres {
                repository,
                runtime,
                ..
            } => match idempotency.as_ref() {
                Some(idempotency) => runtime.block_on(
                    repository.governance_revoke_session_idempotent(command, idempotency.clone()),
                ),
                None => runtime.block_on(repository.governance_revoke_session(command)),
            },
        };
        match result {
            Err(GovernanceRepositoryError::AuditChainConflict) => continue,
            result => return result,
        }
    }
    Err(GovernanceRepositoryError::AuditChainConflict)
}

use super::*;

pub(super) fn revision_matches_command(
    existing: &RevisionRow,
    command: &GovernanceRevisionWriteCommand,
    checksum: &str,
) -> bool {
    existing.checksum == checksum
        && existing.compiled_artifact == command.compiled_artifact
        && existing.authenticity == command.authenticity
        && existing.created_by == command.created_by
        && existing.created_at_unix_ms == command.created_at_unix_ms
}

pub(super) fn approval_is_new(approval: &ApprovalRecord) -> bool {
    approval.state == ApprovalState::PendingApproval
        && approval.version == 1
        && approval.votes.is_empty()
}

pub(super) async fn complete_approval_idempotency_if_present(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    idempotency: Option<&GovernanceMutationIdempotency>,
    completed_at_unix_ms: u64,
) -> Result<(), GovernanceRepositoryError> {
    let Some(idempotency) = idempotency else {
        return Ok(());
    };
    complete_governance_idempotency_postgres(
        transaction,
        tenant_id,
        idempotency,
        GOVERNANCE_APPROVAL_CREATE_IDEMPOTENCY_RESPONSE,
        completed_at_unix_ms,
    )
    .await
}

pub(super) fn governance_audit_export_record(
    row: &Row,
) -> Result<GovernanceAuditExportRecord, GovernanceRepositoryError> {
    Ok(GovernanceAuditExportRecord {
        audit_event_id: row.get::<_, Uuid>(0).to_string(),
        occurred_at_unix_ms: from_i64(row.get(1))?,
        principal_id: row.get::<_, Uuid>(2).to_string(),
        action: row.get(3),
        resource_kind: row.get(4),
        resource_id: row.get(5),
        outcome: row.get(6),
        reason_code: row.get(7),
        reason_detail: row.get(8),
        previous_digest: row.get(9),
        event_digest: row.get(10),
    })
}

use super::*;

pub(super) async fn latest_audit_digest_tx(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
) -> Result<Option<String>, GovernanceRepositoryError> {
    transaction
        .query_opt(
            "SELECT audit.event_digest
             FROM prodex_audit_log audit
             WHERE audit.tenant_id = $1
               AND NOT EXISTS (
                   SELECT 1 FROM prodex_audit_log child
                   WHERE child.tenant_id = audit.tenant_id
                     AND child.previous_digest = audit.event_digest
               )
             ORDER BY audit.occurred_at_unix_ms DESC, audit.audit_event_id DESC
             LIMIT 1",
            &[&tenant_id.as_uuid()],
        )
        .await
        .map_err(database_error)
        .map(|row| row.map(|row| row.get(0)))
}

pub(super) async fn append_audit_outbox_tx(
    transaction: &Transaction<'_>,
    command: AuditOutboxWriteCommand,
) -> Result<(), GovernanceRepositoryError> {
    let plan =
        plan_audit_outbox_write(command).map_err(|_| GovernanceRepositoryError::TenantMismatch)?;
    let envelope = plan.audit.envelope;
    let tenant_id = envelope.event.tenant_id;
    let tenant_lock = tenant_id.to_string();
    transaction
        .query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            &[&tenant_lock],
        )
        .await
        .map_err(database_error)?;
    let current = latest_audit_digest_tx(transaction, tenant_id).await?;
    if current.as_deref()
        != envelope
            .previous_digest
            .as_ref()
            .map(|value| value.as_str())
    {
        return Err(GovernanceRepositoryError::AuditChainConflict);
    }
    let event_envelope =
        serde_json::to_value(&envelope).map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    let occurred_at = to_i64(envelope.event.occurred_at_unix_ms)?;
    let statement = transaction
        .prepare_cached(APPEND_AUDIT_OUTBOX_ATOMIC_STATEMENT.sql)
        .await
        .map_err(database_error)?;
    transaction
        .query_one(
            &statement,
            &[
                &tenant_id.as_uuid(),
                &envelope.event.id.as_uuid(),
                &envelope
                    .previous_digest
                    .as_ref()
                    .map(|value| value.as_str()),
                &envelope.event_digest.as_str(),
                &occurred_at,
                &envelope.event.principal_id.as_uuid(),
                &envelope.event.action.as_str(),
                &envelope.event.resource.kind,
                &envelope.event.resource.id,
                &envelope.event.outcome.as_str(),
                &envelope.event.reason_code,
                &plan.outbox_event_id.as_uuid(),
                &event_envelope,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

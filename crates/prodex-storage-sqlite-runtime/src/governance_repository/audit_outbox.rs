use super::*;

pub(super) fn append_audit_outbox_tx(
    transaction: &Transaction<'_>,
    command: AuditOutboxWriteCommand,
) -> Result<(), GovernanceRepositoryError> {
    let plan =
        plan_audit_outbox_write(command).map_err(|_| GovernanceRepositoryError::TenantMismatch)?;
    let envelope = plan.audit.envelope;
    let tenant_id = envelope.event.tenant_id.to_string();
    let expected_previous = envelope
        .previous_digest
        .as_ref()
        .map(|value| value.as_str());
    let mut statement = transaction
        .prepare(
            "SELECT audit.event_digest
             FROM prodex_audit_log audit
             WHERE audit.tenant_id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM prodex_audit_log child
                   WHERE child.tenant_id = audit.tenant_id
                     AND child.previous_digest = audit.event_digest
               )
             ORDER BY audit.occurred_at_unix_ms DESC, audit.audit_event_id DESC
             LIMIT 2",
        )
        .map_err(database_error)?;
    let mut rows = statement.query([&tenant_id]).map_err(database_error)?;
    let current = rows
        .next()
        .map_err(database_error)?
        .map(|row| row.get::<_, String>(0))
        .transpose()
        .map_err(database_error)?;
    if rows.next().map_err(database_error)?.is_some() || current.as_deref() != expected_previous {
        return Err(GovernanceRepositoryError::AuditChainConflict);
    }
    drop(rows);
    drop(statement);
    let event_envelope =
        serde_json::to_string(&envelope).map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    transaction
        .execute(
            "INSERT INTO prodex_audit_log (
                tenant_id, audit_event_id, previous_digest, event_digest,
                occurred_at_unix_ms, principal_id, action, resource_kind,
                resource_id, outcome, reason_code
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                tenant_id,
                envelope.event.id.to_string(),
                envelope
                    .previous_digest
                    .as_ref()
                    .map(|value| value.as_str()),
                envelope.event_digest.as_str(),
                to_i64(envelope.event.occurred_at_unix_ms)?,
                envelope.event.principal_id.to_string(),
                envelope.event.action.as_str(),
                envelope.event.resource.kind,
                envelope.event.resource.id,
                envelope.event.outcome.as_str(),
                envelope.event.reason_code,
            ],
        )
        .map_err(database_error)?;
    transaction
        .execute(
            "INSERT INTO prodex_siem_outbox (
                tenant_id, event_id, audit_event_id, event_envelope,
                attempt_count, next_attempt_at_unix_ms, created_at_unix_ms,
                delivered_at_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5, NULL)",
            params![
                envelope.event.tenant_id.to_string(),
                plan.outbox_event_id.to_string(),
                envelope.event.id.to_string(),
                event_envelope,
                to_i64(envelope.event.occurred_at_unix_ms)?,
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

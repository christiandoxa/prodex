use super::*;

pub(super) struct RawGovernanceSessionRow {
    session_id_hash: String,
    principal_id: String,
    channel: String,
    credential_scope: String,
    classification: String,
    policy_revision_id: String,
    provider_registry_revision: String,
    provider_descriptor_revision: i64,
    provider_affinity: Option<String>,
    created_at_unix_ms: i64,
    last_seen_at_unix_ms: i64,
    absolute_expires_at_unix_ms: i64,
    idle_expires_at_unix_ms: i64,
    revoked_at_unix_ms: Option<i64>,
    revocation_reason_code: Option<String>,
}

pub(super) fn governance_session_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RawGovernanceSessionRow> {
    Ok(RawGovernanceSessionRow {
        session_id_hash: row.get(0)?,
        principal_id: row.get(1)?,
        channel: row.get(2)?,
        credential_scope: row.get(3)?,
        classification: row.get(4)?,
        policy_revision_id: row.get(5)?,
        provider_registry_revision: row.get(6)?,
        provider_descriptor_revision: row.get(7)?,
        provider_affinity: row.get(8)?,
        created_at_unix_ms: row.get(9)?,
        last_seen_at_unix_ms: row.get(10)?,
        absolute_expires_at_unix_ms: row.get(11)?,
        idle_expires_at_unix_ms: row.get(12)?,
        revoked_at_unix_ms: row.get(13)?,
        revocation_reason_code: row.get(14)?,
    })
}

pub(super) fn governance_session_from_values(
    tenant_id: TenantId,
    row: RawGovernanceSessionRow,
) -> Result<GovernanceSessionRecord, GovernanceRepositoryError> {
    Ok(GovernanceSessionRecord {
        tenant_id,
        session_id_hash: row.session_id_hash,
        principal_id: PrincipalId::from_str(&row.principal_id)
            .map_err(|_| GovernanceRepositoryError::Database)?,
        channel: channel_from_label(&row.channel)?,
        credential_scope: credential_scope_from_label(&row.credential_scope)?,
        classification: classification_from_label(&row.classification)?,
        policy_revision_id: PolicyRevisionId::from_str(&row.policy_revision_id)
            .map_err(|_| GovernanceRepositoryError::Database)?,
        provider_registry_revision: row.provider_registry_revision,
        provider_descriptor_revision: from_i64(row.provider_descriptor_revision)?,
        provider_affinity: row.provider_affinity,
        created_at_unix_ms: from_i64(row.created_at_unix_ms)?,
        last_seen_at_unix_ms: from_i64(row.last_seen_at_unix_ms)?,
        absolute_expires_at_unix_ms: from_i64(row.absolute_expires_at_unix_ms)?,
        idle_expires_at_unix_ms: from_i64(row.idle_expires_at_unix_ms)?,
        revoked_at_unix_ms: row.revoked_at_unix_ms.map(from_i64).transpose()?,
        revocation_reason_code: row.revocation_reason_code,
    })
}

pub(super) fn load_governance_session_tx(
    connection: &Connection,
    tenant_id: TenantId,
    session_id_hash: &str,
) -> Result<Option<GovernanceSessionRecord>, GovernanceRepositoryError> {
    connection
        .query_row(
            "SELECT session.session_id_hash, session.principal_id, session.channel,
                    session.credential_scope, session.classification,
                    session.policy_revision_id, session.provider_registry_revision,
                    session.provider_descriptor_revision, session.provider_affinity,
                    session.created_at_unix_ms,
                    session.last_seen_at_unix_ms, session.absolute_expires_at_unix_ms,
                    session.idle_expires_at_unix_ms, revocation.revoked_at_unix_ms,
                    revocation.reason_code
             FROM prodex_governance_sessions session
             LEFT JOIN prodex_session_revocations revocation
               ON revocation.tenant_id = session.tenant_id
              AND revocation.session_id_hash = session.session_id_hash
             WHERE session.tenant_id = ?1 AND session.session_id_hash = ?2",
            params![tenant_id.to_string(), session_id_hash],
            governance_session_row,
        )
        .optional()
        .map_err(database_error)?
        .map(|row| governance_session_from_values(tenant_id, row))
        .transpose()
}

pub(super) fn count_concurrent_sessions_tx(
    connection: &Connection,
    tenant_id: TenantId,
    principal_id: PrincipalId,
    now_unix_ms: u64,
) -> Result<u64, GovernanceRepositoryError> {
    let count = connection
        .query_row(
            "SELECT COUNT(*) FROM prodex_governance_sessions session
             WHERE session.tenant_id = ?1 AND session.principal_id = ?2
               AND session.absolute_expires_at_unix_ms > ?3
               AND session.idle_expires_at_unix_ms > ?3
               AND NOT EXISTS (
                   SELECT 1 FROM prodex_session_revocations revocation
                   WHERE revocation.tenant_id = session.tenant_id
                     AND revocation.session_id_hash = session.session_id_hash
               )",
            params![
                tenant_id.to_string(),
                principal_id.to_string(),
                to_i64(now_unix_ms)?
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(database_error)?;
    from_i64(count)
}

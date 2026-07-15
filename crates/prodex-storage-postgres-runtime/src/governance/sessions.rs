use super::*;

pub(super) async fn load_governance_session_tx(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    session_id_hash: &str,
) -> Result<Option<GovernanceSessionRecord>, GovernanceRepositoryError> {
    transaction
        .query_opt(
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
             WHERE session.tenant_id = $1 AND session.session_id_hash = $2
             FOR UPDATE OF session",
            &[&tenant_id.as_uuid(), &session_id_hash],
        )
        .await
        .map_err(database_error)?
        .map(|row| governance_session_from_row(tenant_id, &row))
        .transpose()
}

pub(super) fn governance_session_from_row(
    tenant_id: TenantId,
    row: &Row,
) -> Result<GovernanceSessionRecord, GovernanceRepositoryError> {
    Ok(GovernanceSessionRecord {
        tenant_id,
        session_id_hash: row.get(0),
        principal_id: PrincipalId::from_uuid(row.get::<_, Uuid>(1)),
        channel: channel_from_label(row.get::<_, &str>(2))?,
        credential_scope: credential_scope_from_label(row.get::<_, &str>(3))?,
        classification: classification_from_label(row.get::<_, &str>(4))?,
        policy_revision_id: PolicyRevisionId::from_uuid(row.get::<_, Uuid>(5)),
        provider_registry_revision: row.get(6),
        provider_descriptor_revision: from_i64(row.get(7))?,
        provider_affinity: row.get(8),
        created_at_unix_ms: from_i64(row.get(9))?,
        last_seen_at_unix_ms: from_i64(row.get(10))?,
        absolute_expires_at_unix_ms: from_i64(row.get(11))?,
        idle_expires_at_unix_ms: from_i64(row.get(12))?,
        revoked_at_unix_ms: row.get::<_, Option<i64>>(13).map(from_i64).transpose()?,
        revocation_reason_code: row.get(14),
    })
}

pub(super) async fn count_concurrent_sessions_tx(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    principal_id: PrincipalId,
    now_unix_ms: u64,
) -> Result<u64, GovernanceRepositoryError> {
    let now = to_i64(now_unix_ms)?;
    let count = transaction
        .query_one(
            "SELECT COUNT(*) FROM prodex_governance_sessions session
             WHERE session.tenant_id = $1 AND session.principal_id = $2
               AND session.absolute_expires_at_unix_ms > $3
               AND session.idle_expires_at_unix_ms > $3
               AND NOT EXISTS (
                   SELECT 1 FROM prodex_session_revocations revocation
                   WHERE revocation.tenant_id = session.tenant_id
                     AND revocation.session_id_hash = session.session_id_hash
               )",
            &[&tenant_id.as_uuid(), &principal_id.as_uuid(), &now],
        )
        .await
        .map_err(database_error)?
        .get::<_, i64>(0);
    from_i64(count)
}

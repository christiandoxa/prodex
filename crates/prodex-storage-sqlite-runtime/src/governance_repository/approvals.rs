use super::*;

pub(super) fn persist_approval_transition(
    transaction: &Transaction<'_>,
    previous: &ApprovalRecord,
    next: &ApprovalRecord,
) -> Result<(), GovernanceRepositoryError> {
    let changed = transaction
        .execute(
            "UPDATE prodex_approvals
             SET lifecycle_state = ?4, activated_at_unix_ms = ?5,
                 termination_reason = ?6, resource_version = ?7
             WHERE tenant_id = ?1 AND approval_id = ?2 AND resource_version = ?3",
            params![
                next.tenant_id.to_string(),
                next.id.as_str(),
                to_i64(previous.version)?,
                approval_state_label(next.state),
                next.activated_at_unix_ms.map(to_i64).transpose()?,
                next.termination_reason
                    .as_ref()
                    .map(ApprovalReasonCode::as_str),
                to_i64(next.version)?,
            ],
        )
        .map_err(database_error)?;
    if changed != 1 {
        return Err(GovernanceRepositoryError::Conflict);
    }
    for vote in &next.votes {
        transaction
            .execute(
                "INSERT OR IGNORE INTO prodex_approval_votes (
                    tenant_id, approval_id, checker_id, approved_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    next.tenant_id.to_string(),
                    next.id.as_str(),
                    vote.checker.to_string(),
                    to_i64(vote.approved_at_unix_ms)?,
                ],
            )
            .map_err(database_error)?;
    }
    Ok(())
}

pub(super) fn load_approval_tx(
    connection: &Connection,
    tenant_id: TenantId,
    approval_id: &ApprovalId,
) -> Result<Option<ApprovalRecord>, GovernanceRepositoryError> {
    let row = connection
        .query_row(
            "SELECT approval_kind, approval_scope, fingerprint, maker_id, lifecycle_state,
                    required_quorum, expires_at_unix_ms, activated_at_unix_ms,
                    termination_reason, resource_version
             FROM prodex_approvals WHERE tenant_id = ?1 AND approval_id = ?2",
            params![tenant_id.to_string(), approval_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            },
        )
        .optional()
        .map_err(database_error)?;
    let Some((kind, scope, fingerprint, maker, state, quorum, expires, activated, reason, version)) =
        row
    else {
        return Ok(None);
    };
    let mut statement = connection
        .prepare(
            "SELECT checker_id, approved_at_unix_ms FROM prodex_approval_votes
             WHERE tenant_id = ?1 AND approval_id = ?2 ORDER BY checker_id",
        )
        .map_err(database_error)?;
    let vote_rows = statement
        .query_map(
            params![tenant_id.to_string(), approval_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(database_error)?;
    let mut votes = Vec::new();
    for vote in vote_rows {
        let (checker, approved_at) = vote.map_err(database_error)?;
        votes.push(ApprovalVote {
            checker: PrincipalId::from_str(&checker)
                .map_err(|_| GovernanceRepositoryError::Database)?,
            approved_at_unix_ms: from_i64(approved_at)?,
        });
    }
    Ok(Some(ApprovalRecord {
        id: approval_id.clone(),
        tenant_id,
        kind: approval_kind_from_label(&kind)?,
        scope: ApprovalScope::new(scope).map_err(|_| GovernanceRepositoryError::Database)?,
        fingerprint: ApprovalFingerprint::new(fingerprint)
            .map_err(|_| GovernanceRepositoryError::Database)?,
        maker: PrincipalId::from_str(&maker).map_err(|_| GovernanceRepositoryError::Database)?,
        state: approval_state_from_label(&state)?,
        required_quorum: u8::try_from(quorum).map_err(|_| GovernanceRepositoryError::Database)?,
        votes,
        expires_at_unix_ms: from_i64(expires)?,
        activated_at_unix_ms: activated.map(from_i64).transpose()?,
        termination_reason: reason
            .map(ApprovalReasonCode::new)
            .transpose()
            .map_err(|_| GovernanceRepositoryError::Database)?,
        version: from_i64(version)?,
    }))
}

pub(super) fn approval_idempotency_replay_sqlite(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    idempotency: &ApprovalVoteIdempotency,
) -> Result<IdempotencyReplayDecision<Vec<u8>>, GovernanceRepositoryError> {
    if idempotency.operation.tenant_id != tenant_id || idempotency.started_at_unix_ms == 0 {
        return Err(GovernanceRepositoryError::InvalidInput);
    }
    let row = transaction
        .query_row(
            "SELECT request_fingerprint, entry_status, started_at_unix_ms, completed_at_unix_ms, response_body
             FROM prodex_idempotency_records
             WHERE tenant_id = ?1 AND idempotency_key = ?2",
            params![tenant_id.to_string(), idempotency.operation.key.as_str()],
            |row| {
                let status = match row.get::<_, String>(1)?.as_str() {
                    "pending" => IdempotencyRecordLookupRowStatus::Pending,
                    "completed" => IdempotencyRecordLookupRowStatus::Completed,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                Ok(IdempotencyRecordLookupRow {
                    tenant_id,
                    idempotency_key: idempotency.operation.key.clone(),
                    request_fingerprint: row.get(0)?,
                    status,
                    started_at_unix_ms: from_i64(row.get(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    completed_at_unix_ms: row
                        .get::<_, Option<i64>>(3)?
                        .map(from_i64)
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    response_body: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(database_error)?;
    let entry = row
        .map(|row| materialize_idempotency_record_lookup_row(&idempotency.operation, row))
        .transpose()
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    decide_idempotency_replay(&idempotency.operation, entry.as_ref())
        .map_err(|_| GovernanceRepositoryError::Conflict)
}

pub(super) fn insert_approval_idempotency_pending_sqlite(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    idempotency: &ApprovalVoteIdempotency,
) -> Result<(), GovernanceRepositoryError> {
    let inserted = transaction
        .execute(
            "INSERT INTO prodex_idempotency_records (
                tenant_id, idempotency_key, request_fingerprint, entry_status, started_at_unix_ms
             ) VALUES (?1, ?2, ?3, 'pending', ?4)",
            params![
                tenant_id.to_string(),
                idempotency.operation.key.as_str(),
                idempotency.operation.request_fingerprint,
                to_i64(idempotency.started_at_unix_ms)?,
            ],
        )
        .map_err(database_error)?;
    if inserted != 1 {
        return Err(GovernanceRepositoryError::Conflict);
    }
    Ok(())
}

pub(super) fn complete_approval_idempotency_sqlite(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    idempotency: &ApprovalVoteIdempotency,
    outcome: ApprovalVoteStableOutcome,
    completed_at_unix_ms: u64,
) -> Result<(), GovernanceRepositoryError> {
    let updated = transaction
        .execute(
            "UPDATE prodex_idempotency_records
             SET entry_status = 'completed', completed_at_unix_ms = ?4, response_body = ?5
             WHERE tenant_id = ?1 AND idempotency_key = ?2
               AND request_fingerprint = ?3 AND entry_status = 'pending'",
            params![
                tenant_id.to_string(),
                idempotency.operation.key.as_str(),
                idempotency.operation.request_fingerprint,
                to_i64(completed_at_unix_ms)?,
                outcome.encode(),
            ],
        )
        .map_err(database_error)?;
    if updated != 1 {
        return Err(GovernanceRepositoryError::Conflict);
    }
    Ok(())
}

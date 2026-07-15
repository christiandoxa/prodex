use super::*;

pub(super) async fn persist_approval_transition(
    transaction: &Transaction<'_>,
    previous: &ApprovalRecord,
    next: &ApprovalRecord,
) -> Result<(), GovernanceRepositoryError> {
    let previous_version = to_i64(previous.version)?;
    let next_version = to_i64(next.version)?;
    let activated_at = next.activated_at_unix_ms.map(to_i64).transpose()?;
    let changed = transaction
        .execute(
            "UPDATE prodex_approvals
             SET lifecycle_state = $4, activated_at_unix_ms = $5,
                 termination_reason = $6, resource_version = $7
             WHERE tenant_id = $1 AND approval_id = $2 AND resource_version = $3",
            &[
                &next.tenant_id.as_uuid(),
                &next.id.as_str(),
                &previous_version,
                &approval_state_label(next.state),
                &activated_at,
                &next
                    .termination_reason
                    .as_ref()
                    .map(ApprovalReasonCode::as_str),
                &next_version,
            ],
        )
        .await
        .map_err(database_error)?;
    if changed != 1 {
        return Err(GovernanceRepositoryError::Conflict);
    }
    for vote in &next.votes {
        let approved_at = to_i64(vote.approved_at_unix_ms)?;
        transaction
            .execute(
                "INSERT INTO prodex_approval_votes (
                    tenant_id, approval_id, checker_id, approved_at_unix_ms
                 ) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
                &[
                    &next.tenant_id.as_uuid(),
                    &next.id.as_str(),
                    &vote.checker.as_uuid(),
                    &approved_at,
                ],
            )
            .await
            .map_err(database_error)?;
    }
    Ok(())
}

pub(super) async fn load_approval_tx(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    approval_id: &ApprovalId,
) -> Result<Option<ApprovalRecord>, GovernanceRepositoryError> {
    let row = transaction
        .query_opt(
            "SELECT approval_kind, approval_scope, fingerprint, maker_id, lifecycle_state,
                    required_quorum, expires_at_unix_ms, activated_at_unix_ms,
                    termination_reason, resource_version
             FROM prodex_approvals WHERE tenant_id = $1 AND approval_id = $2 FOR UPDATE",
            &[&tenant_id.as_uuid(), &approval_id.as_str()],
        )
        .await
        .map_err(database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let vote_rows = transaction
        .query(
            "SELECT checker_id, approved_at_unix_ms FROM prodex_approval_votes
             WHERE tenant_id = $1 AND approval_id = $2 ORDER BY checker_id",
            &[&tenant_id.as_uuid(), &approval_id.as_str()],
        )
        .await
        .map_err(database_error)?;
    let votes = vote_rows
        .into_iter()
        .map(|row| {
            Ok(ApprovalVote {
                checker: PrincipalId::from_uuid(row.get::<_, Uuid>(0)),
                approved_at_unix_ms: from_i64(row.get(1))?,
            })
        })
        .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
    approval_from_row(row, approval_id.clone(), tenant_id, votes).map(Some)
}

fn approval_from_row(
    row: Row,
    id: ApprovalId,
    tenant_id: TenantId,
    votes: Vec<ApprovalVote>,
) -> Result<ApprovalRecord, GovernanceRepositoryError> {
    let kind = approval_kind_from_label(row.get::<_, &str>(0))?;
    let scope = ApprovalScope::new(row.get::<_, String>(1))
        .map_err(|_| GovernanceRepositoryError::Database)?;
    let fingerprint = ApprovalFingerprint::new(row.get::<_, String>(2))
        .map_err(|_| GovernanceRepositoryError::Database)?;
    let state = approval_state_from_label(row.get::<_, &str>(4))?;
    Ok(ApprovalRecord {
        id,
        tenant_id,
        kind,
        scope,
        fingerprint,
        maker: PrincipalId::from_uuid(row.get::<_, Uuid>(3)),
        state,
        required_quorum: u8::try_from(row.get::<_, i16>(5))
            .map_err(|_| GovernanceRepositoryError::Database)?,
        votes,
        expires_at_unix_ms: from_i64(row.get(6))?,
        activated_at_unix_ms: row.get::<_, Option<i64>>(7).map(from_i64).transpose()?,
        termination_reason: row
            .get::<_, Option<String>>(8)
            .map(ApprovalReasonCode::new)
            .transpose()
            .map_err(|_| GovernanceRepositoryError::Database)?,
        version: from_i64(row.get(9))?,
    })
}

pub(super) async fn approval_idempotency_replay_postgres(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    idempotency: &ApprovalVoteIdempotency,
) -> Result<IdempotencyReplayDecision<Vec<u8>>, GovernanceRepositoryError> {
    if idempotency.operation.tenant_id != tenant_id || idempotency.started_at_unix_ms == 0 {
        return Err(GovernanceRepositoryError::InvalidInput);
    }
    let tenant_lock = tenant_id.to_string();
    transaction
        .query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            &[&tenant_lock],
        )
        .await
        .map_err(database_error)?;
    let row = transaction
        .query_opt(
            "SELECT request_fingerprint, entry_status, started_at_unix_ms,
                    completed_at_unix_ms, response_body
             FROM prodex_idempotency_records
             WHERE tenant_id = $1 AND idempotency_key = $2",
            &[&tenant_id.as_uuid(), &idempotency.operation.key.as_str()],
        )
        .await
        .map_err(database_error)?
        .map(|row| {
            let status = match row.get::<_, String>(1).as_str() {
                "pending" => Ok(IdempotencyRecordLookupRowStatus::Pending),
                "completed" => Ok(IdempotencyRecordLookupRowStatus::Completed),
                _ => Err(GovernanceRepositoryError::InvalidInput),
            }?;
            Ok(IdempotencyRecordLookupRow {
                tenant_id,
                idempotency_key: idempotency.operation.key.clone(),
                request_fingerprint: row.get(0),
                status,
                started_at_unix_ms: from_i64(row.get(2))?,
                completed_at_unix_ms: row.get::<_, Option<i64>>(3).map(from_i64).transpose()?,
                response_body: row.get(4),
            })
        })
        .transpose()?;
    let entry = row
        .map(|row| materialize_idempotency_record_lookup_row(&idempotency.operation, row))
        .transpose()
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    decide_idempotency_replay(&idempotency.operation, entry.as_ref())
        .map_err(|_| GovernanceRepositoryError::Conflict)
}

pub(super) async fn insert_approval_idempotency_pending_postgres(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    idempotency: &ApprovalVoteIdempotency,
) -> Result<(), GovernanceRepositoryError> {
    let inserted = transaction
        .execute(
            "INSERT INTO prodex_idempotency_records (
                tenant_id, idempotency_key, request_fingerprint, entry_status, started_at_unix_ms
             ) VALUES ($1, $2, $3, 'pending', $4)",
            &[
                &tenant_id.as_uuid(),
                &idempotency.operation.key.as_str(),
                &idempotency.operation.request_fingerprint,
                &to_i64(idempotency.started_at_unix_ms)?,
            ],
        )
        .await
        .map_err(database_error)?;
    if inserted != 1 {
        return Err(GovernanceRepositoryError::Conflict);
    }
    Ok(())
}

pub(super) async fn complete_approval_idempotency_postgres(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    idempotency: &ApprovalVoteIdempotency,
    outcome: ApprovalVoteStableOutcome,
    completed_at_unix_ms: u64,
) -> Result<(), GovernanceRepositoryError> {
    let response = outcome.encode();
    let updated = transaction
        .execute(
            "UPDATE prodex_idempotency_records
             SET entry_status = 'completed', completed_at_unix_ms = $4, response_body = $5
             WHERE tenant_id = $1 AND idempotency_key = $2
               AND request_fingerprint = $3 AND entry_status = 'pending'",
            &[
                &tenant_id.as_uuid(),
                &idempotency.operation.key.as_str(),
                &idempotency.operation.request_fingerprint,
                &to_i64(completed_at_unix_ms)?,
                &response,
            ],
        )
        .await
        .map_err(database_error)?;
    if updated != 1 {
        return Err(GovernanceRepositoryError::Conflict);
    }
    Ok(())
}

use super::*;

impl GovernanceSqliteRepository {
    pub fn upsert_audit_legal_hold(
        &self,
        hold: &AuditRetentionHold,
        created_by: PrincipalId,
        created_at_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        self.upsert_audit_legal_hold_inner(hold, created_by, created_at_unix_ms, audit, None)
    }

    pub fn upsert_audit_legal_hold_idempotent(
        &self,
        hold: &AuditRetentionHold,
        created_by: PrincipalId,
        created_at_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<(), GovernanceRepositoryError> {
        self.upsert_audit_legal_hold_inner(
            hold,
            created_by,
            created_at_unix_ms,
            audit,
            Some(idempotency),
        )
    }

    fn upsert_audit_legal_hold_inner(
        &self,
        hold: &AuditRetentionHold,
        created_by: PrincipalId,
        created_at_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
        idempotency: Option<GovernanceMutationIdempotency>,
    ) -> Result<(), GovernanceRepositoryError> {
        let completed_at_unix_ms = audit.audit.event.occurred_at_unix_ms;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_sqlite(&transaction, hold.tenant_id, idempotency)? {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_sqlite(
                        &transaction,
                        hold.tenant_id,
                        idempotency,
                    )?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().map_err(database_error)?;
                    return if response == GOVERNANCE_AUDIT_LEGAL_HOLD_UPSERT_IDEMPOTENCY_RESPONSE {
                        Ok(())
                    } else {
                        Err(GovernanceRepositoryError::InvalidInput)
                    };
                }
            }
        }
        let changed = transaction
            .execute(
                "INSERT INTO prodex_audit_legal_holds (
                    tenant_id, audit_event_id, reason_code, expires_at_unix_ms,
                    created_by, created_at_unix_ms
                 )
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6
                 FROM prodex_audit_log
                 WHERE tenant_id = ?1 AND audit_event_id = ?2
                 ON CONFLICT (tenant_id, audit_event_id) DO UPDATE SET
                    reason_code = excluded.reason_code,
                    expires_at_unix_ms = excluded.expires_at_unix_ms,
                    created_by = excluded.created_by,
                    created_at_unix_ms = excluded.created_at_unix_ms",
                params![
                    hold.tenant_id.to_string(),
                    hold.event_id.to_string(),
                    hold.reason_code.as_str(),
                    hold.expires_at
                        .map(AuditTimestamp::unix_ms)
                        .map(to_i64)
                        .transpose()?,
                    created_by.to_string(),
                    to_i64(created_at_unix_ms)?,
                ],
            )
            .map_err(database_error)?;
        if changed == 0 {
            return Err(GovernanceRepositoryError::NotFound);
        }
        append_audit_outbox_tx(&transaction, audit)?;
        if let Some(idempotency) = idempotency.as_ref() {
            complete_governance_idempotency_sqlite(
                &transaction,
                hold.tenant_id,
                idempotency,
                GOVERNANCE_AUDIT_LEGAL_HOLD_UPSERT_IDEMPOTENCY_RESPONSE,
                completed_at_unix_ms,
            )?;
        }
        transaction.commit().map_err(database_error)?;
        Ok(())
    }

    pub fn list_audit_legal_holds(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<AuditRetentionHold>, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT audit_event_id, reason_code, expires_at_unix_ms
                 FROM prodex_audit_legal_holds
                 WHERE tenant_id = ?1
                 ORDER BY audit_event_id",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map([tenant_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(database_error)?;
        rows.map(|row| {
            let (event_id, reason_code, expires_at_unix_ms) = row.map_err(database_error)?;
            Ok(AuditRetentionHold {
                tenant_id,
                event_id: AuditEventId::from_str(&event_id)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
                reason_code: AuditReasonCode::new(reason_code)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
                expires_at: expires_at_unix_ms
                    .map(from_i64)
                    .transpose()?
                    .map(AuditTimestamp::new)
                    .transpose()
                    .map_err(|_| GovernanceRepositoryError::Database)?,
            })
        })
        .collect()
    }

    pub fn delete_audit_legal_hold(
        &self,
        tenant_id: TenantId,
        event_id: AuditEventId,
        audit: AuditOutboxWriteCommand,
    ) -> Result<bool, GovernanceRepositoryError> {
        self.delete_audit_legal_hold_inner(tenant_id, event_id, audit, None)
    }

    pub fn delete_audit_legal_hold_idempotent(
        &self,
        tenant_id: TenantId,
        event_id: AuditEventId,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<bool, GovernanceRepositoryError> {
        self.delete_audit_legal_hold_inner(tenant_id, event_id, audit, Some(idempotency))
    }

    fn delete_audit_legal_hold_inner(
        &self,
        tenant_id: TenantId,
        event_id: AuditEventId,
        audit: AuditOutboxWriteCommand,
        idempotency: Option<GovernanceMutationIdempotency>,
    ) -> Result<bool, GovernanceRepositoryError> {
        let completed_at_unix_ms = audit.audit.event.occurred_at_unix_ms;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_sqlite(&transaction, tenant_id, idempotency)? {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_sqlite(
                        &transaction,
                        tenant_id,
                        idempotency,
                    )?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().map_err(database_error)?;
                    return match response.as_slice() {
                        GOVERNANCE_AUDIT_LEGAL_HOLD_DELETE_APPLIED_IDEMPOTENCY_RESPONSE => Ok(true),
                        GOVERNANCE_AUDIT_LEGAL_HOLD_DELETE_NOT_FOUND_IDEMPOTENCY_RESPONSE => {
                            Ok(false)
                        }
                        _ => Err(GovernanceRepositoryError::InvalidInput),
                    };
                }
            }
        }
        let changed = transaction
            .execute(
                "DELETE FROM prodex_audit_legal_holds
                 WHERE tenant_id = ?1 AND audit_event_id = ?2",
                params![tenant_id.to_string(), event_id.to_string()],
            )
            .map_err(database_error)?;
        if changed != 0 {
            append_audit_outbox_tx(&transaction, audit)?;
        }
        if let Some(idempotency) = idempotency.as_ref() {
            complete_governance_idempotency_sqlite(
                &transaction,
                tenant_id,
                idempotency,
                if changed == 0 {
                    GOVERNANCE_AUDIT_LEGAL_HOLD_DELETE_NOT_FOUND_IDEMPOTENCY_RESPONSE
                } else {
                    GOVERNANCE_AUDIT_LEGAL_HOLD_DELETE_APPLIED_IDEMPOTENCY_RESPONSE
                },
                completed_at_unix_ms,
            )?;
        }
        transaction.commit().map_err(database_error)?;
        Ok(changed != 0)
    }

    pub fn purge_audit_events(
        &self,
        tenant_id: TenantId,
        event_ids: &[AuditEventId],
        now_unix_ms: u64,
        cutoff_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
    ) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
        self.purge_audit_events_inner(
            tenant_id,
            event_ids,
            now_unix_ms,
            cutoff_unix_ms,
            audit,
            None,
        )
    }

    pub fn purge_audit_events_idempotent(
        &self,
        tenant_id: TenantId,
        event_ids: &[AuditEventId],
        now_unix_ms: u64,
        cutoff_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
        self.purge_audit_events_inner(
            tenant_id,
            event_ids,
            now_unix_ms,
            cutoff_unix_ms,
            audit,
            Some(idempotency),
        )
    }

    fn purge_audit_events_inner(
        &self,
        tenant_id: TenantId,
        event_ids: &[AuditEventId],
        now_unix_ms: u64,
        cutoff_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
        idempotency: Option<GovernanceMutationIdempotency>,
    ) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
        if event_ids.is_empty() || event_ids.len() > usize::from(AuditRetentionBatchLimit::MAX) {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let completed_at_unix_ms = audit.audit.event.occurred_at_unix_ms;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_sqlite(&transaction, tenant_id, idempotency)? {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_sqlite(
                        &transaction,
                        tenant_id,
                        idempotency,
                    )?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    let purged =
                        decode_governance_audit_retention_purge_idempotency_response(&response)?;
                    transaction.commit().map_err(database_error)?;
                    return Ok(purged);
                }
            }
        }
        transaction
            .execute(
                "DELETE FROM prodex_audit_legal_holds
                 WHERE tenant_id = ?1
                   AND expires_at_unix_ms IS NOT NULL
                   AND expires_at_unix_ms < ?2",
                params![tenant_id.to_string(), to_i64(now_unix_ms)?],
            )
            .map_err(database_error)?;
        let boundary_count = transaction
            .query_row(
                "SELECT COUNT(*)
                 FROM prodex_audit_log audit
                 WHERE audit.tenant_id = ?1
                   AND (
                       audit.previous_digest IS NULL OR NOT EXISTS (
                           SELECT 1 FROM prodex_audit_log parent
                           WHERE parent.tenant_id = audit.tenant_id
                             AND parent.event_digest = audit.previous_digest
                       )
                   )",
                [tenant_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(database_error)?;
        if boundary_count > 1 {
            return Err(GovernanceRepositoryError::AuditChainConflict);
        }
        let requested = event_ids
            .iter()
            .map(ToString::to_string)
            .collect::<std::collections::HashSet<_>>();
        let mut statement = transaction
            .prepare(
                "WITH RECURSIVE chain(
                    audit_event_id, event_digest, occurred_at_unix_ms, depth
                 ) AS (
                    SELECT audit.audit_event_id, audit.event_digest,
                           audit.occurred_at_unix_ms, 1
                    FROM prodex_audit_log audit
                    WHERE audit.tenant_id = ?1
                      AND (
                          audit.previous_digest IS NULL OR NOT EXISTS (
                              SELECT 1 FROM prodex_audit_log parent
                              WHERE parent.tenant_id = audit.tenant_id
                                AND parent.event_digest = audit.previous_digest
                          )
                      )
                    UNION ALL
                    SELECT child.audit_event_id, child.event_digest,
                           child.occurred_at_unix_ms, chain.depth + 1
                    FROM prodex_audit_log child
                    JOIN chain ON child.previous_digest = chain.event_digest
                    WHERE child.tenant_id = ?1
                 )
                 SELECT chain.audit_event_id, chain.event_digest,
                        chain.occurred_at_unix_ms,
                        EXISTS (
                            SELECT 1 FROM prodex_audit_legal_holds hold
                            WHERE hold.tenant_id = ?1
                              AND hold.audit_event_id = chain.audit_event_id
                        )
                 FROM chain
                 ORDER BY chain.depth
                 LIMIT ?2",
            )
            .map_err(database_error)?;
        let chain = statement
            .query_map(
                params![tenant_id.to_string(), to_i64(event_ids.len() as u64)?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, bool>(3)?,
                    ))
                },
            )
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        drop(statement);
        let eligible = chain
            .into_iter()
            .take_while(|(event_id, _, occurred_at, held)| {
                requested.contains(event_id)
                    && u64::try_from(*occurred_at).is_ok_and(|value| value < cutoff_unix_ms)
                    && !held
            })
            .map(|(event_id, digest, _, _)| (event_id, digest))
            .collect::<Vec<_>>();
        let mut purged = Vec::with_capacity(eligible.len());
        let last_purged_digest = eligible.last().map(|(_, digest)| digest.clone());
        for (event_id, _) in eligible {
            let changed = transaction
                .execute(
                    "DELETE FROM prodex_audit_log
                     WHERE tenant_id = ?1 AND audit_event_id = ?2",
                    params![tenant_id.to_string(), event_id],
                )
                .map_err(database_error)?;
            if changed != 0 {
                transaction
                    .execute(
                        "DELETE FROM prodex_siem_outbox
                         WHERE tenant_id = ?1 AND audit_event_id = ?2",
                        params![tenant_id.to_string(), event_id],
                    )
                    .map_err(database_error)?;
                transaction
                    .execute(
                        "DELETE FROM prodex_siem_dead_letters
                         WHERE tenant_id = ?1 AND audit_event_id = ?2",
                        params![tenant_id.to_string(), event_id],
                    )
                    .map_err(database_error)?;
                purged.push(
                    AuditEventId::from_str(&event_id)
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                );
            }
        }
        if let Some(last_purged_digest) = last_purged_digest {
            transaction
                .execute(
                    "INSERT INTO prodex_audit_retention_anchors (
                        tenant_id, last_purged_digest, updated_at_unix_ms
                     ) VALUES (?1, ?2, ?3)
                     ON CONFLICT (tenant_id) DO UPDATE SET
                        last_purged_digest = excluded.last_purged_digest,
                        updated_at_unix_ms = excluded.updated_at_unix_ms",
                    params![
                        tenant_id.to_string(),
                        last_purged_digest,
                        to_i64(now_unix_ms)?,
                    ],
                )
                .map_err(database_error)?;
        }
        append_audit_outbox_tx(&transaction, audit)?;
        if let Some(idempotency) = idempotency.as_ref() {
            let response = encode_governance_audit_retention_purge_idempotency_response(&purged);
            complete_governance_idempotency_sqlite(
                &transaction,
                tenant_id,
                idempotency,
                &response,
                completed_at_unix_ms,
            )?;
        }
        transaction.commit().map_err(database_error)?;
        Ok(purged)
    }
}

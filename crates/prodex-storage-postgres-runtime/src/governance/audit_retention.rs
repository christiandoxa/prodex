use super::*;

impl PostgresRepository {
    pub async fn governance_upsert_audit_legal_hold(
        &self,
        hold: AuditRetentionHold,
        created_by: PrincipalId,
        created_at_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        self.governance_timeout(self.governance_upsert_audit_legal_hold_inner(
            hold,
            created_by,
            created_at_unix_ms,
            audit,
            None,
        ))
        .await
    }

    pub async fn governance_upsert_audit_legal_hold_idempotent(
        &self,
        hold: AuditRetentionHold,
        created_by: PrincipalId,
        created_at_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<(), GovernanceRepositoryError> {
        self.governance_timeout(self.governance_upsert_audit_legal_hold_inner(
            hold,
            created_by,
            created_at_unix_ms,
            audit,
            Some(idempotency),
        ))
        .await
    }

    async fn governance_upsert_audit_legal_hold_inner(
        &self,
        hold: AuditRetentionHold,
        created_by: PrincipalId,
        created_at_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
        idempotency: Option<GovernanceMutationIdempotency>,
    ) -> Result<(), GovernanceRepositoryError> {
        let completed_at_unix_ms = audit.audit.event.occurred_at_unix_ms;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, hold.tenant_id)
            .await
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_postgres(&transaction, hold.tenant_id, idempotency)
                .await?
            {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_postgres(
                        &transaction,
                        hold.tenant_id,
                        idempotency,
                    )
                    .await?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().await.map_err(database_error)?;
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
                 SELECT $1, $2, $3, $4, $5, $6
                 FROM prodex_audit_log
                 WHERE tenant_id = $1 AND audit_event_id = $2
                 ON CONFLICT (tenant_id, audit_event_id) DO UPDATE SET
                    reason_code = excluded.reason_code,
                    expires_at_unix_ms = excluded.expires_at_unix_ms,
                    created_by = excluded.created_by,
                    created_at_unix_ms = excluded.created_at_unix_ms",
                &[
                    &hold.tenant_id.as_uuid(),
                    &hold.event_id.as_uuid(),
                    &hold.reason_code.as_str(),
                    &hold
                        .expires_at
                        .map(AuditTimestamp::unix_ms)
                        .map(to_i64)
                        .transpose()?,
                    &created_by.as_uuid(),
                    &to_i64(created_at_unix_ms)?,
                ],
            )
            .await
            .map_err(database_error)?;
        if changed == 0 {
            return Err(GovernanceRepositoryError::NotFound);
        }
        append_audit_outbox_tx(&transaction, audit).await?;
        if let Some(idempotency) = idempotency.as_ref() {
            complete_governance_idempotency_postgres(
                &transaction,
                hold.tenant_id,
                idempotency,
                GOVERNANCE_AUDIT_LEGAL_HOLD_UPSERT_IDEMPOTENCY_RESPONSE,
                completed_at_unix_ms,
            )
            .await?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(())
    }

    pub async fn governance_list_audit_legal_holds(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<AuditRetentionHold>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_list_audit_legal_holds_inner(tenant_id))
            .await
    }

    async fn governance_list_audit_legal_holds_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<AuditRetentionHold>, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "SELECT audit_event_id, reason_code, expires_at_unix_ms
                 FROM prodex_audit_legal_holds
                 WHERE tenant_id = $1
                 ORDER BY audit_event_id",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        let holds = rows
            .into_iter()
            .map(|row| {
                let expires_at_unix_ms = row.get::<_, Option<i64>>(2);
                Ok(AuditRetentionHold {
                    tenant_id,
                    event_id: AuditEventId::from_uuid(row.get(0)),
                    reason_code: AuditReasonCode::new(row.get::<_, String>(1))
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                    expires_at: expires_at_unix_ms
                        .map(from_i64)
                        .transpose()?
                        .map(AuditTimestamp::new)
                        .transpose()
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                })
            })
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        transaction.commit().await.map_err(database_error)?;
        Ok(holds)
    }

    pub async fn governance_delete_audit_legal_hold(
        &self,
        tenant_id: TenantId,
        event_id: AuditEventId,
        audit: AuditOutboxWriteCommand,
    ) -> Result<bool, GovernanceRepositoryError> {
        self.governance_timeout(
            self.governance_delete_audit_legal_hold_inner(tenant_id, event_id, audit, None),
        )
        .await
    }

    pub async fn governance_delete_audit_legal_hold_idempotent(
        &self,
        tenant_id: TenantId,
        event_id: AuditEventId,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<bool, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_delete_audit_legal_hold_inner(
            tenant_id,
            event_id,
            audit,
            Some(idempotency),
        ))
        .await
    }

    async fn governance_delete_audit_legal_hold_inner(
        &self,
        tenant_id: TenantId,
        event_id: AuditEventId,
        audit: AuditOutboxWriteCommand,
        idempotency: Option<GovernanceMutationIdempotency>,
    ) -> Result<bool, GovernanceRepositoryError> {
        let completed_at_unix_ms = audit.audit.event.occurred_at_unix_ms;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_postgres(&transaction, tenant_id, idempotency)
                .await?
            {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_postgres(
                        &transaction,
                        tenant_id,
                        idempotency,
                    )
                    .await?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().await.map_err(database_error)?;
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
                 WHERE tenant_id = $1 AND audit_event_id = $2",
                &[&tenant_id.as_uuid(), &event_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        if changed != 0 {
            append_audit_outbox_tx(&transaction, audit).await?;
        }
        if let Some(idempotency) = idempotency.as_ref() {
            complete_governance_idempotency_postgres(
                &transaction,
                tenant_id,
                idempotency,
                if changed == 0 {
                    GOVERNANCE_AUDIT_LEGAL_HOLD_DELETE_NOT_FOUND_IDEMPOTENCY_RESPONSE
                } else {
                    GOVERNANCE_AUDIT_LEGAL_HOLD_DELETE_APPLIED_IDEMPOTENCY_RESPONSE
                },
                completed_at_unix_ms,
            )
            .await?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(changed != 0)
    }

    pub async fn governance_purge_audit_events(
        &self,
        tenant_id: TenantId,
        event_ids: Vec<AuditEventId>,
        now_unix_ms: u64,
        cutoff_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
    ) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_purge_audit_events_inner(
            tenant_id,
            event_ids,
            now_unix_ms,
            cutoff_unix_ms,
            audit,
            None,
        ))
        .await
    }

    pub async fn governance_purge_audit_events_idempotent(
        &self,
        tenant_id: TenantId,
        event_ids: Vec<AuditEventId>,
        now_unix_ms: u64,
        cutoff_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_purge_audit_events_inner(
            tenant_id,
            event_ids,
            now_unix_ms,
            cutoff_unix_ms,
            audit,
            Some(idempotency),
        ))
        .await
    }

    async fn governance_purge_audit_events_inner(
        &self,
        tenant_id: TenantId,
        event_ids: Vec<AuditEventId>,
        now_unix_ms: u64,
        cutoff_unix_ms: u64,
        audit: AuditOutboxWriteCommand,
        idempotency: Option<GovernanceMutationIdempotency>,
    ) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
        if event_ids.is_empty() || event_ids.len() > usize::from(AuditRetentionBatchLimit::MAX) {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let completed_at_unix_ms = audit.audit.event.occurred_at_unix_ms;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match governance_idempotency_replay_postgres(&transaction, tenant_id, idempotency)
                .await?
            {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_governance_idempotency_pending_postgres(
                        &transaction,
                        tenant_id,
                        idempotency,
                    )
                    .await?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    let purged =
                        decode_governance_audit_retention_purge_idempotency_response(&response)?;
                    transaction.commit().await.map_err(database_error)?;
                    return Ok(purged);
                }
            }
        }
        transaction
            .execute(
                "DELETE FROM prodex_audit_legal_holds
                 WHERE tenant_id = $1
                   AND expires_at_unix_ms IS NOT NULL
                   AND expires_at_unix_ms < $2",
                &[&tenant_id.as_uuid(), &to_i64(now_unix_ms)?],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "SELECT set_config('prodex.audit_retention_purge_tenant', $1, true)",
                &[&tenant_id.to_string()],
            )
            .await
            .map_err(database_error)?;
        let ids = event_ids
            .iter()
            .map(|event_id| event_id.as_uuid())
            .collect::<Vec<_>>();
        let boundary_count = transaction
            .query_one(
                "SELECT COUNT(*)
                 FROM prodex_audit_log audit
                 WHERE audit.tenant_id = $1
                   AND (
                       audit.previous_digest IS NULL OR NOT EXISTS (
                           SELECT 1 FROM prodex_audit_log parent
                           WHERE parent.tenant_id = audit.tenant_id
                             AND parent.event_digest = audit.previous_digest
                       )
                   )",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?
            .get::<_, i64>(0);
        if boundary_count > 1 {
            return Err(GovernanceRepositoryError::AuditChainConflict);
        }
        let chain = transaction
            .query(
                "WITH RECURSIVE chain(
                    audit_event_id, event_digest, occurred_at_unix_ms, depth
                 ) AS (
                    SELECT audit.audit_event_id, audit.event_digest,
                           audit.occurred_at_unix_ms, 1::BIGINT
                    FROM prodex_audit_log audit
                    WHERE audit.tenant_id = $1
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
                    WHERE child.tenant_id = $1
                 )
                 SELECT chain.audit_event_id, chain.event_digest,
                        chain.occurred_at_unix_ms,
                        EXISTS (
                            SELECT 1 FROM prodex_audit_legal_holds hold
                            WHERE hold.tenant_id = $1
                              AND hold.audit_event_id = chain.audit_event_id
                        )
                 FROM chain
                 ORDER BY chain.depth
                 LIMIT $2",
                &[
                    &tenant_id.as_uuid(),
                    &i64::try_from(event_ids.len())
                        .map_err(|_| GovernanceRepositoryError::InvalidInput)?,
                ],
            )
            .await
            .map_err(database_error)?;
        let eligible_ids = chain
            .into_iter()
            .take_while(|row| {
                ids.contains(&row.get::<_, Uuid>(0))
                    && row.get::<_, i64>(2) >= 0
                    && u64::try_from(row.get::<_, i64>(2)).is_ok_and(|value| value < cutoff_unix_ms)
                    && !row.get::<_, bool>(3)
            })
            .map(|row| (row.get::<_, Uuid>(0), row.get::<_, String>(1)))
            .collect::<Vec<_>>();
        let last_purged_digest = eligible_ids.last().map(|(_, digest)| digest.clone());
        let eligible_ids = eligible_ids
            .into_iter()
            .map(|(event_id, _)| event_id)
            .collect::<Vec<_>>();
        transaction
            .execute(
                "DELETE FROM prodex_siem_outbox
                 WHERE tenant_id = $1 AND audit_event_id = ANY($2)",
                &[&tenant_id.as_uuid(), &eligible_ids],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "DELETE FROM prodex_siem_dead_letters
                 WHERE tenant_id = $1 AND audit_event_id = ANY($2)",
                &[&tenant_id.as_uuid(), &eligible_ids],
            )
            .await
            .map_err(database_error)?;
        let deleted = transaction
            .execute(
                "DELETE FROM prodex_audit_log audit
                 WHERE audit.tenant_id = $1
                   AND audit.audit_event_id = ANY($2)",
                &[&tenant_id.as_uuid(), &eligible_ids],
            )
            .await
            .map_err(database_error)?;
        if usize::try_from(deleted).ok() != Some(eligible_ids.len()) {
            return Err(GovernanceRepositoryError::AuditChainConflict);
        }
        let purged = eligible_ids
            .iter()
            .copied()
            .map(AuditEventId::from_uuid)
            .collect::<Vec<_>>();
        if let Some(last_purged_digest) = last_purged_digest {
            transaction
                .execute(
                    "INSERT INTO prodex_audit_retention_anchors (
                        tenant_id, last_purged_digest, updated_at_unix_ms
                     ) VALUES ($1, $2, $3)
                     ON CONFLICT (tenant_id) DO UPDATE SET
                        last_purged_digest = excluded.last_purged_digest,
                        updated_at_unix_ms = excluded.updated_at_unix_ms",
                    &[
                        &tenant_id.as_uuid(),
                        &last_purged_digest,
                        &to_i64(now_unix_ms)?,
                    ],
                )
                .await
                .map_err(database_error)?;
        }
        append_audit_outbox_tx(&transaction, audit).await?;
        if let Some(idempotency) = idempotency.as_ref() {
            let response = encode_governance_audit_retention_purge_idempotency_response(&purged);
            complete_governance_idempotency_postgres(
                &transaction,
                tenant_id,
                idempotency,
                &response,
                completed_at_unix_ms,
            )
            .await?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(purged)
    }
}

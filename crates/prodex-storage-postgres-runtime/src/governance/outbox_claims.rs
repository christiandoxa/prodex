use super::*;

impl PostgresRepository {
    pub async fn governance_claim_siem_outbox_batch(
        &self,
        tenant_id: TenantId,
        now_unix_ms: u64,
        batch_limit: u16,
        lease_ms: u64,
    ) -> Result<Vec<PostgresSiemOutboxClaim>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_claim_siem_outbox_batch_inner(
            tenant_id,
            now_unix_ms,
            batch_limit,
            lease_ms,
        ))
        .await
    }

    async fn governance_claim_siem_outbox_batch_inner(
        &self,
        tenant_id: TenantId,
        now_unix_ms: u64,
        batch_limit: u16,
        lease_ms: u64,
    ) -> Result<Vec<PostgresSiemOutboxClaim>, GovernanceRepositoryError> {
        if batch_limit == 0 || batch_limit > 256 || lease_ms == 0 || lease_ms > 300_000 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let now = to_i64(now_unix_ms)?;
        let lease_expires_at = to_i64(
            now_unix_ms
                .checked_add(lease_ms)
                .ok_or(GovernanceRepositoryError::InvalidInput)?,
        )?;
        let limit = i64::from(batch_limit);
        let claim_token = Uuid::now_v7();
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "WITH due AS (
                    SELECT tenant_id, event_id
                    FROM prodex_siem_outbox
                    WHERE tenant_id = $1
                      AND delivered_at_unix_ms IS NULL
                      AND next_attempt_at_unix_ms <= $2
                      AND (claim_expires_at_unix_ms IS NULL OR claim_expires_at_unix_ms <= $2)
                    ORDER BY next_attempt_at_unix_ms, event_id
                    FOR UPDATE SKIP LOCKED
                    LIMIT $3
                 )
                 UPDATE prodex_siem_outbox AS outbox
                 SET claim_token = $4, claim_expires_at_unix_ms = $5
                 FROM due
                 WHERE outbox.tenant_id = due.tenant_id AND outbox.event_id = due.event_id
                 RETURNING outbox.event_id, outbox.audit_event_id,
                           outbox.event_envelope, outbox.attempt_count",
                &[
                    &tenant_id.as_uuid(),
                    &now,
                    &limit,
                    &claim_token,
                    &lease_expires_at,
                ],
            )
            .await
            .map_err(database_error)?;
        let claims = rows
            .into_iter()
            .map(|row| {
                let envelope = row.get::<_, serde_json::Value>(2).to_string();
                Ok(PostgresSiemOutboxClaim {
                    tenant_id,
                    event_id: AuditEventId::from_uuid(row.get(0)),
                    audit_event_id: AuditEventId::from_uuid(row.get(1)),
                    event_envelope: envelope,
                    attempt_count: u8::try_from(row.get::<_, i32>(3))
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                    claim_token,
                })
            })
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        transaction.commit().await.map_err(database_error)?;
        Ok(claims)
    }

    pub async fn governance_finalize_siem_outbox_claim(
        &self,
        claim: &PostgresSiemOutboxClaim,
        delivered: bool,
        now_unix_ms: u64,
        retry_policy: SiemOutboxRetryPolicy,
    ) -> Result<SiemOutboxDeliveryDecision, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_finalize_siem_outbox_claim_inner(
            claim,
            delivered,
            now_unix_ms,
            retry_policy,
        ))
        .await
    }

    async fn governance_finalize_siem_outbox_claim_inner(
        &self,
        claim: &PostgresSiemOutboxClaim,
        delivered: bool,
        now_unix_ms: u64,
        retry_policy: SiemOutboxRetryPolicy,
    ) -> Result<SiemOutboxDeliveryDecision, GovernanceRepositoryError> {
        let now = to_i64(now_unix_ms)?;
        let decision =
            plan_siem_outbox_delivery(retry_policy, claim.attempt_count, delivered, now_unix_ms);
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, claim.tenant_id)
            .await
            .map_err(database_error)?;
        let owned = transaction
            .query_opt(
                "SELECT 1 FROM prodex_siem_outbox
                 WHERE tenant_id = $1 AND event_id = $2 AND claim_token = $3
                   AND delivered_at_unix_ms IS NULL
                 FOR UPDATE",
                &[
                    &claim.tenant_id.as_uuid(),
                    &claim.event_id.as_uuid(),
                    &claim.claim_token,
                ],
            )
            .await
            .map_err(database_error)?;
        if owned.is_none() {
            return Err(GovernanceRepositoryError::Conflict);
        }
        match decision {
            SiemOutboxDeliveryDecision::Delivered => {
                transaction
                    .execute(
                        "UPDATE prodex_siem_outbox
                         SET attempt_count = attempt_count + 1, delivered_at_unix_ms = $4,
                             claim_token = NULL, claim_expires_at_unix_ms = NULL
                         WHERE tenant_id = $1 AND event_id = $2 AND claim_token = $3",
                        &[
                            &claim.tenant_id.as_uuid(),
                            &claim.event_id.as_uuid(),
                            &claim.claim_token,
                            &now,
                        ],
                    )
                    .await
                    .map_err(database_error)?;
            }
            SiemOutboxDeliveryDecision::RetryAt(next_attempt_at) => {
                let next_attempt_at = to_i64(next_attempt_at)?;
                transaction
                    .execute(
                        "UPDATE prodex_siem_outbox
                         SET attempt_count = attempt_count + 1, next_attempt_at_unix_ms = $4,
                             claim_token = NULL, claim_expires_at_unix_ms = NULL
                         WHERE tenant_id = $1 AND event_id = $2 AND claim_token = $3",
                        &[
                            &claim.tenant_id.as_uuid(),
                            &claim.event_id.as_uuid(),
                            &claim.claim_token,
                            &next_attempt_at,
                        ],
                    )
                    .await
                    .map_err(database_error)?;
            }
            SiemOutboxDeliveryDecision::DeadLetter => {
                let envelope = serde_json::from_str::<serde_json::Value>(&claim.event_envelope)
                    .map_err(|_| GovernanceRepositoryError::Database)?;
                let attempts = i32::from(claim.attempt_count.saturating_add(1));
                transaction
                    .execute(
                        "INSERT INTO prodex_siem_dead_letters (
                            tenant_id, event_id, audit_event_id, event_envelope,
                            attempt_count, stable_reason_code, failed_at_unix_ms
                         ) VALUES ($1, $2, $3, $4, $5, 'delivery_failed', $6)
                         ON CONFLICT DO NOTHING",
                        &[
                            &claim.tenant_id.as_uuid(),
                            &claim.event_id.as_uuid(),
                            &claim.audit_event_id.as_uuid(),
                            &envelope,
                            &attempts,
                            &now,
                        ],
                    )
                    .await
                    .map_err(database_error)?;
                transaction
                    .execute(
                        "DELETE FROM prodex_siem_outbox
                         WHERE tenant_id = $1 AND event_id = $2 AND claim_token = $3",
                        &[
                            &claim.tenant_id.as_uuid(),
                            &claim.event_id.as_uuid(),
                            &claim.claim_token,
                        ],
                    )
                    .await
                    .map_err(database_error)?;
            }
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(decision)
    }

    pub async fn governance_append_audit_outbox(
        &self,
        command: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        self.governance_timeout(self.governance_append_audit_outbox_inner(command))
            .await
    }

    async fn governance_append_audit_outbox_inner(
        &self,
        command: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        let tenant_id = command.audit.event.tenant_id;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        append_audit_outbox_tx(&transaction, command).await?;
        transaction.commit().await.map_err(database_error)
    }
}

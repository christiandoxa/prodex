use super::*;

impl GovernanceSqliteRepository {
    pub fn run_siem_outbox_batch<E>(
        &self,
        now_unix_ms: u64,
        batch_limit: u16,
        retry_policy: SiemOutboxRetryPolicy,
        mut deliver: impl FnMut(&SiemOutboxEvent) -> Result<(), E>,
    ) -> Result<SiemOutboxWorkerReport, GovernanceRepositoryError> {
        if batch_limit == 0 || batch_limit > MAX_OUTBOX_BATCH {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let mut report = SiemOutboxWorkerReport::default();
        let started_at = std::time::Instant::now();
        for _ in 0..batch_limit {
            let claim_now = now_unix_ms.saturating_add(
                started_at
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            );
            let Some(claim) = self
                .claim_siem_outbox_batch(claim_now, 1, DEFAULT_OUTBOX_LEASE_MS)?
                .into_iter()
                .next()
            else {
                break;
            };
            report.selected = report.selected.saturating_add(1);
            let delivered = deliver(&claim.event()).is_ok();
            let finalize_now = now_unix_ms.saturating_add(
                started_at
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            );
            match self.finalize_siem_outbox_claim(&claim, delivered, finalize_now, retry_policy)? {
                SiemOutboxDeliveryDecision::Delivered => {
                    report.delivered = report.delivered.saturating_add(1);
                }
                SiemOutboxDeliveryDecision::RetryAt(_) => {
                    report.retried = report.retried.saturating_add(1);
                }
                SiemOutboxDeliveryDecision::DeadLetter => {
                    report.dead_lettered = report.dead_lettered.saturating_add(1);
                }
            }
        }
        Ok(report)
    }

    pub fn run_siem_outbox_exporter_batch<E: GovernanceAuditExporter>(
        &self,
        now_unix_ms: u64,
        batch_limit: u16,
        retry_policy: SiemOutboxRetryPolicy,
        exporter: &mut E,
    ) -> Result<SiemOutboxWorkerReport, GovernanceRepositoryError> {
        self.run_siem_outbox_batch(now_unix_ms, batch_limit, retry_policy, |event| {
            let batch = SiemExportBatch::bounded(
                vec![SiemExportEvent {
                    event_id: event.event_id,
                    event_envelope: event.event_envelope.clone(),
                }],
                exporter.capabilities(),
            )
            .map_err(|_| ())?;
            match exporter.export_batch(&batch) {
                Ok(receipt) if receipt.accepted_events == 1 => Ok(()),
                Ok(_) | Err(_) => Err(()),
            }
        })
    }

    pub fn claim_siem_outbox_batch(
        &self,
        now_unix_ms: u64,
        batch_limit: u16,
        lease_ms: u64,
    ) -> Result<Vec<SqliteSiemOutboxClaim>, GovernanceRepositoryError> {
        if batch_limit == 0
            || batch_limit > MAX_OUTBOX_BATCH
            || lease_ms == 0
            || lease_ms > MAX_OUTBOX_LEASE_MS
        {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let now = to_i64(now_unix_ms)?;
        let lease_expires_at = to_i64(
            now_unix_ms
                .checked_add(lease_ms)
                .ok_or(GovernanceRepositoryError::InvalidInput)?,
        )?;
        let claim_token = AuditEventId::new().to_string();
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        let claims = {
            let mut statement = transaction
                .prepare(
                    "SELECT tenant_id, event_id, audit_event_id, event_envelope, attempt_count
                     FROM prodex_siem_outbox
                     WHERE delivered_at_unix_ms IS NULL
                       AND next_attempt_at_unix_ms <= ?1
                       AND (claim_expires_at_unix_ms IS NULL OR claim_expires_at_unix_ms <= ?1)
                     ORDER BY next_attempt_at_unix_ms, event_id
                     LIMIT ?2",
                )
                .map_err(database_error)?;
            let rows = statement
                .query_map(params![now, i64::from(batch_limit)], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })
                .map_err(database_error)?;
            let mut claims = Vec::with_capacity(usize::from(batch_limit));
            for row in rows {
                let (tenant_id, event_id, audit_event_id, event_envelope, attempt_count) =
                    row.map_err(database_error)?;
                claims.push(SqliteSiemOutboxClaim {
                    tenant_id: TenantId::from_str(&tenant_id)
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                    event_id: AuditEventId::from_str(&event_id)
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                    audit_event_id: AuditEventId::from_str(&audit_event_id)
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                    event_envelope,
                    attempt_count: u8::try_from(attempt_count)
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                    claim_token: claim_token.clone(),
                });
            }
            claims
        };
        for claim in &claims {
            let changed = transaction
                .execute(
                    "UPDATE prodex_siem_outbox
                     SET claim_token = ?3, claim_expires_at_unix_ms = ?4
                     WHERE tenant_id = ?1 AND event_id = ?2
                       AND delivered_at_unix_ms IS NULL
                       AND next_attempt_at_unix_ms <= ?5
                       AND (claim_expires_at_unix_ms IS NULL OR claim_expires_at_unix_ms <= ?5)",
                    params![
                        claim.tenant_id.to_string(),
                        claim.event_id.to_string(),
                        &claim_token,
                        lease_expires_at,
                        now,
                    ],
                )
                .map_err(database_error)?;
            if changed != 1 {
                return Err(GovernanceRepositoryError::Conflict);
            }
        }
        transaction.commit().map_err(database_error)?;
        Ok(claims)
    }

    pub fn finalize_siem_outbox_claim(
        &self,
        claim: &SqliteSiemOutboxClaim,
        delivered: bool,
        now_unix_ms: u64,
        retry_policy: SiemOutboxRetryPolicy,
    ) -> Result<SiemOutboxDeliveryDecision, GovernanceRepositoryError> {
        let now = to_i64(now_unix_ms)?;
        let decision =
            plan_siem_outbox_delivery(retry_policy, claim.attempt_count, delivered, now_unix_ms);
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        let owned = transaction
            .query_row(
                "SELECT 1 FROM prodex_siem_outbox
                 WHERE tenant_id = ?1 AND event_id = ?2 AND claim_token = ?3
                   AND claim_expires_at_unix_ms > ?4
                   AND delivered_at_unix_ms IS NULL",
                params![
                    claim.tenant_id.to_string(),
                    claim.event_id.to_string(),
                    &claim.claim_token,
                    now,
                ],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(database_error)?;
        if owned.is_none() {
            return Err(GovernanceRepositoryError::Conflict);
        }
        match decision {
            SiemOutboxDeliveryDecision::Delivered => {
                let changed = transaction
                    .execute(
                        "UPDATE prodex_siem_outbox
                         SET attempt_count = attempt_count + 1,
                             delivered_at_unix_ms = ?4,
                             claim_token = NULL,
                             claim_expires_at_unix_ms = NULL
                         WHERE tenant_id = ?1 AND event_id = ?2 AND claim_token = ?3
                           AND claim_expires_at_unix_ms > ?5
                           AND delivered_at_unix_ms IS NULL",
                        params![
                            claim.tenant_id.to_string(),
                            claim.event_id.to_string(),
                            &claim.claim_token,
                            now,
                            now,
                        ],
                    )
                    .map_err(database_error)?;
                if changed != 1 {
                    return Err(GovernanceRepositoryError::Conflict);
                }
            }
            SiemOutboxDeliveryDecision::RetryAt(next_attempt_at) => {
                let changed = transaction
                    .execute(
                        "UPDATE prodex_siem_outbox
                         SET attempt_count = attempt_count + 1,
                             next_attempt_at_unix_ms = ?4,
                             claim_token = NULL,
                             claim_expires_at_unix_ms = NULL
                         WHERE tenant_id = ?1 AND event_id = ?2 AND claim_token = ?3
                           AND claim_expires_at_unix_ms > ?5
                           AND delivered_at_unix_ms IS NULL",
                        params![
                            claim.tenant_id.to_string(),
                            claim.event_id.to_string(),
                            &claim.claim_token,
                            to_i64(next_attempt_at)?,
                            now,
                        ],
                    )
                    .map_err(database_error)?;
                if changed != 1 {
                    return Err(GovernanceRepositoryError::Conflict);
                }
            }
            SiemOutboxDeliveryDecision::DeadLetter => {
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO prodex_siem_dead_letters (
                            tenant_id, event_id, audit_event_id, event_envelope,
                            attempt_count, stable_reason_code, failed_at_unix_ms
                         ) VALUES (?1, ?2, ?3, ?4, ?5, 'delivery_failed', ?6)",
                        params![
                            claim.tenant_id.to_string(),
                            claim.event_id.to_string(),
                            claim.audit_event_id.to_string(),
                            &claim.event_envelope,
                            i64::from(claim.attempt_count.saturating_add(1)),
                            now,
                        ],
                    )
                    .map_err(database_error)?;
                let changed = transaction
                    .execute(
                        "DELETE FROM prodex_siem_outbox
                         WHERE tenant_id = ?1 AND event_id = ?2 AND claim_token = ?3
                           AND claim_expires_at_unix_ms > ?4
                           AND delivered_at_unix_ms IS NULL",
                        params![
                            claim.tenant_id.to_string(),
                            claim.event_id.to_string(),
                            &claim.claim_token,
                            now,
                        ],
                    )
                    .map_err(database_error)?;
                if changed != 1 {
                    return Err(GovernanceRepositoryError::Conflict);
                }
            }
        }
        transaction.commit().map_err(database_error)?;
        Ok(decision)
    }
}

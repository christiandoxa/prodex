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
        let events = self.load_due_outbox(now_unix_ms, batch_limit)?;
        let mut report = SiemOutboxWorkerReport {
            selected: u16::try_from(events.len()).unwrap_or(batch_limit),
            ..SiemOutboxWorkerReport::default()
        };
        for event in events {
            let delivered = deliver(&event).is_ok();
            let decision = plan_siem_outbox_delivery(
                retry_policy,
                event.attempt_count,
                delivered,
                now_unix_ms,
            );
            let mut connection = self.connection()?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(database_error)?;
            match decision {
                SiemOutboxDeliveryDecision::Delivered => {
                    transaction
                        .execute(
                            "UPDATE prodex_siem_outbox
                             SET attempt_count = attempt_count + 1, delivered_at_unix_ms = ?3
                             WHERE tenant_id = ?1 AND event_id = ?2 AND delivered_at_unix_ms IS NULL",
                            params![event.tenant_id.to_string(), event.event_id.to_string(), to_i64(now_unix_ms)?],
                        )
                        .map_err(database_error)?;
                    report.delivered = report.delivered.saturating_add(1);
                }
                SiemOutboxDeliveryDecision::RetryAt(next_attempt_at) => {
                    transaction
                        .execute(
                            "UPDATE prodex_siem_outbox
                             SET attempt_count = attempt_count + 1, next_attempt_at_unix_ms = ?3
                             WHERE tenant_id = ?1 AND event_id = ?2 AND delivered_at_unix_ms IS NULL",
                            params![event.tenant_id.to_string(), event.event_id.to_string(), to_i64(next_attempt_at)?],
                        )
                        .map_err(database_error)?;
                    report.retried = report.retried.saturating_add(1);
                }
                SiemOutboxDeliveryDecision::DeadLetter => {
                    transaction
                        .execute(
                            "INSERT OR IGNORE INTO prodex_siem_dead_letters (
                                tenant_id, event_id, audit_event_id, event_envelope,
                                attempt_count, stable_reason_code, failed_at_unix_ms
                             ) VALUES (?1, ?2, ?3, ?4, ?5, 'delivery_failed', ?6)",
                            params![
                                event.tenant_id.to_string(),
                                event.event_id.to_string(),
                                event.audit_event_id.to_string(),
                                event.event_envelope,
                                i64::from(event.attempt_count.saturating_add(1)),
                                to_i64(now_unix_ms)?,
                            ],
                        )
                        .map_err(database_error)?;
                    transaction
                        .execute(
                            "DELETE FROM prodex_siem_outbox WHERE tenant_id = ?1 AND event_id = ?2",
                            params![event.tenant_id.to_string(), event.event_id.to_string()],
                        )
                        .map_err(database_error)?;
                    report.dead_lettered = report.dead_lettered.saturating_add(1);
                }
            }
            transaction.commit().map_err(database_error)?;
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

    fn load_due_outbox(
        &self,
        now_unix_ms: u64,
        batch_limit: u16,
    ) -> Result<Vec<SiemOutboxEvent>, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT tenant_id, event_id, audit_event_id, event_envelope, attempt_count
                 FROM prodex_siem_outbox
                 WHERE delivered_at_unix_ms IS NULL AND next_attempt_at_unix_ms <= ?1
                 ORDER BY next_attempt_at_unix_ms, event_id
                 LIMIT ?2",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map(
                params![to_i64(now_unix_ms)?, i64::from(batch_limit)],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .map_err(database_error)?;
        let mut events = Vec::with_capacity(usize::from(batch_limit));
        for row in rows {
            let (tenant_id, event_id, audit_event_id, event_envelope, attempt_count) =
                row.map_err(database_error)?;
            events.push(SiemOutboxEvent {
                tenant_id: TenantId::from_str(&tenant_id)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
                event_id: AuditEventId::from_str(&event_id)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
                audit_event_id: AuditEventId::from_str(&audit_event_id)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
                event_envelope,
                attempt_count: u8::try_from(attempt_count)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
            });
        }
        Ok(events)
    }
}

use super::*;

const MAX_INVALIDATION_BATCH: u16 = 256;
const MAX_INVALIDATION_COMPACTION_BATCH: i64 = 256;
const REPLICA_LEASE_MS: i64 = 5 * 60 * 1_000;
const REGISTER_REPLICA_SQL: &str = r#"
INSERT INTO prodex_governance_invalidation_replicas (
    tenant_id, replica_id, registered_at_unix_ms, last_seen_at_unix_ms
)
SELECT $1, $2, observed.now_ms, observed.now_ms
FROM (
    SELECT (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT AS now_ms
) observed
ON CONFLICT (tenant_id, replica_id) DO UPDATE SET
    last_seen_at_unix_ms = GREATEST(
        prodex_governance_invalidation_replicas.last_seen_at_unix_ms,
        EXCLUDED.last_seen_at_unix_ms
    )
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostgresGovernanceInvalidation {
    pub event_id: i64,
    pub tenant_id: TenantId,
    pub kind: GovernanceArtifactKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GovernanceInvalidationOutboxCleanup {
    pub replica_count: usize,
    pub eligible_event_count: usize,
    pub removed_event_count: usize,
    pub retained_event_count: usize,
}

impl PostgresRepository {
    pub async fn governance_poll_invalidation_outbox(
        &self,
        tenant_id: TenantId,
        replica_id: &str,
        limit: u16,
    ) -> Result<Vec<PostgresGovernanceInvalidation>, GovernanceRepositoryError> {
        if !valid_replica_id(replica_id) || limit == 0 || limit > MAX_INVALIDATION_BATCH {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        self.governance_timeout(
            self.governance_poll_invalidation_outbox_inner(tenant_id, replica_id, limit),
        )
        .await
    }

    async fn governance_poll_invalidation_outbox_inner(
        &self,
        tenant_id: TenantId,
        replica_id: &str,
        limit: u16,
    ) -> Result<Vec<PostgresGovernanceInvalidation>, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        transaction
            .execute(REGISTER_REPLICA_SQL, &[&tenant_id.as_uuid(), &replica_id])
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                r#"
                SELECT event.event_id, event.artifact_kind
                FROM prodex_governance_invalidation_outbox event
                WHERE event.tenant_id = $1
                  AND NOT EXISTS (
                      SELECT 1 FROM prodex_governance_invalidation_acks ack
                      WHERE ack.tenant_id = event.tenant_id
                        AND ack.event_id = event.event_id
                        AND ack.replica_id = $2
                  )
                ORDER BY event.event_id
                LIMIT $3
                "#,
                &[&tenant_id.as_uuid(), &replica_id, &i64::from(limit)],
            )
            .await
            .map_err(database_error)?;
        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            events.push(PostgresGovernanceInvalidation {
                event_id: row.try_get(0).map_err(database_error)?,
                tenant_id,
                kind: invalidation_kind_from_label(
                    row.try_get::<_, &str>(1).map_err(database_error)?,
                )?,
            });
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(events)
    }

    pub async fn governance_ack_invalidation_outbox_event(
        &self,
        replica_id: &str,
        event: PostgresGovernanceInvalidation,
    ) -> Result<(), GovernanceRepositoryError> {
        if !valid_replica_id(replica_id) || event.event_id <= 0 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        self.governance_timeout(
            self.governance_ack_invalidation_outbox_event_inner(replica_id, event),
        )
        .await
    }

    async fn governance_ack_invalidation_outbox_event_inner(
        &self,
        replica_id: &str,
        event: PostgresGovernanceInvalidation,
    ) -> Result<(), GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, event.tenant_id)
            .await
            .map_err(database_error)?;
        let event_exists = transaction
            .query_opt(
                r#"
                SELECT 1
                FROM prodex_governance_invalidation_outbox
                WHERE tenant_id = $1
                  AND event_id = $2
                  AND artifact_kind = $3
                FOR KEY SHARE
                "#,
                &[
                    &event.tenant_id.as_uuid(),
                    &event.event_id,
                    &artifact_kind_label(event.kind),
                ],
            )
            .await
            .map_err(database_error)?;
        if event_exists.is_none() {
            return Err(GovernanceRepositoryError::NotFound);
        }
        transaction
            .execute(
                REGISTER_REPLICA_SQL,
                &[&event.tenant_id.as_uuid(), &replica_id],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                r#"
                INSERT INTO prodex_governance_invalidation_acks (
                    tenant_id, replica_id, event_id, delivered_at_unix_ms
                )
                SELECT event.tenant_id, $2, event.event_id,
                    (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT
                FROM prodex_governance_invalidation_outbox event
                WHERE event.tenant_id = $1
                  AND event.event_id = $3
                  AND event.artifact_kind = $4
                ON CONFLICT (tenant_id, replica_id, event_id) DO NOTHING
                "#,
                &[
                    &event.tenant_id.as_uuid(),
                    &replica_id,
                    &event.event_id,
                    &artifact_kind_label(event.kind),
                ],
            )
            .await
            .map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(())
    }

    pub async fn governance_compact_invalidation_outbox(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceInvalidationOutboxCleanup, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_compact_invalidation_outbox_inner(tenant_id))
            .await
    }

    async fn governance_compact_invalidation_outbox_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceInvalidationOutboxCleanup, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                r#"
                DELETE FROM prodex_governance_invalidation_replicas
                WHERE tenant_id = $1
                  AND last_seen_at_unix_ms <
                      (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT - $2
                "#,
                &[&tenant_id.as_uuid(), &REPLICA_LEASE_MS],
            )
            .await
            .map_err(database_error)?;
        let replica_count = count(
            transaction
                .query_one(
                    "SELECT COUNT(*) FROM prodex_governance_invalidation_replicas WHERE tenant_id = $1",
                    &[&tenant_id.as_uuid()],
                )
                .await
                .map_err(database_error)?
                .try_get(0)
                .map_err(database_error)?,
        )?;
        let retained_before = count(
            transaction
                .query_one(
                    "SELECT COUNT(*) FROM prodex_governance_invalidation_outbox WHERE tenant_id = $1",
                    &[&tenant_id.as_uuid()],
                )
                .await
                .map_err(database_error)?
                .try_get(0)
                .map_err(database_error)?,
        )?;
        let eligible = r#"NOT EXISTS (
            SELECT 1 FROM prodex_governance_invalidation_replicas replica
            WHERE replica.tenant_id = event.tenant_id
              AND NOT EXISTS (
                  SELECT 1 FROM prodex_governance_invalidation_acks ack
                  WHERE ack.tenant_id = event.tenant_id
                    AND ack.event_id = event.event_id
                    AND ack.replica_id = replica.replica_id
              )
        )"#;
        let eligible_event_count = count(
            transaction
                .query_one(
                    &format!(
                        "SELECT COUNT(*) FROM prodex_governance_invalidation_outbox event \
                         WHERE event.tenant_id = $1 AND {eligible}"
                    ),
                    &[&tenant_id.as_uuid()],
                )
                .await
                .map_err(database_error)?
                .try_get(0)
                .map_err(database_error)?,
        )?;
        let removed_event_count = usize::try_from(
            transaction
                .execute(
                    &format!(
                        "WITH deletable AS ( \
                             SELECT event.tenant_id, event.event_id \
                             FROM prodex_governance_invalidation_outbox event \
                             WHERE event.tenant_id = $1 AND {eligible} \
                               AND event.event_id <> ( \
                                   SELECT MAX(latest.event_id) \
                                   FROM prodex_governance_invalidation_outbox latest \
                                   WHERE latest.tenant_id = event.tenant_id \
                                     AND latest.artifact_kind = event.artifact_kind \
                               ) \
                             ORDER BY event.event_id \
                             LIMIT $2 \
                         ) \
                         DELETE FROM prodex_governance_invalidation_outbox event \
                         USING deletable \
                         WHERE event.tenant_id = deletable.tenant_id \
                           AND event.event_id = deletable.event_id"
                    ),
                    &[&tenant_id.as_uuid(), &MAX_INVALIDATION_COMPACTION_BATCH],
                )
                .await
                .map_err(database_error)?,
        )
        .map_err(|_| GovernanceRepositoryError::Database)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceInvalidationOutboxCleanup {
            replica_count,
            eligible_event_count,
            removed_event_count,
            retained_event_count: retained_before.saturating_sub(removed_event_count),
        })
    }
}

fn valid_replica_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn invalidation_kind_from_label(
    value: &str,
) -> Result<GovernanceArtifactKind, GovernanceRepositoryError> {
    match value {
        "policy" => Ok(GovernanceArtifactKind::Policy),
        "classification_rules" => Ok(GovernanceArtifactKind::ClassificationRules),
        "provider_registry" => Ok(GovernanceArtifactKind::ProviderRegistry),
        "routing_scores" => Ok(GovernanceArtifactKind::RoutingScores),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

fn count(value: i64) -> Result<usize, GovernanceRepositoryError> {
    usize::try_from(value).map_err(|_| GovernanceRepositoryError::Database)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replica_ids_and_kinds_are_bounded() {
        assert!(valid_replica_id("gateway-a_1.example"));
        assert!(!valid_replica_id(""));
        assert!(!valid_replica_id("gateway/a"));
        assert!(!valid_replica_id(&"a".repeat(129)));
        assert_eq!(
            invalidation_kind_from_label("routing_scores"),
            Ok(GovernanceArtifactKind::RoutingScores)
        );
        assert_eq!(
            invalidation_kind_from_label("unknown"),
            Err(GovernanceRepositoryError::Database)
        );
    }
}

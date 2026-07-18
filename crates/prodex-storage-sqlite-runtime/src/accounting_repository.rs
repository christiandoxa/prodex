use std::error::Error;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use prodex_domain::{
    BudgetSnapshot, CallId, RequestId, ReservationId, ReservationRecord, TenantId, UsageAmount,
    VirtualKeyId,
};
use prodex_storage::{ExpiredReservationRecoveryCommand, TenantStorageKey};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

const MAX_EXPIRED_RESERVATION_BATCH: usize = 256;

const LOAD_EXPIRED_RESERVATIONS_SQL: &str = r#"
SELECT
    reservation.tenant_id,
    reservation.reservation_id,
    reservation.call_id,
    reservation.virtual_key_id,
    reservation.storage_scope,
    reservation.reserved_tokens,
    reservation.reserved_cost_micros,
    reservation.created_at_unix_ms,
    reservation.expires_at_unix_ms,
    counter.reserved_tokens,
    counter.reserved_cost_micros,
    counter.committed_tokens,
    counter.committed_cost_micros
FROM prodex_reservations reservation
JOIN prodex_budget_counters counter
  ON counter.tenant_id = reservation.tenant_id
 AND counter.storage_scope = reservation.storage_scope
WHERE reservation.committed_at_unix_ms IS NULL
  AND reservation.released_at_unix_ms IS NULL
  AND reservation.expires_at_unix_ms <= ?1
ORDER BY reservation.expires_at_unix_ms, reservation.tenant_id, reservation.reservation_id
LIMIT ?2
"#;

#[derive(Clone, PartialEq, Eq)]
pub struct SqliteExpiredReservationCandidate {
    pub storage_key: TenantStorageKey,
    pub snapshot: BudgetSnapshot,
    pub record: ReservationRecord,
}

impl fmt::Debug for SqliteExpiredReservationCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteExpiredReservationCandidate")
            .field("storage_key", &"<redacted>")
            .field("snapshot", &"<redacted>")
            .field("record", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqliteIdempotentWriteOutcome {
    Applied,
    Replayed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqliteAccountingRepositoryError {
    Configuration,
    Planning,
    NumericOverflow,
    Database,
    InvalidDatabaseState,
    StateConflict,
}

impl fmt::Display for SqliteAccountingRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "SQLite accounting configuration is invalid",
            Self::Planning => "SQLite accounting operation is invalid",
            Self::NumericOverflow => "SQLite accounting value is out of range",
            Self::Database => "SQLite accounting operation failed",
            Self::InvalidDatabaseState => "SQLite accounting state is invalid",
            Self::StateConflict => "SQLite accounting state conflicts with the request",
        })
    }
}

impl Error for SqliteAccountingRepositoryError {}

impl From<rusqlite::Error> for SqliteAccountingRepositoryError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Database
    }
}

pub struct SqliteAccountingRepository {
    connection: Connection,
}

impl SqliteAccountingRepository {
    pub fn open(path: &Path) -> Result<Self, SqliteAccountingRepositoryError> {
        let connection = Connection::open(path)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| SqliteAccountingRepositoryError::Configuration)?;
        Ok(Self { connection })
    }

    pub fn from_connection(connection: Connection) -> Self {
        Self { connection }
    }

    pub fn load_expired_reservations(
        &self,
        now_unix_ms: u64,
        limit: usize,
    ) -> Result<Vec<SqliteExpiredReservationCandidate>, SqliteAccountingRepositoryError> {
        if limit == 0 || limit > MAX_EXPIRED_RESERVATION_BATCH {
            return Err(SqliteAccountingRepositoryError::Configuration);
        }
        let now_unix_ms = to_i64(now_unix_ms)?;
        let limit =
            i64::try_from(limit).map_err(|_| SqliteAccountingRepositoryError::NumericOverflow)?;
        let mut statement = self.connection.prepare(LOAD_EXPIRED_RESERVATIONS_SQL)?;
        let candidates = statement
            .query_map(params![now_unix_ms, limit], candidate_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(candidates)
    }

    pub fn release_expired(
        &mut self,
        command: ExpiredReservationRecoveryCommand,
    ) -> Result<SqliteIdempotentWriteOutcome, SqliteAccountingRepositoryError> {
        prodex_storage_sqlite::plan_sqlite_expired_reservation_recovery(command.clone())
            .map_err(|_| SqliteAccountingRepositoryError::Planning)?;
        let now_unix_ms = to_i64(command.now_unix_ms)?;
        let reserved_tokens = to_i64(command.record.reserved.tokens)?;
        let reserved_cost_micros = to_i64(command.record.reserved.cost_micros)?;
        let tenant_id = command.record.tenant_id.to_string();
        let reservation_id = command.record.reservation_id.to_string();
        let call_id = command.record.call_id.to_string();
        let storage_scope = command.storage_key.storage_scope();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let counter_updated = transaction.execute(
            r#"
            UPDATE prodex_budget_counters
            SET reserved_tokens = reserved_tokens - ?5,
                reserved_cost_micros = reserved_cost_micros - ?6,
                updated_at_unix_ms = ?4
            WHERE tenant_id = ?1
              AND storage_scope = ?7
              AND reserved_tokens >= ?5
              AND reserved_cost_micros >= ?6
              AND EXISTS (
                  SELECT 1
                  FROM prodex_reservations
                  WHERE tenant_id = ?1
                    AND reservation_id = ?2
                    AND call_id = ?3
                    AND committed_at_unix_ms IS NULL
                    AND released_at_unix_ms IS NULL
                    AND expires_at_unix_ms <= ?4
              )
            "#,
            params![
                tenant_id,
                reservation_id,
                call_id,
                now_unix_ms,
                reserved_tokens,
                reserved_cost_micros,
                storage_scope,
            ],
        )?;
        if counter_updated == 0 {
            return replayed_or_conflict(transaction, &command);
        }
        if counter_updated != 1 {
            return Err(SqliteAccountingRepositoryError::InvalidDatabaseState);
        }
        let reservation_updated = transaction.execute(
            r#"
            UPDATE prodex_reservations
            SET released_at_unix_ms = ?4
            WHERE tenant_id = ?1
              AND reservation_id = ?2
              AND call_id = ?3
              AND committed_at_unix_ms IS NULL
              AND released_at_unix_ms IS NULL
              AND expires_at_unix_ms <= ?4
            "#,
            params![tenant_id, reservation_id, call_id, now_unix_ms],
        )?;
        if reservation_updated != 1 {
            return Err(SqliteAccountingRepositoryError::StateConflict);
        }
        transaction.execute(
            r#"
            INSERT OR IGNORE INTO prodex_usage_ledger (
                tenant_id, ledger_event_id, reservation_id, call_id, event_kind,
                tokens, cost_micros, occurred_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?4, 'released', ?5, ?6, ?7)
            "#,
            params![
                tenant_id,
                RequestId::new().to_string(),
                reservation_id,
                call_id,
                reserved_tokens,
                reserved_cost_micros,
                now_unix_ms,
            ],
        )?;
        transaction.commit()?;
        Ok(SqliteIdempotentWriteOutcome::Applied)
    }
}

fn replayed_or_conflict(
    transaction: rusqlite::Transaction<'_>,
    command: &ExpiredReservationRecoveryCommand,
) -> Result<SqliteIdempotentWriteOutcome, SqliteAccountingRepositoryError> {
    let stored = transaction
        .query_row(
            r#"
            SELECT storage_scope, reserved_tokens, reserved_cost_micros,
                   committed_at_unix_ms, released_at_unix_ms
            FROM prodex_reservations
            WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3
            "#,
            params![
                command.record.tenant_id.to_string(),
                command.record.reservation_id.to_string(),
                command.record.call_id.to_string(),
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((storage_scope, tokens, cost_micros, committed_at, released_at)) = stored else {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    };
    if storage_scope == command.storage_key.storage_scope()
        && from_i64(tokens)? == command.record.reserved.tokens
        && from_i64(cost_micros)? == command.record.reserved.cost_micros
        && committed_at.is_none()
        && released_at.is_some()
    {
        transaction.commit()?;
        Ok(SqliteIdempotentWriteOutcome::Replayed)
    } else {
        Err(SqliteAccountingRepositoryError::StateConflict)
    }
}

fn candidate_from_row(
    row: &rusqlite::Row<'_>,
) -> Result<SqliteExpiredReservationCandidate, rusqlite::Error> {
    candidate_from_row_inner(row).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn candidate_from_row_inner(
    row: &rusqlite::Row<'_>,
) -> Result<SqliteExpiredReservationCandidate, SqliteAccountingRepositoryError> {
    let tenant_id = TenantId::from_str(&row.get::<_, String>(0)?)
        .map_err(|_| SqliteAccountingRepositoryError::InvalidDatabaseState)?;
    let reservation_id = ReservationId::from_str(&row.get::<_, String>(1)?)
        .map_err(|_| SqliteAccountingRepositoryError::InvalidDatabaseState)?;
    let call_id = CallId::from_str(&row.get::<_, String>(2)?)
        .map_err(|_| SqliteAccountingRepositoryError::InvalidDatabaseState)?;
    let virtual_key_id = row
        .get::<_, Option<String>>(3)?
        .map(|value| VirtualKeyId::from_str(&value))
        .transpose()
        .map_err(|_| SqliteAccountingRepositoryError::InvalidDatabaseState)?;
    let storage_scope = row.get::<_, String>(4)?;
    let storage_key =
        TenantStorageKey::from_storage_scope(tenant_id, virtual_key_id, &storage_scope)
            .ok_or(SqliteAccountingRepositoryError::InvalidDatabaseState)?;
    Ok(SqliteExpiredReservationCandidate {
        storage_key,
        snapshot: BudgetSnapshot {
            reserved: UsageAmount::new(from_i64(row.get(9)?)?, from_i64(row.get(10)?)?),
            committed: UsageAmount::new(from_i64(row.get(11)?)?, from_i64(row.get(12)?)?),
        },
        record: ReservationRecord {
            tenant_id,
            reservation_id,
            call_id,
            reserved: UsageAmount::new(from_i64(row.get(5)?)?, from_i64(row.get(6)?)?),
            created_at_unix_ms: from_i64(row.get(7)?)?,
            expires_at_unix_ms: from_i64(row.get(8)?)?,
        },
    })
}

fn to_i64(value: u64) -> Result<i64, SqliteAccountingRepositoryError> {
    i64::try_from(value).map_err(|_| SqliteAccountingRepositoryError::NumericOverflow)
}

fn from_i64(value: i64) -> Result<u64, SqliteAccountingRepositoryError> {
    u64::try_from(value).map_err(|_| SqliteAccountingRepositoryError::InvalidDatabaseState)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_with_expired_reservation() -> (
        SqliteAccountingRepository,
        ExpiredReservationRecoveryCommand,
    ) {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE prodex_budget_counters (
                    tenant_id TEXT NOT NULL,
                    storage_scope TEXT NOT NULL,
                    virtual_key_id TEXT,
                    reserved_tokens INTEGER NOT NULL,
                    reserved_cost_micros INTEGER NOT NULL,
                    committed_tokens INTEGER NOT NULL,
                    committed_cost_micros INTEGER NOT NULL,
                    updated_at_unix_ms INTEGER NOT NULL,
                    PRIMARY KEY (tenant_id, storage_scope)
                );
                CREATE TABLE prodex_reservations (
                    tenant_id TEXT NOT NULL,
                    reservation_id TEXT NOT NULL,
                    call_id TEXT NOT NULL,
                    virtual_key_id TEXT,
                    storage_scope TEXT NOT NULL,
                    reserved_tokens INTEGER NOT NULL,
                    reserved_cost_micros INTEGER NOT NULL,
                    created_at_unix_ms INTEGER NOT NULL,
                    expires_at_unix_ms INTEGER NOT NULL,
                    committed_at_unix_ms INTEGER,
                    released_at_unix_ms INTEGER,
                    PRIMARY KEY (tenant_id, reservation_id)
                );
                CREATE TABLE prodex_usage_ledger (
                    tenant_id TEXT NOT NULL,
                    ledger_event_id TEXT NOT NULL,
                    reservation_id TEXT NOT NULL,
                    call_id TEXT NOT NULL,
                    event_kind TEXT NOT NULL,
                    tokens INTEGER NOT NULL,
                    cost_micros INTEGER NOT NULL,
                    occurred_at_unix_ms INTEGER NOT NULL,
                    UNIQUE (tenant_id, reservation_id, event_kind)
                );
                "#,
            )
            .unwrap();
        let tenant_id = TenantId::new();
        let record = ReservationRecord {
            tenant_id,
            reservation_id: ReservationId::new(),
            call_id: CallId::new(),
            reserved: UsageAmount::new(10, 20),
            created_at_unix_ms: 100,
            expires_at_unix_ms: 200,
        };
        connection
            .execute(
                "INSERT INTO prodex_budget_counters VALUES (?1, 'tenant-default', NULL, 10, 20, 1, 2, 100)",
                params![tenant_id.to_string()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO prodex_reservations VALUES (?1, ?2, ?3, NULL, 'tenant-default', 10, 20, 100, 200, NULL, NULL)",
                params![
                    tenant_id.to_string(),
                    record.reservation_id.to_string(),
                    record.call_id.to_string(),
                ],
            )
            .unwrap();
        let command = ExpiredReservationRecoveryCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            snapshot: BudgetSnapshot {
                reserved: record.reserved,
                committed: UsageAmount::new(1, 2),
            },
            record,
            now_unix_ms: 300,
        };
        (
            SqliteAccountingRepository::from_connection(connection),
            command,
        )
    }

    #[test]
    fn expired_reservation_recovery_is_bounded_atomic_and_idempotent() {
        let (mut repository, command) = repository_with_expired_reservation();
        let candidates = repository.load_expired_reservations(300, 64).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].storage_key, command.storage_key);
        assert_eq!(candidates[0].record, command.record);

        assert_eq!(
            repository.release_expired(command.clone()).unwrap(),
            SqliteIdempotentWriteOutcome::Applied
        );
        assert_eq!(
            repository.release_expired(command).unwrap(),
            SqliteIdempotentWriteOutcome::Replayed
        );
        assert!(
            repository
                .load_expired_reservations(300, 64)
                .unwrap()
                .is_empty()
        );

        let counters: (i64, i64) = repository
            .connection
            .query_row(
                "SELECT reserved_tokens, reserved_cost_micros FROM prodex_budget_counters",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(counters, (0, 0));
        let ledger_count: i64 = repository
            .connection
            .query_row("SELECT COUNT(*) FROM prodex_usage_ledger", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(ledger_count, 1);
    }
}

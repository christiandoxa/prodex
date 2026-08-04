use std::error::Error;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use prodex_domain::{
    BudgetSnapshot, CallId, RequestId, ReservationId, ReservationRecord, TenantId, UsageAmount,
    VirtualKeyId,
};
use prodex_storage::{
    AtomicReservationCommand, ExpiredReservationRecoveryCommand, TenantStorageKey,
    UsageReconciliationCommand,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

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

#[derive(Clone, PartialEq, Eq)]
pub enum SqliteReserveOutcome {
    Reserved(ReservationRecord),
    Replayed(ReservationRecord),
    Rejected,
}

impl fmt::Debug for SqliteReserveOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved(_) => f.debug_tuple("Reserved").field(&"<redacted>").finish(),
            Self::Replayed(_) => f.debug_tuple("Replayed").field(&"<redacted>").finish(),
            Self::Rejected => f.write_str("Rejected"),
        }
    }
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

struct SqliteReconciliationValues {
    occurred_at: i64,
    reserved_tokens: i64,
    reserved_cost_micros: i64,
    actual_tokens: i64,
    actual_cost_micros: i64,
    created_at: i64,
    expires_at: i64,
    released_tokens: i64,
    released_cost_micros: i64,
    tenant_id: String,
    reservation_id: String,
    call_id: String,
    storage_scope: String,
    virtual_key_id: Option<String>,
}

impl SqliteReconciliationValues {
    fn new(
        command: UsageReconciliationCommand,
        occurred_at_unix_ms: u64,
    ) -> Result<Self, SqliteAccountingRepositoryError> {
        let released = command.record.reserved.saturating_sub(command.actual);
        Ok(Self {
            occurred_at: to_i64(occurred_at_unix_ms)?,
            reserved_tokens: to_i64(command.record.reserved.tokens)?,
            reserved_cost_micros: to_i64(command.record.reserved.cost_micros)?,
            actual_tokens: to_i64(command.actual.tokens)?,
            actual_cost_micros: to_i64(command.actual.cost_micros)?,
            created_at: to_i64(command.record.created_at_unix_ms)?,
            expires_at: to_i64(command.record.expires_at_unix_ms)?,
            released_tokens: to_i64(released.tokens)?,
            released_cost_micros: to_i64(released.cost_micros)?,
            tenant_id: command.record.tenant_id.to_string(),
            reservation_id: command.record.reservation_id.to_string(),
            call_id: command.record.call_id.to_string(),
            storage_scope: command.storage_key.storage_scope(),
            virtual_key_id: command.storage_key.virtual_key_id.map(|id| id.to_string()),
        })
    }
}

struct SqliteReconciliationState {
    committed_at: Option<i64>,
    reservation_was_released: bool,
    committed_usage: Option<(i64, i64)>,
    released_usage: Option<(i64, i64)>,
}

impl SqliteAccountingRepository {
    pub fn open(path: &Path) -> Result<Self, SqliteAccountingRepositoryError> {
        let connection = Connection::open(path)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|_| SqliteAccountingRepositoryError::Configuration)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| SqliteAccountingRepositoryError::Configuration)?;
        Ok(Self { connection })
    }

    pub fn from_connection(connection: Connection) -> Self {
        connection
            .pragma_update(None, "foreign_keys", true)
            .expect("SQLite accounting connection accepts foreign_keys pragma");
        Self { connection }
    }

    pub fn reserve(
        &mut self,
        command: AtomicReservationCommand,
    ) -> Result<SqliteReserveOutcome, SqliteAccountingRepositoryError> {
        prodex_storage_sqlite::plan_sqlite_atomic_reservation(command.clone())
            .map_err(|_| SqliteAccountingRepositoryError::Planning)?;
        let record = prodex_storage::plan_atomic_reservation(command.clone())
            .map_err(|_| SqliteAccountingRepositoryError::Planning)?
            .reservation_record;
        let expires_at = command
            .created_at_unix_ms
            .checked_add(command.ttl_ms)
            .ok_or(SqliteAccountingRepositoryError::NumericOverflow)?;
        let reserved_tokens = to_i64(command.request.estimate.tokens)?;
        let reserved_cost_micros = to_i64(command.request.estimate.cost_micros)?;
        let created_at = to_i64(command.created_at_unix_ms)?;
        let expires_at = to_i64(expires_at)?;
        let max_tokens = i64::try_from(command.limit.max.tokens).unwrap_or(i64::MAX);
        let max_cost_micros = i64::try_from(command.limit.max.cost_micros).unwrap_or(i64::MAX);
        let tenant_id = command.request.tenant_id.to_string();
        let reservation_id = command.request.reservation_id.to_string();
        let call_id = command.request.call_id.to_string();
        let idempotency_key = command.idempotency_key.as_str().to_string();
        let storage_scope = command.storage_key.storage_scope();
        let virtual_key_id = command.storage_key.virtual_key_id.map(|id| id.to_string());
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        let existing = {
            let mut statement = transaction.prepare(
                "SELECT reservation_id, call_id, virtual_key_id, storage_scope,
                        idempotency_key, reserved_tokens, reserved_cost_micros,
                        created_at_unix_ms, expires_at_unix_ms
                 FROM prodex_reservations
                 WHERE tenant_id = ?1
                   AND (reservation_id = ?2 OR call_id = ?3 OR idempotency_key = ?4)",
            )?;
            let rows = statement
                .query_map(
                    params![&tenant_id, &reservation_id, &call_id, &idempotency_key],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, i64>(6)?,
                            row.get::<_, i64>(7)?,
                            row.get::<_, i64>(8)?,
                        ))
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            if rows.len() > 1 {
                return Err(SqliteAccountingRepositoryError::StateConflict);
            }
            rows.into_iter().next()
        };
        if let Some((
            stored_reservation_id,
            stored_call_id,
            stored_virtual_key_id,
            stored_scope,
            stored_idempotency_key,
            stored_tokens,
            stored_cost_micros,
            stored_created_at,
            stored_expires_at,
        )) = existing
        {
            let counter: Option<(Option<String>, String)> = transaction
                .query_row(
                    "SELECT virtual_key_id, storage_scope
                     FROM prodex_budget_counters
                     WHERE tenant_id = ?1 AND storage_scope = ?2",
                    params![&tenant_id, &storage_scope],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let reserved_ledger: Option<(String, String, i64, i64)> = transaction
                .query_row(
                    "SELECT reservation_id, call_id, tokens, cost_micros
                     FROM prodex_usage_ledger
                     WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3
                       AND event_kind = 'reserved'",
                    params![&tenant_id, &reservation_id, &call_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?;
            let exact = stored_reservation_id == reservation_id
                && stored_call_id == call_id
                && stored_virtual_key_id == virtual_key_id
                && stored_scope == storage_scope
                && stored_idempotency_key == idempotency_key
                && stored_tokens == reserved_tokens
                && stored_cost_micros == reserved_cost_micros
                && stored_created_at == created_at
                && stored_expires_at == expires_at
                && counter == Some((virtual_key_id.clone(), storage_scope.clone()))
                && reserved_ledger
                    == Some((
                        reservation_id,
                        call_id,
                        reserved_tokens,
                        reserved_cost_micros,
                    ));
            if !exact {
                return Err(SqliteAccountingRepositoryError::StateConflict);
            }
            transaction.commit()?;
            return Ok(SqliteReserveOutcome::Replayed(record));
        }

        let counter: Option<(Option<String>, String)> = transaction
            .query_row(
                "SELECT virtual_key_id, storage_scope
                 FROM prodex_budget_counters
                 WHERE tenant_id = ?1 AND storage_scope = ?2",
                params![&tenant_id, &storage_scope],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if counter.is_some_and(|value| value != (virtual_key_id.clone(), storage_scope.clone())) {
            return Err(SqliteAccountingRepositoryError::StateConflict);
        }

        let changed = transaction.execute(
            r#"
            INSERT INTO prodex_budget_counters (
                tenant_id, storage_scope, virtual_key_id, reserved_tokens,
                reserved_cost_micros, committed_tokens, committed_cost_micros,
                updated_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6)
            ON CONFLICT(tenant_id, storage_scope) DO UPDATE SET
                reserved_tokens = reserved_tokens + excluded.reserved_tokens,
                reserved_cost_micros = reserved_cost_micros + excluded.reserved_cost_micros,
                updated_at_unix_ms = excluded.updated_at_unix_ms
            WHERE prodex_budget_counters.reserved_tokens
                    + prodex_budget_counters.committed_tokens
                    + excluded.reserved_tokens <= ?7
              AND prodex_budget_counters.reserved_cost_micros
                    + prodex_budget_counters.committed_cost_micros
                    + excluded.reserved_cost_micros <= ?8
            "#,
            params![
                &tenant_id,
                &storage_scope,
                &virtual_key_id,
                reserved_tokens,
                reserved_cost_micros,
                created_at,
                max_tokens,
                max_cost_micros,
            ],
        )?;
        if changed == 0 {
            return Ok(SqliteReserveOutcome::Rejected);
        }
        transaction.execute(
            r#"
            INSERT INTO prodex_reservations (
                tenant_id, reservation_id, call_id, virtual_key_id, storage_scope,
                idempotency_key, reserved_tokens, reserved_cost_micros,
                created_at_unix_ms, expires_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                &tenant_id,
                &reservation_id,
                &call_id,
                &virtual_key_id,
                &storage_scope,
                &idempotency_key,
                reserved_tokens,
                reserved_cost_micros,
                created_at,
                expires_at,
            ],
        )?;
        transaction.execute(
            r#"
            INSERT INTO prodex_usage_ledger (
                tenant_id, ledger_event_id, reservation_id, call_id, event_kind,
                tokens, cost_micros, occurred_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?4, 'reserved', ?5, ?6, ?7)
            "#,
            params![
                &tenant_id,
                RequestId::new().to_string(),
                &reservation_id,
                &call_id,
                reserved_tokens,
                reserved_cost_micros,
                created_at,
            ],
        )?;
        transaction.commit()?;
        Ok(SqliteReserveOutcome::Reserved(record))
    }

    pub fn reconcile_usage(
        &mut self,
        command: UsageReconciliationCommand,
        occurred_at_unix_ms: u64,
    ) -> Result<SqliteIdempotentWriteOutcome, SqliteAccountingRepositoryError> {
        prodex_storage_sqlite::plan_sqlite_usage_reconciliation(command.clone())
            .map_err(|_| SqliteAccountingRepositoryError::Planning)?;
        let values = SqliteReconciliationValues::new(command, occurred_at_unix_ms)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = load_sqlite_reconciliation_state(&transaction, &values)?;
        if validate_sqlite_reconciliation_replay(&state, &values)? {
            transaction.commit()?;
            return Ok(SqliteIdempotentWriteOutcome::Replayed);
        }
        apply_sqlite_usage_reconciliation(&transaction, &values, state.reservation_was_released)?;
        transaction.commit()?;
        Ok(SqliteIdempotentWriteOutcome::Applied)
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
                    AND virtual_key_id IS ?8
                    AND storage_scope = ?7
                    AND reserved_tokens = ?5
                    AND reserved_cost_micros = ?6
                    AND created_at_unix_ms = ?9
                    AND expires_at_unix_ms = ?10
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
                command.storage_key.virtual_key_id.map(|id| id.to_string()),
                to_i64(command.record.created_at_unix_ms)?,
                to_i64(command.record.expires_at_unix_ms)?,
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
              AND virtual_key_id IS ?8
              AND storage_scope = ?7
              AND reserved_tokens = ?5
              AND reserved_cost_micros = ?6
              AND created_at_unix_ms = ?9
              AND expires_at_unix_ms = ?10
              AND committed_at_unix_ms IS NULL
              AND released_at_unix_ms IS NULL
              AND expires_at_unix_ms <= ?4
            "#,
            params![
                tenant_id,
                reservation_id,
                call_id,
                now_unix_ms,
                reserved_tokens,
                reserved_cost_micros,
                storage_scope,
                command.storage_key.virtual_key_id.map(|id| id.to_string()),
                to_i64(command.record.created_at_unix_ms)?,
                to_i64(command.record.expires_at_unix_ms)?,
            ],
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

fn load_sqlite_reconciliation_state(
    transaction: &Transaction<'_>,
    values: &SqliteReconciliationValues,
) -> Result<SqliteReconciliationState, SqliteAccountingRepositoryError> {
    let (committed_at, released_at) = load_sqlite_reconciliation_reservation(transaction, values)?;
    validate_sqlite_reconciliation_counter_and_reservation(transaction, values)?;
    let state = SqliteReconciliationState {
        committed_at,
        reservation_was_released: released_at.is_some(),
        committed_usage: load_single_sqlite_ledger_usage(transaction, values, "committed")?,
        released_usage: load_single_sqlite_ledger_usage(transaction, values, "released")?,
    };
    validate_sqlite_reconciliation_release_state(&state, values)?;
    Ok(state)
}

fn load_sqlite_reconciliation_reservation(
    transaction: &Transaction<'_>,
    values: &SqliteReconciliationValues,
) -> Result<(Option<i64>, Option<i64>), SqliteAccountingRepositoryError> {
    let stored = transaction
        .query_row(
            "SELECT reservation_id, call_id, virtual_key_id, storage_scope,
                    reserved_tokens, reserved_cost_micros, created_at_unix_ms,
                    expires_at_unix_ms, committed_at_unix_ms, released_at_unix_ms
             FROM prodex_reservations
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3",
            params![&values.tenant_id, &values.reservation_id, &values.call_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                ))
            },
        )
        .optional()?;
    let Some((
        reservation_id,
        call_id,
        virtual_key_id,
        storage_scope,
        reserved_tokens,
        reserved_cost_micros,
        created_at,
        expires_at,
        committed_at,
        released_at,
    )) = stored
    else {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    };
    if reservation_id != values.reservation_id
        || call_id != values.call_id
        || virtual_key_id != values.virtual_key_id
        || storage_scope != values.storage_scope
        || reserved_tokens != values.reserved_tokens
        || reserved_cost_micros != values.reserved_cost_micros
        || created_at != values.created_at
        || expires_at != values.expires_at
    {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    }
    Ok((committed_at, released_at))
}

fn validate_sqlite_reconciliation_counter_and_reservation(
    transaction: &Transaction<'_>,
    values: &SqliteReconciliationValues,
) -> Result<(), SqliteAccountingRepositoryError> {
    let counter: Option<(Option<String>, String)> = transaction
        .query_row(
            "SELECT virtual_key_id, storage_scope
             FROM prodex_budget_counters
             WHERE tenant_id = ?1 AND storage_scope = ?2",
            params![&values.tenant_id, &values.storage_scope],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if counter != Some((values.virtual_key_id.clone(), values.storage_scope.clone())) {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    }
    let reserved_ledger: Option<(String, String, i64, i64)> = transaction
        .query_row(
            "SELECT reservation_id, call_id, tokens, cost_micros
             FROM prodex_usage_ledger
             WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3
               AND event_kind = 'reserved'",
            params![&values.tenant_id, &values.reservation_id, &values.call_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    if reserved_ledger
        != Some((
            values.reservation_id.clone(),
            values.call_id.clone(),
            values.reserved_tokens,
            values.reserved_cost_micros,
        ))
    {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    }
    Ok(())
}

fn load_single_sqlite_ledger_usage(
    transaction: &Transaction<'_>,
    values: &SqliteReconciliationValues,
    event_kind: &str,
) -> Result<Option<(i64, i64)>, SqliteAccountingRepositoryError> {
    let (count, tokens, cost_micros): (i64, Option<i64>, Option<i64>) = transaction.query_row(
        "SELECT COUNT(*), MIN(tokens), MIN(cost_micros)
         FROM prodex_usage_ledger
         WHERE tenant_id = ?1 AND reservation_id = ?2 AND call_id = ?3
           AND event_kind = ?4",
        params![
            &values.tenant_id,
            &values.reservation_id,
            &values.call_id,
            event_kind,
        ],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    match (count, tokens, cost_micros) {
        (0, None, None) => Ok(None),
        (1, Some(tokens), Some(cost_micros)) => Ok(Some((tokens, cost_micros))),
        _ => Err(SqliteAccountingRepositoryError::StateConflict),
    }
}

fn validate_sqlite_reconciliation_release_state(
    state: &SqliteReconciliationState,
    values: &SqliteReconciliationValues,
) -> Result<(), SqliteAccountingRepositoryError> {
    if state.reservation_was_released != state.released_usage.is_some() {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    }
    if state.reservation_was_released
        && state.committed_at.is_none()
        && state.released_usage != Some((values.reserved_tokens, values.reserved_cost_micros))
    {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    }
    Ok(())
}

fn validate_sqlite_reconciliation_replay(
    state: &SqliteReconciliationState,
    values: &SqliteReconciliationValues,
) -> Result<bool, SqliteAccountingRepositoryError> {
    if state.committed_at.is_none() {
        if state.committed_usage.is_some() {
            return Err(SqliteAccountingRepositoryError::StateConflict);
        }
        return Ok(false);
    }
    if state.committed_usage != Some((values.actual_tokens, values.actual_cost_micros)) {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    }
    let expected_released = (values.released_tokens, values.released_cost_micros);
    if (state.reservation_was_released
        && state.released_usage != Some((values.reserved_tokens, values.reserved_cost_micros))
        && state.released_usage != Some(expected_released))
        || (!state.reservation_was_released && expected_released != (0, 0))
    {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    }
    Ok(true)
}

fn apply_sqlite_usage_reconciliation(
    transaction: &Transaction<'_>,
    values: &SqliteReconciliationValues,
    reservation_was_released: bool,
) -> Result<(), SqliteAccountingRepositoryError> {
    let reserved_tokens_to_release = if reservation_was_released {
        0
    } else {
        values.reserved_tokens
    };
    let reserved_cost_to_release = if reservation_was_released {
        0
    } else {
        values.reserved_cost_micros
    };
    update_sqlite_reconciliation_counter(
        transaction,
        values,
        reserved_tokens_to_release,
        reserved_cost_to_release,
    )?;
    update_sqlite_reconciliation_reservation(
        transaction,
        values,
        reserved_tokens_to_release,
        reserved_cost_to_release,
    )?;
    insert_sqlite_reconciliation_ledger(transaction, values)
}

fn update_sqlite_reconciliation_counter(
    transaction: &Transaction<'_>,
    values: &SqliteReconciliationValues,
    reserved_tokens_to_release: i64,
    reserved_cost_to_release: i64,
) -> Result<(), SqliteAccountingRepositoryError> {
    let changed = transaction.execute(
        r#"
        UPDATE prodex_budget_counters
        SET reserved_tokens = reserved_tokens - ?4,
            reserved_cost_micros = reserved_cost_micros - ?5,
            committed_tokens = committed_tokens + ?6,
            committed_cost_micros = committed_cost_micros + ?7,
            updated_at_unix_ms = ?8
        WHERE tenant_id = ?1
          AND storage_scope = ?9
          AND reserved_tokens >= ?4
          AND reserved_cost_micros >= ?5
          AND EXISTS (
              SELECT 1
              FROM prodex_reservations
              WHERE tenant_id = ?1
                AND reservation_id = ?2
                AND call_id = ?3
                AND virtual_key_id IS ?10
                AND storage_scope = ?9
                AND reserved_tokens = ?11
                AND reserved_cost_micros = ?12
                AND created_at_unix_ms = ?13
                AND expires_at_unix_ms = ?14
                AND committed_at_unix_ms IS NULL
          )
        "#,
        params![
            &values.tenant_id,
            &values.reservation_id,
            &values.call_id,
            reserved_tokens_to_release,
            reserved_cost_to_release,
            values.actual_tokens,
            values.actual_cost_micros,
            values.occurred_at,
            &values.storage_scope,
            &values.virtual_key_id,
            values.reserved_tokens,
            values.reserved_cost_micros,
            values.created_at,
            values.expires_at,
        ],
    )?;
    if changed != 1 {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    }
    Ok(())
}

fn update_sqlite_reconciliation_reservation(
    transaction: &Transaction<'_>,
    values: &SqliteReconciliationValues,
    reserved_tokens_to_release: i64,
    reserved_cost_to_release: i64,
) -> Result<(), SqliteAccountingRepositoryError> {
    let changed = transaction.execute(
        r#"
        UPDATE prodex_reservations
        SET committed_at_unix_ms = ?8,
            released_at_unix_ms = CASE
                WHEN released_at_unix_ms IS NOT NULL THEN released_at_unix_ms
                WHEN ?10 > 0 OR ?11 > 0 THEN ?8
                ELSE NULL
            END
        WHERE tenant_id = ?1
          AND reservation_id = ?2
          AND call_id = ?3
          AND virtual_key_id IS ?9
          AND storage_scope = ?12
          AND reserved_tokens = ?13
          AND reserved_cost_micros = ?14
          AND created_at_unix_ms = ?15
          AND expires_at_unix_ms = ?16
          AND committed_at_unix_ms IS NULL
        "#,
        params![
            &values.tenant_id,
            &values.reservation_id,
            &values.call_id,
            reserved_tokens_to_release,
            reserved_cost_to_release,
            values.actual_tokens,
            values.actual_cost_micros,
            values.occurred_at,
            &values.virtual_key_id,
            values.released_tokens,
            values.released_cost_micros,
            &values.storage_scope,
            values.reserved_tokens,
            values.reserved_cost_micros,
            values.created_at,
            values.expires_at,
        ],
    )?;
    if changed != 1 {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    }
    Ok(())
}

fn insert_sqlite_reconciliation_ledger(
    transaction: &Transaction<'_>,
    values: &SqliteReconciliationValues,
) -> Result<(), SqliteAccountingRepositoryError> {
    transaction.execute(
        r#"
        INSERT INTO prodex_usage_ledger (
            tenant_id, ledger_event_id, reservation_id, call_id, event_kind,
            tokens, cost_micros, occurred_at_unix_ms
        ) VALUES (?1, ?2, ?3, ?4, 'committed', ?5, ?6, ?7)
        "#,
        params![
            &values.tenant_id,
            RequestId::new().to_string(),
            &values.reservation_id,
            &values.call_id,
            values.actual_tokens,
            values.actual_cost_micros,
            values.occurred_at,
        ],
    )?;
    if values.released_tokens > 0 || values.released_cost_micros > 0 {
        transaction.execute(
            r#"
            INSERT INTO prodex_usage_ledger (
                tenant_id, ledger_event_id, reservation_id, call_id, event_kind,
                tokens, cost_micros, occurred_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?4, 'released', ?5, ?6, ?7)
            "#,
            params![
                &values.tenant_id,
                RequestId::new().to_string(),
                &values.reservation_id,
                &values.call_id,
                values.released_tokens,
                values.released_cost_micros,
                values.occurred_at,
            ],
        )?;
    }
    Ok(())
}

fn replayed_or_conflict(
    transaction: rusqlite::Transaction<'_>,
    command: &ExpiredReservationRecoveryCommand,
) -> Result<SqliteIdempotentWriteOutcome, SqliteAccountingRepositoryError> {
    let stored = transaction
        .query_row(
            r#"
            SELECT virtual_key_id, storage_scope, reserved_tokens, reserved_cost_micros,
                   created_at_unix_ms, expires_at_unix_ms,
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
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                ))
            },
        )
        .optional()?;
    let Some((
        virtual_key_id,
        storage_scope,
        tokens,
        cost_micros,
        created_at,
        expires_at,
        committed_at,
        released_at,
    )) = stored
    else {
        return Err(SqliteAccountingRepositoryError::StateConflict);
    };
    if virtual_key_id == command.storage_key.virtual_key_id.map(|id| id.to_string())
        && storage_scope == command.storage_key.storage_scope()
        && from_i64(tokens)? == command.record.reserved.tokens
        && from_i64(cost_micros)? == command.record.reserved.cost_micros
        && from_i64(created_at)? == command.record.created_at_unix_ms
        && from_i64(expires_at)? == command.record.expires_at_unix_ms
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
    fn connection_enables_foreign_keys() {
        let repository = SqliteAccountingRepository::from_connection(
            Connection::open_in_memory().expect("in-memory SQLite connection"),
        );
        let enabled: i64 = repository
            .connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign_keys pragma");
        assert_eq!(enabled, 1);
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

    #[test]
    fn expired_reservation_replay_requires_exact_record_without_mutation() {
        let (mut repository, mut command) = repository_with_expired_reservation();
        command.record.reserved = UsageAmount::new(9, 19);

        assert_eq!(
            repository.release_expired(command),
            Err(SqliteAccountingRepositoryError::StateConflict)
        );
        let counters: (i64, i64) = repository
            .connection
            .query_row(
                "SELECT reserved_tokens, reserved_cost_micros FROM prodex_budget_counters",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(counters, (10, 20));
        let ledger_count: i64 = repository
            .connection
            .query_row("SELECT COUNT(*) FROM prodex_usage_ledger", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(ledger_count, 0);
    }
}

#![forbid(unsafe_code)]
//! Async pooled execution for the PostgreSQL accounting plans.
//!
//! Pool construction requires the caller to supply a TLS connector. The only
//! no-TLS convenience API is deliberately named `create_pool_explicit_no_tls`.
//! This crate does not execute migrations or any other DDL.

use std::{fmt, future::Future, time::Duration};

mod tls;
mod types;
pub use tls::{PostgresTlsConfig, PostgresTlsMode, connect_blocking};
pub use types::{
    IdempotentWriteOutcome, PostgresRuntimeError, ReserveOutcome, ReserveRejection,
    StoredReservation, StoredReservationState,
};

use deadpool_postgres::{
    Config as DeadpoolConfig, ManagerConfig, Pool, PoolConfig, RecyclingMethod, Runtime, SslMode,
    Timeouts,
};
use prodex_domain::{
    CallId, IdempotencyKey, RequestId, ReservationId, ReservationRecord, TenantId, UsageAmount,
    VirtualKeyId,
};
#[cfg(test)]
use prodex_storage::TenantStorageKey;
use prodex_storage::{
    AtomicReservationCommand, ExpiredReservationRecoveryCommand, UsageReconciliationCommand,
    plan_atomic_reservation,
};
use prodex_storage_postgres::{
    ATOMIC_RESERVATION_STATEMENT, RECONCILE_USAGE_STATEMENT, RECOVER_EXPIRED_RESERVATION_STATEMENT,
    SET_TENANT_STATEMENT, plan_postgres_atomic_reservation,
    plan_postgres_expired_reservation_recovery, plan_postgres_usage_reconciliation,
};
use tokio_postgres::tls::{MakeTlsConnect, TlsConnect};
use tokio_postgres::{IsolationLevel, Row, Socket};
use uuid::Uuid;

const DEFAULT_MAX_SERIALIZATION_RETRIES: usize = 3;
const DEFAULT_OPERATION_TIMEOUT: Duration = Duration::from_secs(5);

/// Read-only SQL required for cross-replica reservation reconciliation.
///
/// The driver-free plan crate does not currently expose a reservation lookup
/// plan. All mutating SQL continues to come from `prodex-storage-postgres`.
pub const LOAD_RESERVATION_BY_CALL_SQL: &str = r#"
SELECT
    reservation.tenant_id,
    reservation.reservation_id,
    reservation.call_id,
    reservation.virtual_key_id,
    reservation.idempotency_key,
    reservation.reserved_tokens,
    reservation.reserved_cost_micros,
    reservation.created_at_unix_ms,
    reservation.expires_at_unix_ms,
    reservation.committed_at_unix_ms,
    reservation.released_at_unix_ms,
    committed.tokens AS committed_tokens,
    committed.cost_micros AS committed_cost_micros,
    released.tokens AS released_tokens,
    released.cost_micros AS released_cost_micros
FROM prodex_reservations AS reservation
LEFT JOIN prodex_usage_ledger AS committed
    ON committed.tenant_id = reservation.tenant_id
   AND committed.reservation_id = reservation.reservation_id
   AND committed.event_kind = 'committed'
LEFT JOIN prodex_usage_ledger AS released
    ON released.tenant_id = reservation.tenant_id
   AND released.reservation_id = reservation.reservation_id
   AND released.event_kind = 'released'
WHERE reservation.tenant_id = $1
  AND reservation.call_id = $2
"#;

const LOAD_RESERVATION_BY_IDEMPOTENCY_SQL: &str = r#"
SELECT
    reservation.tenant_id,
    reservation.reservation_id,
    reservation.call_id,
    reservation.virtual_key_id,
    reservation.idempotency_key,
    reservation.reserved_tokens,
    reservation.reserved_cost_micros,
    reservation.created_at_unix_ms,
    reservation.expires_at_unix_ms,
    reservation.committed_at_unix_ms,
    reservation.released_at_unix_ms,
    committed.tokens AS committed_tokens,
    committed.cost_micros AS committed_cost_micros,
    released.tokens AS released_tokens,
    released.cost_micros AS released_cost_micros
FROM prodex_reservations AS reservation
LEFT JOIN prodex_usage_ledger AS committed
    ON committed.tenant_id = reservation.tenant_id
   AND committed.reservation_id = reservation.reservation_id
   AND committed.event_kind = 'committed'
LEFT JOIN prodex_usage_ledger AS released
    ON released.tenant_id = reservation.tenant_id
   AND released.reservation_id = reservation.reservation_id
   AND released.event_kind = 'released'
WHERE reservation.tenant_id = $1
  AND reservation.idempotency_key = $2
"#;

#[derive(Clone, PartialEq, Eq)]
pub struct PostgresRuntimeConfig {
    database_url: String,
    max_pool_size: usize,
    max_serialization_retries: usize,
    operation_timeout: Duration,
}

impl PostgresRuntimeConfig {
    pub fn new(
        database_url: impl Into<String>,
        max_pool_size: usize,
    ) -> Result<Self, PostgresRuntimeError> {
        let database_url = database_url.into();
        if database_url.trim().is_empty()
            || database_url.parse::<tokio_postgres::Config>().is_err()
            || max_pool_size == 0
        {
            return Err(PostgresRuntimeError::Configuration);
        }
        Ok(Self {
            database_url,
            max_pool_size,
            max_serialization_retries: DEFAULT_MAX_SERIALIZATION_RETRIES,
            operation_timeout: DEFAULT_OPERATION_TIMEOUT,
        })
    }

    pub fn with_max_serialization_retries(mut self, retries: usize) -> Self {
        self.max_serialization_retries = retries;
        self
    }

    pub fn max_pool_size(&self) -> usize {
        self.max_pool_size
    }

    pub fn max_serialization_retries(&self) -> usize {
        self.max_serialization_retries
    }

    pub fn with_operation_timeout(
        mut self,
        timeout: Duration,
    ) -> Result<Self, PostgresRuntimeError> {
        if timeout.is_zero() {
            return Err(PostgresRuntimeError::Configuration);
        }
        self.operation_timeout = timeout;
        Ok(self)
    }

    pub fn operation_timeout(&self) -> Duration {
        self.operation_timeout
    }

    /// Builds a pool with the caller-provided TLS implementation.
    pub fn create_pool_with_tls<T>(&self, tls: T) -> Result<Pool, PostgresRuntimeError>
    where
        T: MakeTlsConnect<Socket> + Clone + Sync + Send + 'static,
        T::Stream: Sync + Send,
        T::TlsConnect: Sync + Send,
        <T::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        self.create_pool_with_tls_and_ssl_mode(tls, None)
    }

    fn create_pool_with_tls_and_ssl_mode<T>(
        &self,
        tls: T,
        ssl_mode: Option<SslMode>,
    ) -> Result<Pool, PostgresRuntimeError>
    where
        T: MakeTlsConnect<Socket> + Clone + Sync + Send + 'static,
        T::Stream: Sync + Send,
        T::TlsConnect: Sync + Send,
        <T::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        let mut config = DeadpoolConfig::new();
        config.url = Some(self.database_url.clone());
        config.ssl_mode = ssl_mode;
        config.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Verified,
        });
        let mut pool = PoolConfig::new(self.max_pool_size);
        pool.timeouts = Timeouts {
            wait: Some(self.operation_timeout),
            create: Some(self.operation_timeout),
            recycle: Some(self.operation_timeout),
        };
        config.pool = Some(pool);
        config
            .create_pool(Some(Runtime::Tokio1), tls)
            .map_err(|_| PostgresRuntimeError::PoolBuild)
    }

    /// Builds an unencrypted pool only when the caller opts into that choice by name.
    pub fn create_pool_explicit_no_tls(&self) -> Result<Pool, PostgresRuntimeError> {
        self.create_pool_with_tls_and_ssl_mode(tokio_postgres::NoTls, Some(SslMode::Disable))
    }

    pub fn create_pool_with_tls_config(
        &self,
        tls: &PostgresTlsConfig,
    ) -> Result<Pool, PostgresRuntimeError> {
        match tls.mode() {
            PostgresTlsMode::Disable => self.create_pool_explicit_no_tls(),
            PostgresTlsMode::VerifyFull => self
                .create_pool_with_tls_and_ssl_mode(tls.rustls_connector()?, Some(SslMode::Require)),
        }
    }
}

impl fmt::Debug for PostgresRuntimeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresRuntimeConfig")
            .field("database_url", &"<redacted>")
            .field("max_pool_size", &self.max_pool_size)
            .field("max_serialization_retries", &self.max_serialization_retries)
            .field("operation_timeout", &self.operation_timeout)
            .finish()
    }
}

#[derive(Clone)]
pub struct PostgresRepository {
    pool: Pool,
    max_serialization_retries: usize,
    operation_timeout: Duration,
}

impl PostgresRepository {
    pub fn from_pool(pool: Pool) -> Self {
        Self {
            pool,
            max_serialization_retries: DEFAULT_MAX_SERIALIZATION_RETRIES,
            operation_timeout: DEFAULT_OPERATION_TIMEOUT,
        }
    }

    pub fn from_pool_with_config(pool: Pool, config: &PostgresRuntimeConfig) -> Self {
        Self {
            pool,
            max_serialization_retries: config.max_serialization_retries,
            operation_timeout: config.operation_timeout,
        }
    }

    pub fn from_config_with_tls<T>(
        config: &PostgresRuntimeConfig,
        tls: T,
    ) -> Result<Self, PostgresRuntimeError>
    where
        T: MakeTlsConnect<Socket> + Clone + Sync + Send + 'static,
        T::Stream: Sync + Send,
        T::TlsConnect: Sync + Send,
        <T::TlsConnect as TlsConnect<Socket>>::Future: Send,
    {
        let pool = config.create_pool_with_tls(tls)?;
        Ok(Self::from_pool_with_config(pool, config))
    }

    pub fn from_config_explicit_no_tls(
        config: &PostgresRuntimeConfig,
    ) -> Result<Self, PostgresRuntimeError> {
        let pool = config.create_pool_explicit_no_tls()?;
        Ok(Self::from_pool_with_config(pool, config))
    }

    pub fn from_config_with_tls_config(
        config: &PostgresRuntimeConfig,
        tls: &PostgresTlsConfig,
    ) -> Result<Self, PostgresRuntimeError> {
        let pool = config.create_pool_with_tls_config(tls)?;
        Ok(Self::from_pool_with_config(pool, config))
    }

    pub async fn reserve(
        &self,
        command: AtomicReservationCommand,
    ) -> Result<ReserveOutcome, PostgresRuntimeError> {
        self.with_timeout(self.reserve_inner(command)).await
    }

    async fn reserve_inner(
        &self,
        command: AtomicReservationCommand,
    ) -> Result<ReserveOutcome, PostgresRuntimeError> {
        plan_postgres_atomic_reservation(command.clone())
            .map_err(|_| PostgresRuntimeError::Planning)?;
        let record = plan_atomic_reservation(command.clone())
            .map_err(|_| PostgresRuntimeError::Planning)?
            .reservation_record;
        let arguments = ReserveArguments::try_from_command(&command)?;

        for attempt in 0..=self.max_serialization_retries {
            match self.reserve_once(&command, record, &arguments).await {
                Err(AttemptError::SerializationFailure)
                    if attempt < self.max_serialization_retries => {}
                Err(AttemptError::SerializationFailure) => {
                    return Err(PostgresRuntimeError::Database);
                }
                Err(AttemptError::UniqueConflict) => {
                    return Ok(ReserveOutcome::Rejected(ReserveRejection::Conflict));
                }
                Err(AttemptError::Runtime(error)) => return Err(error),
                Ok(outcome) => return Ok(outcome),
            }
        }
        Err(PostgresRuntimeError::Database)
    }

    async fn reserve_once(
        &self,
        command: &AtomicReservationCommand,
        record: ReservationRecord,
        arguments: &ReserveArguments,
    ) -> Result<ReserveOutcome, AttemptError> {
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| AttemptError::Runtime(PostgresRuntimeError::PoolUnavailable))?;
        let transaction = client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .await
            .map_err(classify_database_attempt_error)?;
        set_tenant_context(&transaction, command.request.tenant_id)
            .await
            .map_err(classify_database_attempt_error)?;

        let statement = transaction
            .prepare_cached(ATOMIC_RESERVATION_STATEMENT.sql)
            .await
            .map_err(classify_database_attempt_error)?;
        let reserved = transaction
            .query_opt(
                &statement,
                &[
                    &command.request.tenant_id.as_uuid(),
                    &command.idempotency_key.as_str(),
                    &arguments.storage_scope,
                    &command
                        .storage_key
                        .virtual_key_id
                        .map(VirtualKeyId::as_uuid),
                    &arguments.reserved_tokens,
                    &arguments.reserved_cost_micros,
                    &arguments.created_at_unix_ms,
                    &arguments.max_tokens,
                    &arguments.max_cost_micros,
                    &arguments.max_requests,
                    &command.request.reservation_id.as_uuid(),
                    &command.request.call_id.as_uuid(),
                    &arguments.expires_at_unix_ms,
                    &RequestId::new().as_uuid(),
                ],
            )
            .await
            .map_err(classify_database_attempt_error)?;

        let outcome = if reserved.is_some() {
            ReserveOutcome::Reserved(record)
        } else if let Some(stored) = load_by_call_in_transaction(
            &transaction,
            command.request.tenant_id,
            command.request.call_id,
        )
        .await
        .map_err(AttemptError::Runtime)?
        {
            classify_existing_reservation(command, record, stored)
        } else if let Some(stored) = load_by_idempotency_in_transaction(
            &transaction,
            command.request.tenant_id,
            &command.idempotency_key,
        )
        .await
        .map_err(AttemptError::Runtime)?
        {
            classify_existing_reservation(command, record, stored)
        } else {
            ReserveOutcome::Rejected(
                classify_reservation_limit_rejection(
                    &transaction,
                    command.request.tenant_id,
                    &arguments.storage_scope,
                    arguments.max_requests,
                )
                .await
                .map_err(AttemptError::Runtime)?,
            )
        };

        transaction
            .commit()
            .await
            .map_err(classify_database_attempt_error)?;
        Ok(outcome)
    }

    pub async fn reconcile_usage(
        &self,
        command: UsageReconciliationCommand,
        occurred_at_unix_ms: u64,
    ) -> Result<IdempotentWriteOutcome, PostgresRuntimeError> {
        self.with_timeout(self.reconcile_usage_inner(command, occurred_at_unix_ms))
            .await
    }

    async fn reconcile_usage_inner(
        &self,
        command: UsageReconciliationCommand,
        occurred_at_unix_ms: u64,
    ) -> Result<IdempotentWriteOutcome, PostgresRuntimeError> {
        plan_postgres_usage_reconciliation(command.clone())
            .map_err(|_| PostgresRuntimeError::Planning)?;
        let occurred_at_unix_ms = to_i64(occurred_at_unix_ms)?;
        let reserved = usage_to_i64(command.record.reserved)?;
        let actual = usage_to_i64(command.actual)?;
        let released = command.record.reserved.saturating_sub(command.actual);
        let released = usage_to_i64(released)?;
        let storage_scope = command.storage_key.storage_scope();

        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| PostgresRuntimeError::PoolUnavailable)?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        set_tenant_context(&transaction, command.record.tenant_id)
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        let statement = transaction
            .prepare_cached(RECONCILE_USAGE_STATEMENT.sql)
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        let row = transaction
            .query_opt(
                &statement,
                &[
                    &command.record.tenant_id.as_uuid(),
                    &command.record.reservation_id.as_uuid(),
                    &command.record.call_id.as_uuid(),
                    &reserved.tokens,
                    &reserved.cost_micros,
                    &actual.tokens,
                    &actual.cost_micros,
                    &occurred_at_unix_ms,
                    &storage_scope,
                    &released.tokens,
                    &released.cost_micros,
                    &RequestId::new().as_uuid(),
                    &RequestId::new().as_uuid(),
                ],
            )
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;

        let outcome = if row.is_some() {
            IdempotentWriteOutcome::Applied
        } else {
            let stored = load_by_call_in_transaction(
                &transaction,
                command.record.tenant_id,
                command.record.call_id,
            )
            .await?
            .ok_or(PostgresRuntimeError::StateConflict)?;
            if stored.record != command.record
                || stored.virtual_key_id != command.storage_key.virtual_key_id
                || stored.committed_usage != Some(command.actual)
            {
                return Err(PostgresRuntimeError::StateConflict);
            }
            IdempotentWriteOutcome::Replayed
        };
        transaction
            .commit()
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        Ok(outcome)
    }

    pub async fn release_expired(
        &self,
        command: ExpiredReservationRecoveryCommand,
    ) -> Result<IdempotentWriteOutcome, PostgresRuntimeError> {
        self.with_timeout(self.release_expired_inner(command)).await
    }

    async fn release_expired_inner(
        &self,
        command: ExpiredReservationRecoveryCommand,
    ) -> Result<IdempotentWriteOutcome, PostgresRuntimeError> {
        plan_postgres_expired_reservation_recovery(command.clone())
            .map_err(|_| PostgresRuntimeError::Planning)?;
        let now_unix_ms = to_i64(command.now_unix_ms)?;
        let reserved = usage_to_i64(command.record.reserved)?;
        let storage_scope = command.storage_key.storage_scope();

        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| PostgresRuntimeError::PoolUnavailable)?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        set_tenant_context(&transaction, command.record.tenant_id)
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        let statement = transaction
            .prepare_cached(RECOVER_EXPIRED_RESERVATION_STATEMENT.sql)
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        let row = transaction
            .query_opt(
                &statement,
                &[
                    &command.record.tenant_id.as_uuid(),
                    &command.record.reservation_id.as_uuid(),
                    &command.record.call_id.as_uuid(),
                    &now_unix_ms,
                    &reserved.tokens,
                    &reserved.cost_micros,
                    &storage_scope,
                    &RequestId::new().as_uuid(),
                ],
            )
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;

        let outcome = if row.is_some() {
            IdempotentWriteOutcome::Applied
        } else {
            let stored = load_by_call_in_transaction(
                &transaction,
                command.record.tenant_id,
                command.record.call_id,
            )
            .await?
            .ok_or(PostgresRuntimeError::StateConflict)?;
            if stored.record != command.record
                || stored.virtual_key_id != command.storage_key.virtual_key_id
                || stored.state != StoredReservationState::Released
                || stored.released_usage != Some(command.record.reserved)
            {
                return Err(PostgresRuntimeError::StateConflict);
            }
            IdempotentWriteOutcome::Replayed
        };
        transaction
            .commit()
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        Ok(outcome)
    }

    pub async fn load_reservation(
        &self,
        tenant_id: TenantId,
        call_id: CallId,
    ) -> Result<Option<StoredReservation>, PostgresRuntimeError> {
        self.with_timeout(self.load_reservation_inner(tenant_id, call_id))
            .await
    }

    async fn load_reservation_inner(
        &self,
        tenant_id: TenantId,
        call_id: CallId,
    ) -> Result<Option<StoredReservation>, PostgresRuntimeError> {
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| PostgresRuntimeError::PoolUnavailable)?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        let reservation = load_by_call_in_transaction(&transaction, tenant_id, call_id).await?;
        transaction
            .commit()
            .await
            .map_err(|_| PostgresRuntimeError::Database)?;
        Ok(reservation)
    }

    async fn with_timeout<T>(
        &self,
        operation: impl Future<Output = Result<T, PostgresRuntimeError>>,
    ) -> Result<T, PostgresRuntimeError> {
        tokio::time::timeout(self.operation_timeout, operation)
            .await
            .map_err(|_| PostgresRuntimeError::Timeout)?
    }
}

impl fmt::Debug for PostgresRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresRepository")
            .field("pool", &"<redacted>")
            .field("max_serialization_retries", &self.max_serialization_retries)
            .field("operation_timeout", &self.operation_timeout)
            .finish()
    }
}

#[derive(Clone, Copy)]
struct SignedUsageAmount {
    tokens: i64,
    cost_micros: i64,
}

struct ReserveArguments {
    storage_scope: String,
    reserved_tokens: i64,
    reserved_cost_micros: i64,
    created_at_unix_ms: i64,
    max_tokens: i64,
    max_cost_micros: i64,
    max_requests: i64,
    expires_at_unix_ms: i64,
}

impl ReserveArguments {
    fn try_from_command(command: &AtomicReservationCommand) -> Result<Self, PostgresRuntimeError> {
        let reserved = usage_to_i64(command.request.estimate)?;
        let limit = usage_to_i64(command.limit.max)?;
        let expires_at_unix_ms = command
            .created_at_unix_ms
            .checked_add(command.ttl_ms)
            .ok_or(PostgresRuntimeError::NumericOverflow)?;
        Ok(Self {
            storage_scope: command.storage_key.storage_scope(),
            reserved_tokens: reserved.tokens,
            reserved_cost_micros: reserved.cost_micros,
            created_at_unix_ms: to_i64(command.created_at_unix_ms)?,
            max_tokens: limit.tokens,
            max_cost_micros: limit.cost_micros,
            max_requests: if command.limit.max_requests == u64::MAX {
                i64::MAX
            } else {
                to_i64(command.limit.max_requests)?
            },
            expires_at_unix_ms: to_i64(expires_at_unix_ms)?,
        })
    }
}

enum AttemptError {
    SerializationFailure,
    UniqueConflict,
    Runtime(PostgresRuntimeError),
}

fn classify_database_attempt_error(error: tokio_postgres::Error) -> AttemptError {
    match error.code() {
        Some(code) if *code == tokio_postgres::error::SqlState::T_R_SERIALIZATION_FAILURE => {
            AttemptError::SerializationFailure
        }
        Some(code) if *code == tokio_postgres::error::SqlState::UNIQUE_VIOLATION => {
            AttemptError::UniqueConflict
        }
        _ => AttemptError::Runtime(PostgresRuntimeError::Database),
    }
}

fn classify_existing_reservation(
    command: &AtomicReservationCommand,
    expected: ReservationRecord,
    stored: StoredReservation,
) -> ReserveOutcome {
    if stored.record == expected
        && stored.virtual_key_id == command.storage_key.virtual_key_id
        && stored.idempotency_key == command.idempotency_key
    {
        ReserveOutcome::Replayed(stored)
    } else {
        ReserveOutcome::Rejected(ReserveRejection::Conflict)
    }
}

async fn set_tenant_context(
    transaction: &deadpool_postgres::Transaction<'_>,
    tenant_id: TenantId,
) -> Result<(), tokio_postgres::Error> {
    let statement = transaction.prepare_cached(SET_TENANT_STATEMENT.sql).await?;
    transaction
        .query_one(&statement, &[&tenant_id.to_string()])
        .await?;
    Ok(())
}

async fn load_by_call_in_transaction(
    transaction: &deadpool_postgres::Transaction<'_>,
    tenant_id: TenantId,
    call_id: CallId,
) -> Result<Option<StoredReservation>, PostgresRuntimeError> {
    let statement = transaction
        .prepare_cached(LOAD_RESERVATION_BY_CALL_SQL)
        .await
        .map_err(|_| PostgresRuntimeError::Database)?;
    let row = transaction
        .query_opt(&statement, &[&tenant_id.as_uuid(), &call_id.as_uuid()])
        .await
        .map_err(|_| PostgresRuntimeError::Database)?;
    row.map(stored_reservation_from_row).transpose()
}

async fn load_by_idempotency_in_transaction(
    transaction: &deadpool_postgres::Transaction<'_>,
    tenant_id: TenantId,
    idempotency_key: &IdempotencyKey,
) -> Result<Option<StoredReservation>, PostgresRuntimeError> {
    let statement = transaction
        .prepare_cached(LOAD_RESERVATION_BY_IDEMPOTENCY_SQL)
        .await
        .map_err(|_| PostgresRuntimeError::Database)?;
    let row = transaction
        .query_opt(
            &statement,
            &[&tenant_id.as_uuid(), &idempotency_key.as_str()],
        )
        .await
        .map_err(|_| PostgresRuntimeError::Database)?;
    row.map(stored_reservation_from_row).transpose()
}

fn stored_reservation_from_row(row: Row) -> Result<StoredReservation, PostgresRuntimeError> {
    let tenant_id = TenantId::from_uuid(row.get::<_, Uuid>("tenant_id"));
    let reservation_id = ReservationId::from_uuid(row.get::<_, Uuid>("reservation_id"));
    let call_id = CallId::from_uuid(row.get::<_, Uuid>("call_id"));
    let virtual_key_id = row
        .get::<_, Option<Uuid>>("virtual_key_id")
        .map(VirtualKeyId::from_uuid);
    let idempotency_key = IdempotencyKey::new(row.get::<_, String>("idempotency_key"))
        .map_err(|_| PostgresRuntimeError::InvalidDatabaseState)?;
    let committed_at = row.get::<_, Option<i64>>("committed_at_unix_ms");
    let released_at = row.get::<_, Option<i64>>("released_at_unix_ms");
    let committed_usage =
        optional_usage_from_row(&row, "committed_tokens", "committed_cost_micros")?;
    let released_usage = optional_usage_from_row(&row, "released_tokens", "released_cost_micros")?;
    let state = if committed_at.is_some() {
        StoredReservationState::Committed
    } else if released_at.is_some() {
        StoredReservationState::Released
    } else {
        StoredReservationState::Active
    };
    Ok(StoredReservation {
        record: ReservationRecord {
            tenant_id,
            call_id,
            reservation_id,
            reserved: UsageAmount::new(
                from_i64(row.get("reserved_tokens"))?,
                from_i64(row.get("reserved_cost_micros"))?,
            ),
            created_at_unix_ms: from_i64(row.get("created_at_unix_ms"))?,
            expires_at_unix_ms: from_i64(row.get("expires_at_unix_ms"))?,
        },
        virtual_key_id,
        idempotency_key,
        state,
        committed_usage,
        released_usage,
    })
}

async fn classify_reservation_limit_rejection(
    transaction: &tokio_postgres::Transaction<'_>,
    tenant_id: TenantId,
    storage_scope: &str,
    max_requests: i64,
) -> Result<ReserveRejection, PostgresRuntimeError> {
    let row = transaction
        .query_opt(
            "SELECT request_count FROM prodex_budget_counters WHERE tenant_id = $1 AND storage_scope = $2",
            &[&tenant_id.as_uuid(), &storage_scope],
        )
        .await
        .map_err(|_| PostgresRuntimeError::Database)?;
    if row
        .map(|row| row.get::<_, i64>("request_count"))
        .is_some_and(|requests| requests.saturating_add(1) > max_requests)
    {
        return Ok(ReserveRejection::RequestBudgetExceeded);
    }
    Ok(ReserveRejection::BudgetLimitExceeded)
}

fn optional_usage_from_row(
    row: &Row,
    tokens_column: &str,
    cost_column: &str,
) -> Result<Option<UsageAmount>, PostgresRuntimeError> {
    match (
        row.get::<_, Option<i64>>(tokens_column),
        row.get::<_, Option<i64>>(cost_column),
    ) {
        (Some(tokens), Some(cost_micros)) => Ok(Some(UsageAmount::new(
            from_i64(tokens)?,
            from_i64(cost_micros)?,
        ))),
        (None, None) => Ok(None),
        _ => Err(PostgresRuntimeError::InvalidDatabaseState),
    }
}

fn usage_to_i64(amount: UsageAmount) -> Result<SignedUsageAmount, PostgresRuntimeError> {
    Ok(SignedUsageAmount {
        tokens: to_i64(amount.tokens)?,
        cost_micros: to_i64(amount.cost_micros)?,
    })
}

fn to_i64(value: u64) -> Result<i64, PostgresRuntimeError> {
    i64::try_from(value).map_err(|_| PostgresRuntimeError::NumericOverflow)
}

fn from_i64(value: i64) -> Result<u64, PostgresRuntimeError> {
    u64::try_from(value).map_err(|_| PostgresRuntimeError::InvalidDatabaseState)
}

#[cfg(test)]
mod tests;

use std::{error::Error, fmt};

use prodex_domain::{BudgetSnapshot, IdempotencyKey, ReservationRecord, UsageAmount, VirtualKeyId};
use prodex_storage::TenantStorageKey;

#[derive(Clone, PartialEq, Eq)]
pub struct ExpiredReservationCandidate {
    pub storage_key: TenantStorageKey,
    pub snapshot: BudgetSnapshot,
    pub record: ReservationRecord,
}

impl fmt::Debug for ExpiredReservationCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpiredReservationCandidate")
            .field("storage_key", &"<redacted>")
            .field("snapshot", &"<redacted>")
            .field("record", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ReserveOutcome {
    Reserved(ReservationRecord),
    Replayed(StoredReservation),
    Rejected(ReserveRejection),
}

impl fmt::Debug for ReserveOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved(_) => f.debug_tuple("Reserved").field(&"<redacted>").finish(),
            Self::Replayed(_) => f.debug_tuple("Replayed").field(&"<redacted>").finish(),
            Self::Rejected(reason) => f.debug_tuple("Rejected").field(reason).finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReserveRejection {
    BudgetLimitExceeded,
    RequestBudgetExceeded,
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdempotentWriteOutcome {
    Applied,
    Replayed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredReservationState {
    Active,
    Committed,
    Released,
}

#[derive(Clone, PartialEq, Eq)]
pub struct StoredReservation {
    pub record: ReservationRecord,
    pub virtual_key_id: Option<VirtualKeyId>,
    pub idempotency_key: IdempotencyKey,
    pub state: StoredReservationState,
    pub committed_usage: Option<UsageAmount>,
    pub released_usage: Option<UsageAmount>,
}

impl fmt::Debug for StoredReservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredReservation")
            .field("record", &"<redacted>")
            .field("virtual_key_id", &"<redacted>")
            .field("idempotency_key", &"<redacted>")
            .field("state", &self.state)
            .field("committed_usage", &"<redacted>")
            .field("released_usage", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PostgresRuntimeError {
    Configuration,
    PoolBuild,
    PoolUnavailable,
    Timeout,
    Planning,
    NumericOverflow,
    Database,
    InvalidDatabaseState,
    StateConflict,
}

impl fmt::Debug for PostgresRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "Configuration",
            Self::PoolBuild => "PoolBuild",
            Self::PoolUnavailable => "PoolUnavailable",
            Self::Timeout => "Timeout",
            Self::Planning => "Planning",
            Self::NumericOverflow => "NumericOverflow",
            Self::Database => "Database",
            Self::InvalidDatabaseState => "InvalidDatabaseState",
            Self::StateConflict => "StateConflict",
        })
    }
}

impl fmt::Display for PostgresRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "PostgreSQL runtime configuration is invalid",
            Self::PoolBuild => "PostgreSQL connection pool configuration is invalid",
            Self::PoolUnavailable => "PostgreSQL connection pool is unavailable",
            Self::Timeout => "PostgreSQL storage operation timed out",
            Self::Planning => "PostgreSQL storage operation is invalid",
            Self::NumericOverflow => "PostgreSQL storage value is out of range",
            Self::Database => "PostgreSQL storage operation failed",
            Self::InvalidDatabaseState => "PostgreSQL storage state is invalid",
            Self::StateConflict => "PostgreSQL storage state conflicts with the request",
        })
    }
}

impl Error for PostgresRuntimeError {}

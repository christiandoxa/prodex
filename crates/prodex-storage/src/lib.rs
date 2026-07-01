#![forbid(unsafe_code)]
//! Storage boundary contracts for tenant-scoped atomic operations.
//!
//! This crate defines adapter-neutral storage commands and validation helpers.
//! It intentionally contains no database driver, filesystem, network, async
//! runtime, or HTTP dependency. Postgres/Redis/SQLite crates can implement these
//! contracts in separate adapter crates.

use std::error::Error;
use std::fmt;

use prodex_domain::{
    AuditDigest, AuditEnvelope, AuditEvent, AuditExportPlan, AuditRetentionPurgeBatch,
    AuditSortOrder, BudgetLimit, BudgetSnapshot, IdempotencyEntry, IdempotencyKey,
    IdempotencyRecord, IdempotentOperation, LedgerEvent, PrincipalId, ProviderCredentialId,
    ReservationReconciliation, ReservationReconciliationReason, ReservationRecord,
    ReservationRecoveryError, ReservationRequest, Role, RoleBindingId, SecretRef, TenantId,
    UsageAmount, VirtualKeyId, reconcile_reserved_usage, release_expired_reservation,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurableStoreKind {
    Postgres,
    Sqlite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheStoreKind {
    Redis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationRuntimePolicy {
    ExternalMigratorOnly,
    RequestPathForbidden,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StorageTopology {
    pub durable_store: DurableStoreKind,
    pub cache_store: Option<CacheStoreKind>,
    pub migration_policy: MigrationRuntimePolicy,
}

impl fmt::Debug for StorageTopology {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorageTopology")
            .field("durable_store", &"<redacted>")
            .field("cache_store", &"<redacted>")
            .field("migration_policy", &"<redacted>")
            .finish()
    }
}

impl StorageTopology {
    pub fn validate_for_gateway(self) -> Result<(), StorageTopologyError> {
        if self.migration_policy != MigrationRuntimePolicy::RequestPathForbidden {
            return Err(StorageTopologyError::MigrationMayRunOnRequestPath);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageTopologyError {
    MigrationMayRunOnRequestPath,
}

impl fmt::Display for StorageTopologyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MigrationMayRunOnRequestPath => write!(
                f,
                "migrations must not run from request-serving storage paths"
            ),
        }
    }
}

impl Error for StorageTopologyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoragePlanErrorStatus {
    BadRequest,
    InvalidConfiguration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoragePlanErrorResponsePlan {
    pub status: StoragePlanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_storage_topology_error_response(
    _error: &StorageTopologyError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::InvalidConfiguration,
        code: "storage_topology_invalid",
        message: "storage topology configuration is invalid",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TenantStorageKey {
    pub tenant_id: TenantId,
    pub virtual_key_id: Option<VirtualKeyId>,
}

impl TenantStorageKey {
    pub fn tenant(tenant_id: TenantId) -> Self {
        Self {
            tenant_id,
            virtual_key_id: None,
        }
    }

    pub fn virtual_key(tenant_id: TenantId, virtual_key_id: VirtualKeyId) -> Self {
        Self {
            tenant_id,
            virtual_key_id: Some(virtual_key_id),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AtomicReservationCommand {
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub snapshot: BudgetSnapshot,
    pub limit: BudgetLimit,
    pub request: ReservationRequest,
    pub created_at_unix_ms: u64,
    pub ttl_ms: u64,
}

impl fmt::Debug for AtomicReservationCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AtomicReservationCommand")
            .field("storage_key", &"<redacted>")
            .field("idempotency_key", &"<redacted>")
            .field("snapshot", &"<redacted>")
            .field("limit", &"<redacted>")
            .field("request", &"<redacted>")
            .field("created_at_unix_ms", &"<redacted>")
            .field("ttl_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AtomicReservationPlan {
    pub storage_key: TenantStorageKey,
    pub idempotency_key: IdempotencyKey,
    pub reservation_record: ReservationRecord,
    pub ledger_event: LedgerEvent,
}

impl fmt::Debug for AtomicReservationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AtomicReservationPlan")
            .field("storage_key", &"<redacted>")
            .field("idempotency_key", &"<redacted>")
            .field("reservation_record", &"<redacted>")
            .field("ledger_event", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AtomicReservationPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
    ExpiryOverflow,
}

impl fmt::Debug for AtomicReservationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
            Self::ExpiryOverflow => f.write_str("ExpiryOverflow"),
        }
    }
}

impl fmt::Display for AtomicReservationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } | Self::ExpiryOverflow => {
                write!(f, "atomic reservation request is invalid")
            }
        }
    }
}

impl Error for AtomicReservationPlanError {}

pub fn plan_atomic_reservation_error_response(
    _error: &AtomicReservationPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "atomic_reservation_rejected",
        message: "atomic reservation request is invalid",
    }
}

pub fn plan_atomic_reservation(
    command: AtomicReservationCommand,
) -> Result<AtomicReservationPlan, AtomicReservationPlanError> {
    if command.storage_key.tenant_id != command.request.tenant_id {
        return Err(AtomicReservationPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.request.tenant_id,
        });
    }
    let record = ReservationRecord::from_request(
        command.request,
        command.created_at_unix_ms,
        command.ttl_ms,
    )
    .map_err(|_| AtomicReservationPlanError::ExpiryOverflow)?;
    Ok(AtomicReservationPlan {
        storage_key: command.storage_key,
        idempotency_key: command.idempotency_key,
        reservation_record: record,
        ledger_event: LedgerEvent {
            tenant_id: command.request.tenant_id,
            call_id: command.request.call_id,
            reservation_id: command.request.reservation_id,
            kind: prodex_domain::LedgerEventKind::Reserved,
            amount: command.request.estimate,
        },
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct UsageReconciliationCommand {
    pub storage_key: TenantStorageKey,
    pub snapshot: BudgetSnapshot,
    pub record: ReservationRecord,
    pub actual: UsageAmount,
    pub reason: ReservationReconciliationReason,
}

impl fmt::Debug for UsageReconciliationCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsageReconciliationCommand")
            .field("storage_key", &"<redacted>")
            .field("snapshot", &"<redacted>")
            .field("record", &"<redacted>")
            .field("actual", &"<redacted>")
            .field("reason", &self.reason)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct UsageReconciliationPlan {
    pub storage_key: TenantStorageKey,
    pub updated_snapshot: BudgetSnapshot,
    pub reconciliation: ReservationReconciliation,
    pub ledger_events: Vec<LedgerEvent>,
}

impl fmt::Debug for UsageReconciliationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsageReconciliationPlan")
            .field("storage_key", &"<redacted>")
            .field("updated_snapshot", &"<redacted>")
            .field("reconciliation", &"<redacted>")
            .field("ledger_events", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum UsageReconciliationPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        record_tenant: TenantId,
    },
    ActualExceedsReserved {
        reserved: UsageAmount,
        actual: UsageAmount,
    },
    CommittedUsageOverflow {
        committed: UsageAmount,
        actual: UsageAmount,
    },
    ReservedBalanceUnderflow {
        reserved: UsageAmount,
        available: UsageAmount,
    },
}

impl fmt::Debug for UsageReconciliationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("record_tenant", &"<redacted>")
                .finish(),
            Self::ActualExceedsReserved { .. } => f
                .debug_struct("ActualExceedsReserved")
                .field("reserved", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::CommittedUsageOverflow { .. } => f
                .debug_struct("CommittedUsageOverflow")
                .field("committed", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::ReservedBalanceUnderflow { .. } => f
                .debug_struct("ReservedBalanceUnderflow")
                .field("reserved", &"<redacted>")
                .field("available", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for UsageReconciliationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. }
            | Self::ActualExceedsReserved { .. }
            | Self::CommittedUsageOverflow { .. }
            | Self::ReservedBalanceUnderflow { .. } => {
                write!(f, "usage reconciliation request is invalid")
            }
        }
    }
}

impl Error for UsageReconciliationPlanError {}

pub fn plan_usage_reconciliation_error_response(
    _error: &UsageReconciliationPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "usage_reconciliation_rejected",
        message: "usage reconciliation request is invalid",
    }
}

pub fn plan_usage_reconciliation(
    command: UsageReconciliationCommand,
) -> Result<UsageReconciliationPlan, UsageReconciliationPlanError> {
    if command.storage_key.tenant_id != command.record.tenant_id {
        return Err(UsageReconciliationPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            record_tenant: command.record.tenant_id,
        });
    }
    let (updated_snapshot, reconciliation) = reconcile_reserved_usage(
        command.snapshot,
        command.record,
        command.actual,
        command.reason,
    )
    .map_err(|error| match error {
        prodex_domain::ReservationReconciliationError::ActualExceedsReserved {
            reserved,
            actual,
        } => UsageReconciliationPlanError::ActualExceedsReserved { reserved, actual },
        prodex_domain::ReservationReconciliationError::CommittedUsageOverflow {
            committed,
            actual,
        } => UsageReconciliationPlanError::CommittedUsageOverflow { committed, actual },
        prodex_domain::ReservationReconciliationError::ReservedBalanceUnderflow {
            reserved,
            available,
        } => UsageReconciliationPlanError::ReservedBalanceUnderflow {
            reserved,
            available,
        },
    })?;
    let mut ledger_events = vec![reconciliation.committed_event];
    if let Some(released_event) = reconciliation.released_event {
        ledger_events.push(released_event);
    }
    Ok(UsageReconciliationPlan {
        storage_key: command.storage_key,
        updated_snapshot,
        reconciliation,
        ledger_events,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BillingLedgerSortOrder {
    OccurredAtAsc,
    OccurredAtDesc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BillingLedgerPageLimit {
    value: u16,
}

impl BillingLedgerPageLimit {
    pub const DEFAULT: u16 = 100;
    pub const MAX: u16 = 1_000;

    pub fn new(value: Option<u16>) -> Result<Self, BillingLedgerPageLimitError> {
        let value = value.unwrap_or(Self::DEFAULT);
        if value == 0 {
            return Err(BillingLedgerPageLimitError::Zero);
        }
        if value > Self::MAX {
            return Err(BillingLedgerPageLimitError::TooLarge { value });
        }
        Ok(Self { value })
    }

    pub fn get(self) -> u16 {
        self.value
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum BillingLedgerPageLimitError {
    Zero,
    TooLarge { value: u16 },
}

impl fmt::Debug for BillingLedgerPageLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => f.write_str("Zero"),
            Self::TooLarge { .. } => f
                .debug_struct("TooLarge")
                .field("value", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for BillingLedgerPageLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero | Self::TooLarge { .. } => {
                write!(f, "billing ledger page limit is invalid")
            }
        }
    }
}

impl Error for BillingLedgerPageLimitError {}

#[derive(Clone, PartialEq, Eq)]
pub struct BillingLedgerQueryCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub start_unix_ms: Option<u64>,
    pub end_unix_ms: Option<u64>,
    pub page_limit: BillingLedgerPageLimit,
    pub sort_order: BillingLedgerSortOrder,
}

impl fmt::Debug for BillingLedgerQueryCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BillingLedgerQueryCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("start_unix_ms", &"<redacted>")
            .field("end_unix_ms", &"<redacted>")
            .field("page_limit", &"<redacted>")
            .field("sort_order", &self.sort_order)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct BillingLedgerQueryPlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub start_unix_ms: Option<u64>,
    pub end_unix_ms: Option<u64>,
    pub page_limit: u16,
    pub sort_order: BillingLedgerSortOrder,
}

impl fmt::Debug for BillingLedgerQueryPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BillingLedgerQueryPlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("start_unix_ms", &"<redacted>")
            .field("end_unix_ms", &"<redacted>")
            .field("page_limit", &"<redacted>")
            .field("sort_order", &self.sort_order)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum BillingLedgerQueryPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        query_tenant: TenantId,
    },
    StartAfterEnd {
        start_unix_ms: u64,
        end_unix_ms: u64,
    },
}

impl fmt::Debug for BillingLedgerQueryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("query_tenant", &"<redacted>")
                .finish(),
            Self::StartAfterEnd { .. } => f
                .debug_struct("StartAfterEnd")
                .field("start_unix_ms", &"<redacted>")
                .field("end_unix_ms", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for BillingLedgerQueryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } | Self::StartAfterEnd { .. } => {
                write!(f, "billing ledger query request is invalid")
            }
        }
    }
}

impl Error for BillingLedgerQueryPlanError {}

pub fn plan_billing_ledger_query_error_response(
    _error: &BillingLedgerQueryPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "billing_ledger_query_rejected",
        message: "billing ledger query request is invalid",
    }
}

pub fn plan_billing_ledger_query(
    command: BillingLedgerQueryCommand,
) -> Result<BillingLedgerQueryPlan, BillingLedgerQueryPlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(BillingLedgerQueryPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            query_tenant: command.tenant_id,
        });
    }
    if let (Some(start_unix_ms), Some(end_unix_ms)) = (command.start_unix_ms, command.end_unix_ms)
        && start_unix_ms > end_unix_ms
    {
        return Err(BillingLedgerQueryPlanError::StartAfterEnd {
            start_unix_ms,
            end_unix_ms,
        });
    }
    Ok(BillingLedgerQueryPlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        start_unix_ms: command.start_unix_ms,
        end_unix_ms: command.end_unix_ms,
        page_limit: command.page_limit.get(),
        sort_order: command.sort_order,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ExpiredReservationRecoveryCommand {
    pub storage_key: TenantStorageKey,
    pub snapshot: BudgetSnapshot,
    pub record: ReservationRecord,
    pub now_unix_ms: u64,
}

impl fmt::Debug for ExpiredReservationRecoveryCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpiredReservationRecoveryCommand")
            .field("storage_key", &"<redacted>")
            .field("snapshot", &"<redacted>")
            .field("record", &"<redacted>")
            .field("now_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ExpiredReservationRecoveryPlan {
    pub storage_key: TenantStorageKey,
    pub updated_snapshot: BudgetSnapshot,
    pub ledger_event: LedgerEvent,
}

impl fmt::Debug for ExpiredReservationRecoveryPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpiredReservationRecoveryPlan")
            .field("storage_key", &"<redacted>")
            .field("updated_snapshot", &"<redacted>")
            .field("ledger_event", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ExpiredReservationRecoveryPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        record_tenant: TenantId,
    },
    NotExpired,
    InvalidRecord,
}

impl fmt::Debug for ExpiredReservationRecoveryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("record_tenant", &"<redacted>")
                .finish(),
            Self::NotExpired => f.write_str("NotExpired"),
            Self::InvalidRecord => f.write_str("InvalidRecord"),
        }
    }
}

impl fmt::Display for ExpiredReservationRecoveryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } | Self::NotExpired | Self::InvalidRecord => {
                write!(f, "expired reservation recovery request is invalid")
            }
        }
    }
}

impl Error for ExpiredReservationRecoveryPlanError {}

pub fn plan_expired_reservation_recovery_error_response(
    _error: &ExpiredReservationRecoveryPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "expired_reservation_recovery_rejected",
        message: "expired reservation recovery request is invalid",
    }
}

pub fn plan_expired_reservation_recovery(
    command: ExpiredReservationRecoveryCommand,
) -> Result<ExpiredReservationRecoveryPlan, ExpiredReservationRecoveryPlanError> {
    if command.storage_key.tenant_id != command.record.tenant_id {
        return Err(ExpiredReservationRecoveryPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            record_tenant: command.record.tenant_id,
        });
    }
    let (updated_snapshot, ledger_event) = release_expired_reservation(
        command.snapshot,
        command.storage_key.tenant_id,
        command.record,
        command.now_unix_ms,
    )
    .map_err(|error| match error {
        ReservationRecoveryError::NotExpired => ExpiredReservationRecoveryPlanError::NotExpired,
        ReservationRecoveryError::Tenant { expected, actual } => {
            ExpiredReservationRecoveryPlanError::TenantMismatch {
                key_tenant: actual,
                record_tenant: expected,
            }
        }
        ReservationRecoveryError::ExpiryOverflow => ExpiredReservationRecoveryPlanError::NotExpired,
        ReservationRecoveryError::ZeroTtl
        | ReservationRecoveryError::ZeroReserved
        | ReservationRecoveryError::ReservedBalanceUnderflow { .. } => {
            ExpiredReservationRecoveryPlanError::InvalidRecord
        }
    })?;
    Ok(ExpiredReservationRecoveryPlan {
        storage_key: command.storage_key,
        updated_snapshot,
        ledger_event,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditAppendMode {
    HashChain,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AppendOnlyAuditCommand {
    pub storage_key: TenantStorageKey,
    pub event: AuditEvent,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for AppendOnlyAuditCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppendOnlyAuditCommand")
            .field("storage_key", &"<redacted>")
            .field("event", &"<redacted>")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AppendOnlyAuditPlan {
    pub storage_key: TenantStorageKey,
    pub mode: AuditAppendMode,
    pub envelope: AuditEnvelope,
}

impl fmt::Debug for AppendOnlyAuditPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppendOnlyAuditPlan")
            .field("storage_key", &"<redacted>")
            .field("mode", &self.mode)
            .field("envelope", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AppendOnlyAuditPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        event_tenant: TenantId,
    },
}

impl fmt::Debug for AppendOnlyAuditPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("event_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AppendOnlyAuditPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "append-only audit request is invalid"),
        }
    }
}

impl Error for AppendOnlyAuditPlanError {}

pub fn plan_append_only_audit_error_response(
    _error: &AppendOnlyAuditPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "append_only_audit_rejected",
        message: "append-only audit request is invalid",
    }
}

pub fn plan_append_only_audit(
    command: AppendOnlyAuditCommand,
) -> Result<AppendOnlyAuditPlan, AppendOnlyAuditPlanError> {
    if command.storage_key.tenant_id != command.event.tenant_id {
        return Err(AppendOnlyAuditPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            event_tenant: command.event.tenant_id,
        });
    }
    Ok(AppendOnlyAuditPlan {
        storage_key: command.storage_key,
        mode: AuditAppendMode::HashChain,
        envelope: AuditEnvelope::new(command.event, command.previous_digest, command.event_digest),
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditRetentionPurgeCommand {
    pub storage_key: TenantStorageKey,
    pub batch: AuditRetentionPurgeBatch,
}

impl fmt::Debug for AuditRetentionPurgeCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditRetentionPurgeCommand")
            .field("storage_key", &"<redacted>")
            .field("batch", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditRetentionPurgePlan {
    pub storage_key: TenantStorageKey,
    pub batch: AuditRetentionPurgeBatch,
}

impl fmt::Debug for AuditRetentionPurgePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditRetentionPurgePlan")
            .field("storage_key", &"<redacted>")
            .field("batch", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditRetentionPurgePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        batch_tenant: TenantId,
    },
}

impl fmt::Debug for AuditRetentionPurgePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("batch_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AuditRetentionPurgePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "audit retention purge request is invalid"),
        }
    }
}

impl Error for AuditRetentionPurgePlanError {}

pub fn plan_audit_retention_purge_error_response(
    _error: &AuditRetentionPurgePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "audit_retention_purge_rejected",
        message: "audit retention purge request is invalid",
    }
}

pub fn plan_audit_retention_purge(
    command: AuditRetentionPurgeCommand,
) -> Result<AuditRetentionPurgePlan, AuditRetentionPurgePlanError> {
    if command.storage_key.tenant_id != command.batch.tenant_id {
        return Err(AuditRetentionPurgePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            batch_tenant: command.batch.tenant_id,
        });
    }
    Ok(AuditRetentionPurgePlan {
        storage_key: command.storage_key,
        batch: command.batch,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditExportQueryCommand {
    pub storage_key: TenantStorageKey,
    pub export: AuditExportPlan,
}

impl fmt::Debug for AuditExportQueryCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditExportQueryCommand")
            .field("storage_key", &"<redacted>")
            .field("export", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditExportQueryPlan {
    pub storage_key: TenantStorageKey,
    pub export: AuditExportPlan,
    pub tenant_id: TenantId,
    pub sort_order: AuditSortOrder,
    pub page_limit: u16,
}

impl fmt::Debug for AuditExportQueryPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditExportQueryPlan")
            .field("storage_key", &"<redacted>")
            .field("export", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("sort_order", &self.sort_order)
            .field("page_limit", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditExportQueryPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        query_tenant: TenantId,
    },
}

impl fmt::Debug for AuditExportQueryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("query_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AuditExportQueryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "audit export query request is invalid"),
        }
    }
}

impl Error for AuditExportQueryPlanError {}

pub fn plan_audit_export_query_error_response(
    _error: &AuditExportQueryPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "audit_export_query_rejected",
        message: "audit export query request is invalid",
    }
}

pub fn plan_audit_export_query(
    command: AuditExportQueryCommand,
) -> Result<AuditExportQueryPlan, AuditExportQueryPlanError> {
    let query_tenant = command.export.query.scope.tenant_id;
    if command.storage_key.tenant_id != query_tenant {
        return Err(AuditExportQueryPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            query_tenant,
        });
    }
    Ok(AuditExportQueryPlan {
        storage_key: command.storage_key,
        tenant_id: query_tenant,
        sort_order: command.export.query.sort_order,
        page_limit: command.export.query.page_limit.get(),
        export: command.export,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleBindingMutationKind {
    Grant,
    Revoke,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RoleBindingMutationCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub role_binding_id: RoleBindingId,
    pub principal_id: PrincipalId,
    pub role: Role,
    pub kind: RoleBindingMutationKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for RoleBindingMutationCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RoleBindingMutationCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("role_binding_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("role", &self.role)
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RoleBindingMutationPlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub role_binding_id: RoleBindingId,
    pub principal_id: PrincipalId,
    pub role: Role,
    pub kind: RoleBindingMutationKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for RoleBindingMutationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RoleBindingMutationPlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("role_binding_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("role", &self.role)
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum RoleBindingMutationPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
}

impl fmt::Debug for RoleBindingMutationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for RoleBindingMutationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "role-binding mutation request is invalid"),
        }
    }
}

impl Error for RoleBindingMutationPlanError {}

pub fn plan_role_binding_mutation_error_response(
    _error: &RoleBindingMutationPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "role_binding_mutation_rejected",
        message: "role-binding mutation request is invalid",
    }
}

pub fn plan_role_binding_mutation(
    command: RoleBindingMutationCommand,
) -> Result<RoleBindingMutationPlan, RoleBindingMutationPlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(RoleBindingMutationPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    Ok(RoleBindingMutationPlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        role_binding_id: command.role_binding_id,
        principal_id: command.principal_id,
        role: command.role,
        kind: command.kind,
        occurred_at_unix_ms: command.occurred_at_unix_ms,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ServiceIdentityCreateCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub created_at_unix_ms: u64,
}

impl fmt::Debug for ServiceIdentityCreateCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceIdentityCreateCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("created_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ServiceIdentityCreatePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub created_at_unix_ms: u64,
}

impl fmt::Debug for ServiceIdentityCreatePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceIdentityCreatePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("created_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ServiceIdentityCreatePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
}

impl fmt::Debug for ServiceIdentityCreatePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ServiceIdentityCreatePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "service identity create request is invalid"),
        }
    }
}

impl Error for ServiceIdentityCreatePlanError {}

pub fn plan_service_identity_create_error_response(
    _error: &ServiceIdentityCreatePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "service_identity_create_rejected",
        message: "service identity create request is invalid",
    }
}

pub fn plan_service_identity_create(
    command: ServiceIdentityCreateCommand,
) -> Result<ServiceIdentityCreatePlan, ServiceIdentityCreatePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(ServiceIdentityCreatePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    Ok(ServiceIdentityCreatePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        principal_id: command.principal_id,
        display_name: command.display_name,
        created_at_unix_ms: command.created_at_unix_ms,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserLifecycleKind {
    Create,
    Update,
    Delete,
}

#[derive(Clone, PartialEq, Eq)]
pub struct UserLifecycleCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub external_id: String,
    pub display_name: String,
    pub kind: UserLifecycleKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for UserLifecycleCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserLifecycleCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("external_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct UserLifecyclePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub external_id: String,
    pub display_name: String,
    pub kind: UserLifecycleKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for UserLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserLifecyclePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("external_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum UserLifecyclePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
    EmptyExternalId,
    EmptyDisplayName,
}

impl fmt::Debug for UserLifecyclePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
            Self::EmptyExternalId => f.write_str("EmptyExternalId"),
            Self::EmptyDisplayName => f.write_str("EmptyDisplayName"),
        }
    }
}

impl fmt::Display for UserLifecyclePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } | Self::EmptyExternalId | Self::EmptyDisplayName => {
                write!(f, "user lifecycle request is invalid")
            }
        }
    }
}

impl Error for UserLifecyclePlanError {}

pub fn plan_user_lifecycle_error_response(
    _error: &UserLifecyclePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "user_lifecycle_rejected",
        message: "user lifecycle request is invalid",
    }
}

pub fn plan_user_lifecycle(
    command: UserLifecycleCommand,
) -> Result<UserLifecyclePlan, UserLifecyclePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(UserLifecyclePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    if command.external_id.trim().is_empty() {
        return Err(UserLifecyclePlanError::EmptyExternalId);
    }
    if command.kind != UserLifecycleKind::Delete && command.display_name.trim().is_empty() {
        return Err(UserLifecyclePlanError::EmptyDisplayName);
    }
    Ok(UserLifecyclePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        principal_id: command.principal_id,
        external_id: command.external_id,
        display_name: command.display_name,
        kind: command.kind,
        occurred_at_unix_ms: command.occurred_at_unix_ms,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct BudgetPolicyUpdateCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub budget_scope: String,
    pub limit: BudgetLimit,
    pub updated_at_unix_ms: u64,
}

impl fmt::Debug for BudgetPolicyUpdateCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BudgetPolicyUpdateCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("budget_scope", &"<redacted>")
            .field("limit", &"<redacted>")
            .field("updated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct BudgetPolicyUpdatePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub budget_scope: String,
    pub limit: BudgetLimit,
    pub updated_at_unix_ms: u64,
}

impl fmt::Debug for BudgetPolicyUpdatePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BudgetPolicyUpdatePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("budget_scope", &"<redacted>")
            .field("limit", &"<redacted>")
            .field("updated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum BudgetPolicyUpdatePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
    EmptyBudgetScope,
}

impl fmt::Debug for BudgetPolicyUpdatePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
            Self::EmptyBudgetScope => f.write_str("EmptyBudgetScope"),
        }
    }
}

impl fmt::Display for BudgetPolicyUpdatePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } | Self::EmptyBudgetScope => {
                write!(f, "budget policy update request is invalid")
            }
        }
    }
}

impl Error for BudgetPolicyUpdatePlanError {}

pub fn plan_budget_policy_update_error_response(
    _error: &BudgetPolicyUpdatePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "budget_policy_update_rejected",
        message: "budget policy update request is invalid",
    }
}

pub fn plan_budget_policy_update(
    command: BudgetPolicyUpdateCommand,
) -> Result<BudgetPolicyUpdatePlan, BudgetPolicyUpdatePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(BudgetPolicyUpdatePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    if command.budget_scope.trim().is_empty() {
        return Err(BudgetPolicyUpdatePlanError::EmptyBudgetScope);
    }
    Ok(BudgetPolicyUpdatePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        budget_scope: command.budget_scope,
        limit: command.limit,
        updated_at_unix_ms: command.updated_at_unix_ms,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TenantLifecycleKind {
    Create,
    Update,
}

#[derive(Clone, PartialEq, Eq)]
pub struct TenantLifecycleCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub display_name: String,
    pub kind: TenantLifecycleKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for TenantLifecycleCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantLifecycleCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TenantLifecyclePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub display_name: String,
    pub kind: TenantLifecycleKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for TenantLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantLifecyclePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum TenantLifecyclePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
    EmptyDisplayName,
}

impl fmt::Debug for TenantLifecyclePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
            Self::EmptyDisplayName => f.write_str("EmptyDisplayName"),
        }
    }
}

impl fmt::Display for TenantLifecyclePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } | Self::EmptyDisplayName => {
                write!(f, "tenant lifecycle request is invalid")
            }
        }
    }
}

impl Error for TenantLifecyclePlanError {}

pub fn plan_tenant_lifecycle_error_response(
    _error: &TenantLifecyclePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "tenant_lifecycle_rejected",
        message: "tenant lifecycle request is invalid",
    }
}

pub fn plan_tenant_lifecycle(
    command: TenantLifecycleCommand,
) -> Result<TenantLifecyclePlan, TenantLifecyclePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(TenantLifecyclePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    if command.display_name.trim().is_empty() {
        return Err(TenantLifecyclePlanError::EmptyDisplayName);
    }
    Ok(TenantLifecyclePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        display_name: command.display_name,
        kind: command.kind,
        occurred_at_unix_ms: command.occurred_at_unix_ms,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualKeySecretReferenceKind {
    Create,
    Rotate,
}

#[derive(Clone, PartialEq, Eq)]
pub struct VirtualKeySecretReferenceCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub virtual_key_id: VirtualKeyId,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub secret_ref: SecretRef,
    pub kind: VirtualKeySecretReferenceKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for VirtualKeySecretReferenceCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualKeySecretReferenceCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("virtual_key_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("secret_ref", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct VirtualKeySecretReferencePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub virtual_key_id: VirtualKeyId,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub secret_ref: SecretRef,
    pub kind: VirtualKeySecretReferenceKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for VirtualKeySecretReferencePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualKeySecretReferencePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("virtual_key_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("secret_ref", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum VirtualKeySecretReferencePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
    VirtualKeyMismatch {
        key_virtual_key_id: Option<VirtualKeyId>,
        request_virtual_key_id: VirtualKeyId,
    },
}

impl fmt::Debug for VirtualKeySecretReferencePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
            Self::VirtualKeyMismatch { .. } => f
                .debug_struct("VirtualKeyMismatch")
                .field("key_virtual_key_id", &"<redacted>")
                .field("request_virtual_key_id", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for VirtualKeySecretReferencePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => {
                write!(f, "virtual-key secret reference request is invalid")
            }
            Self::VirtualKeyMismatch { .. } => {
                write!(f, "virtual-key secret reference request is invalid")
            }
        }
    }
}

impl Error for VirtualKeySecretReferencePlanError {}

pub fn plan_virtual_key_secret_reference_error_response(
    _error: &VirtualKeySecretReferencePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "virtual_key_secret_reference_rejected",
        message: "virtual-key secret reference request is invalid",
    }
}

pub fn plan_virtual_key_secret_reference(
    command: VirtualKeySecretReferenceCommand,
) -> Result<VirtualKeySecretReferencePlan, VirtualKeySecretReferencePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(VirtualKeySecretReferencePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    if command.storage_key.virtual_key_id != Some(command.virtual_key_id) {
        return Err(VirtualKeySecretReferencePlanError::VirtualKeyMismatch {
            key_virtual_key_id: command.storage_key.virtual_key_id,
            request_virtual_key_id: command.virtual_key_id,
        });
    }
    Ok(VirtualKeySecretReferencePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        virtual_key_id: command.virtual_key_id,
        principal_id: command.principal_id,
        display_name: command.display_name,
        secret_ref: command.secret_ref,
        kind: command.kind,
        occurred_at_unix_ms: command.occurred_at_unix_ms,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderCredentialReferenceCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub provider_credential_id: ProviderCredentialId,
    pub provider_name: String,
    pub secret_ref: SecretRef,
    pub rotated_at_unix_ms: u64,
}

impl fmt::Debug for ProviderCredentialReferenceCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderCredentialReferenceCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("provider_credential_id", &"<redacted>")
            .field("provider_name", &"<redacted>")
            .field("secret_ref", &"<redacted>")
            .field("rotated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderCredentialReferencePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub provider_credential_id: ProviderCredentialId,
    pub provider_name: String,
    pub secret_ref: SecretRef,
    pub rotated_at_unix_ms: u64,
}

impl fmt::Debug for ProviderCredentialReferencePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderCredentialReferencePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("provider_credential_id", &"<redacted>")
            .field("provider_name", &"<redacted>")
            .field("secret_ref", &"<redacted>")
            .field("rotated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ProviderCredentialReferencePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
}

impl fmt::Debug for ProviderCredentialReferencePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ProviderCredentialReferencePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => {
                write!(f, "provider credential reference request is invalid")
            }
        }
    }
}

impl Error for ProviderCredentialReferencePlanError {}

pub fn plan_provider_credential_reference_error_response(
    _error: &ProviderCredentialReferencePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "provider_credential_reference_rejected",
        message: "provider credential reference request is invalid",
    }
}

pub fn plan_provider_credential_reference(
    command: ProviderCredentialReferenceCommand,
) -> Result<ProviderCredentialReferencePlan, ProviderCredentialReferencePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(ProviderCredentialReferencePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    Ok(ProviderCredentialReferencePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        provider_credential_id: command.provider_credential_id,
        provider_name: command.provider_name,
        secret_ref: command.secret_ref,
        rotated_at_unix_ms: command.rotated_at_unix_ms,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyPendingRecordCommand {
    pub storage_key: TenantStorageKey,
    pub operation: IdempotentOperation,
    pub started_at_unix_ms: u64,
}

impl fmt::Debug for IdempotencyPendingRecordCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyPendingRecordCommand")
            .field("storage_key", &"<redacted>")
            .field("operation", &"<redacted>")
            .field("started_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyPendingRecordPlan {
    pub storage_key: TenantStorageKey,
    pub entry: IdempotencyEntry<()>,
}

impl fmt::Debug for IdempotencyPendingRecordPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyPendingRecordPlan")
            .field("storage_key", &"<redacted>")
            .field("entry", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyPendingRecordPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        operation_tenant: TenantId,
    },
}

impl fmt::Debug for IdempotencyPendingRecordPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("operation_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for IdempotencyPendingRecordPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => {
                write!(f, "idempotency pending record request is invalid")
            }
        }
    }
}

impl Error for IdempotencyPendingRecordPlanError {}

pub fn plan_idempotency_pending_record_error_response(
    _error: &IdempotencyPendingRecordPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "idempotency_pending_record_rejected",
        message: "idempotency pending record request is invalid",
    }
}

pub fn plan_idempotency_pending_record(
    command: IdempotencyPendingRecordCommand,
) -> Result<IdempotencyPendingRecordPlan, IdempotencyPendingRecordPlanError> {
    if command.storage_key.tenant_id != command.operation.tenant_id {
        return Err(IdempotencyPendingRecordPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            operation_tenant: command.operation.tenant_id,
        });
    }
    Ok(IdempotencyPendingRecordPlan {
        storage_key: command.storage_key,
        entry: IdempotencyEntry::pending(command.operation, command.started_at_unix_ms),
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyCompletedRecordCommand {
    pub storage_key: TenantStorageKey,
    pub operation: IdempotentOperation,
    pub completed_at_unix_ms: u64,
    pub response_body: Vec<u8>,
}

impl fmt::Debug for IdempotencyCompletedRecordCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyCompletedRecordCommand")
            .field("storage_key", &"<redacted>")
            .field("operation", &"<redacted>")
            .field("completed_at_unix_ms", &"<redacted>")
            .field("response_body", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyCompletedRecordPlan {
    pub storage_key: TenantStorageKey,
    pub completed_at_unix_ms: u64,
    pub entry: IdempotencyEntry<Vec<u8>>,
}

impl fmt::Debug for IdempotencyCompletedRecordPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyCompletedRecordPlan")
            .field("storage_key", &"<redacted>")
            .field("completed_at_unix_ms", &"<redacted>")
            .field("entry", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyCompletedRecordPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        operation_tenant: TenantId,
    },
}

impl fmt::Debug for IdempotencyCompletedRecordPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("operation_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for IdempotencyCompletedRecordPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => {
                write!(f, "idempotency completed record request is invalid")
            }
        }
    }
}

impl Error for IdempotencyCompletedRecordPlanError {}

pub fn plan_idempotency_completed_record_error_response(
    _error: &IdempotencyCompletedRecordPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "idempotency_completed_record_rejected",
        message: "idempotency completed record request is invalid",
    }
}

pub fn plan_idempotency_completed_record(
    command: IdempotencyCompletedRecordCommand,
) -> Result<IdempotencyCompletedRecordPlan, IdempotencyCompletedRecordPlanError> {
    if command.storage_key.tenant_id != command.operation.tenant_id {
        return Err(IdempotencyCompletedRecordPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            operation_tenant: command.operation.tenant_id,
        });
    }
    Ok(IdempotencyCompletedRecordPlan {
        storage_key: command.storage_key,
        completed_at_unix_ms: command.completed_at_unix_ms,
        entry: IdempotencyEntry::completed(IdempotencyRecord {
            operation: command.operation,
            response: command.response_body,
        }),
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyRecordLookupCommand {
    pub storage_key: TenantStorageKey,
    pub operation: IdempotentOperation,
}

impl fmt::Debug for IdempotencyRecordLookupCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyRecordLookupCommand")
            .field("storage_key", &"<redacted>")
            .field("operation", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyRecordLookupPlan {
    pub storage_key: TenantStorageKey,
    pub operation: IdempotentOperation,
}

impl fmt::Debug for IdempotencyRecordLookupPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyRecordLookupPlan")
            .field("storage_key", &"<redacted>")
            .field("operation", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyRecordLookupPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        operation_tenant: TenantId,
    },
}

impl fmt::Debug for IdempotencyRecordLookupPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("operation_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for IdempotencyRecordLookupPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => {
                write!(f, "idempotency record lookup request is invalid")
            }
        }
    }
}

impl Error for IdempotencyRecordLookupPlanError {}

pub fn plan_idempotency_record_lookup_error_response(
    _error: &IdempotencyRecordLookupPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "idempotency_record_lookup_rejected",
        message: "idempotency record lookup request is invalid",
    }
}

pub fn plan_idempotency_record_lookup(
    command: IdempotencyRecordLookupCommand,
) -> Result<IdempotencyRecordLookupPlan, IdempotencyRecordLookupPlanError> {
    if command.storage_key.tenant_id != command.operation.tenant_id {
        return Err(IdempotencyRecordLookupPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            operation_tenant: command.operation.tenant_id,
        });
    }
    Ok(IdempotencyRecordLookupPlan {
        storage_key: command.storage_key,
        operation: command.operation,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdempotencyRecordLookupRowStatus {
    Pending,
    Completed,
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdempotencyRecordLookupRow {
    pub tenant_id: TenantId,
    pub idempotency_key: IdempotencyKey,
    pub request_fingerprint: String,
    pub status: IdempotencyRecordLookupRowStatus,
    pub started_at_unix_ms: u64,
    pub completed_at_unix_ms: Option<u64>,
    pub response_body: Option<Vec<u8>>,
}

impl fmt::Debug for IdempotencyRecordLookupRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdempotencyRecordLookupRow")
            .field("tenant_id", &"<redacted>")
            .field("idempotency_key", &"<redacted>")
            .field("request_fingerprint", &"<redacted>")
            .field("status", &self.status)
            .field("started_at_unix_ms", &"<redacted>")
            .field("completed_at_unix_ms", &"<redacted>")
            .field("response_body", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum IdempotencyRecordLookupRowError {
    TenantMismatch {
        operation_tenant: TenantId,
        row_tenant: TenantId,
    },
    KeyMismatch,
    InvalidRequestFingerprint,
    CompletedResponseMissing,
    CompletedTimestampMissing,
}

impl fmt::Debug for IdempotencyRecordLookupRowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("operation_tenant", &"<redacted>")
                .field("row_tenant", &"<redacted>")
                .finish(),
            Self::KeyMismatch => f.write_str("KeyMismatch"),
            Self::InvalidRequestFingerprint => f.write_str("InvalidRequestFingerprint"),
            Self::CompletedResponseMissing => f.write_str("CompletedResponseMissing"),
            Self::CompletedTimestampMissing => f.write_str("CompletedTimestampMissing"),
        }
    }
}

impl fmt::Display for IdempotencyRecordLookupRowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. }
            | Self::KeyMismatch
            | Self::InvalidRequestFingerprint
            | Self::CompletedResponseMissing
            | Self::CompletedTimestampMissing => {
                write!(f, "idempotency record lookup row is invalid")
            }
        }
    }
}

impl Error for IdempotencyRecordLookupRowError {}

pub fn materialize_idempotency_record_lookup_row_error_response(
    _error: &IdempotencyRecordLookupRowError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "idempotency_record_lookup_row_invalid",
        message: "idempotency record lookup row is invalid",
    }
}

pub fn materialize_idempotency_record_lookup_row(
    operation: &IdempotentOperation,
    row: IdempotencyRecordLookupRow,
) -> Result<IdempotencyEntry<Vec<u8>>, IdempotencyRecordLookupRowError> {
    if row.tenant_id != operation.tenant_id {
        return Err(IdempotencyRecordLookupRowError::TenantMismatch {
            operation_tenant: operation.tenant_id,
            row_tenant: row.tenant_id,
        });
    }
    if row.idempotency_key != operation.key {
        return Err(IdempotencyRecordLookupRowError::KeyMismatch);
    }
    let row_operation =
        IdempotentOperation::new(row.tenant_id, row.idempotency_key, row.request_fingerprint)
            .map_err(|_| IdempotencyRecordLookupRowError::InvalidRequestFingerprint)?;
    match row.status {
        IdempotencyRecordLookupRowStatus::Pending => Ok(IdempotencyEntry::pending(
            row_operation,
            row.started_at_unix_ms,
        )),
        IdempotencyRecordLookupRowStatus::Completed => {
            if row.completed_at_unix_ms.is_none() {
                return Err(IdempotencyRecordLookupRowError::CompletedTimestampMissing);
            }
            let Some(response_body) = row.response_body else {
                return Err(IdempotencyRecordLookupRowError::CompletedResponseMissing);
            };
            Ok(IdempotencyEntry::completed(IdempotencyRecord {
                operation: row_operation,
                response: response_body,
            }))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiReplicaAccountingCheck {
    NoLostUpdate,
    NoDuplicateCharge,
    NoDroppedLedgerEvent,
    NoRequestIdCollision,
    NoLimitOvershoot,
}

#[derive(Clone, PartialEq, Eq)]
pub struct MultiReplicaAccountingConcurrencySpec {
    pub topology: StorageTopology,
    pub gateway_replica_count: u16,
    pub checks: Vec<MultiReplicaAccountingCheck>,
}

impl fmt::Debug for MultiReplicaAccountingConcurrencySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiReplicaAccountingConcurrencySpec")
            .field("topology", &"<redacted>")
            .field("gateway_replica_count", &"<redacted>")
            .field("checks", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MultiReplicaAccountingEvidence {
    pub topology: StorageTopology,
    pub gateway_replica_count: u16,
    pub passed_checks: Vec<MultiReplicaAccountingCheck>,
    pub documented_limit_overshoot_tolerance: bool,
}

impl fmt::Debug for MultiReplicaAccountingEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiReplicaAccountingEvidence")
            .field("topology", &"<redacted>")
            .field("gateway_replica_count", &"<redacted>")
            .field("passed_checks", &"<redacted>")
            .field(
                "documented_limit_overshoot_tolerance",
                &self.documented_limit_overshoot_tolerance,
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MultiReplicaAccountingVerificationPlan {
    pub topology: StorageTopology,
    pub gateway_replica_count: u16,
    pub verified_checks: Vec<MultiReplicaAccountingCheck>,
}

impl fmt::Debug for MultiReplicaAccountingVerificationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiReplicaAccountingVerificationPlan")
            .field("topology", &"<redacted>")
            .field("gateway_replica_count", &"<redacted>")
            .field("verified_checks", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum MultiReplicaAccountingConcurrencySpecError {
    RequiresAtLeastTwoGatewayReplicas {
        actual: u16,
    },
    RequiresPostgresDurableStore,
    RequiresRedisCoordination,
    EvidenceTopologyMismatch {
        expected: StorageTopology,
        actual: StorageTopology,
    },
    EvidenceReplicaCountMismatch {
        expected: u16,
        actual: u16,
    },
    MissingEvidenceCheck {
        check: MultiReplicaAccountingCheck,
    },
    MissingLimitOvershootTolerance,
}

impl fmt::Debug for MultiReplicaAccountingConcurrencySpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiresAtLeastTwoGatewayReplicas { .. } => f
                .debug_struct("RequiresAtLeastTwoGatewayReplicas")
                .field("actual", &"<redacted>")
                .finish(),
            Self::RequiresPostgresDurableStore => f.write_str("RequiresPostgresDurableStore"),
            Self::RequiresRedisCoordination => f.write_str("RequiresRedisCoordination"),
            Self::EvidenceTopologyMismatch { .. } => f
                .debug_struct("EvidenceTopologyMismatch")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::EvidenceReplicaCountMismatch { .. } => f
                .debug_struct("EvidenceReplicaCountMismatch")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::MissingEvidenceCheck { .. } => f
                .debug_struct("MissingEvidenceCheck")
                .field("check", &"<redacted>")
                .finish(),
            Self::MissingLimitOvershootTolerance => f.write_str("MissingLimitOvershootTolerance"),
        }
    }
}

impl fmt::Display for MultiReplicaAccountingConcurrencySpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "multi-replica accounting configuration is invalid")
    }
}

impl Error for MultiReplicaAccountingConcurrencySpecError {}

pub fn plan_multi_replica_accounting_error_response(
    _error: &MultiReplicaAccountingConcurrencySpecError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::InvalidConfiguration,
        code: "multi_replica_accounting_invalid",
        message: "multi-replica accounting configuration is invalid",
    }
}

pub fn plan_multi_replica_accounting_concurrency_spec(
    topology: StorageTopology,
    gateway_replica_count: u16,
) -> Result<MultiReplicaAccountingConcurrencySpec, MultiReplicaAccountingConcurrencySpecError> {
    if gateway_replica_count < 2 {
        return Err(
            MultiReplicaAccountingConcurrencySpecError::RequiresAtLeastTwoGatewayReplicas {
                actual: gateway_replica_count,
            },
        );
    }
    if topology.durable_store != DurableStoreKind::Postgres {
        return Err(MultiReplicaAccountingConcurrencySpecError::RequiresPostgresDurableStore);
    }
    if topology.cache_store != Some(CacheStoreKind::Redis) {
        return Err(MultiReplicaAccountingConcurrencySpecError::RequiresRedisCoordination);
    }
    Ok(MultiReplicaAccountingConcurrencySpec {
        topology,
        gateway_replica_count,
        checks: vec![
            MultiReplicaAccountingCheck::NoLostUpdate,
            MultiReplicaAccountingCheck::NoDuplicateCharge,
            MultiReplicaAccountingCheck::NoDroppedLedgerEvent,
            MultiReplicaAccountingCheck::NoRequestIdCollision,
            MultiReplicaAccountingCheck::NoLimitOvershoot,
        ],
    })
}

pub fn plan_multi_replica_accounting_verification(
    spec: &MultiReplicaAccountingConcurrencySpec,
    evidence: MultiReplicaAccountingEvidence,
) -> Result<MultiReplicaAccountingVerificationPlan, MultiReplicaAccountingConcurrencySpecError> {
    if evidence.topology != spec.topology {
        return Err(
            MultiReplicaAccountingConcurrencySpecError::EvidenceTopologyMismatch {
                expected: spec.topology,
                actual: evidence.topology,
            },
        );
    }
    if evidence.gateway_replica_count != spec.gateway_replica_count {
        return Err(
            MultiReplicaAccountingConcurrencySpecError::EvidenceReplicaCountMismatch {
                expected: spec.gateway_replica_count,
                actual: evidence.gateway_replica_count,
            },
        );
    }
    for check in &spec.checks {
        if !evidence.passed_checks.contains(check) {
            return Err(
                MultiReplicaAccountingConcurrencySpecError::MissingEvidenceCheck { check: *check },
            );
        }
    }
    if spec
        .checks
        .contains(&MultiReplicaAccountingCheck::NoLimitOvershoot)
        && !evidence.documented_limit_overshoot_tolerance
    {
        return Err(MultiReplicaAccountingConcurrencySpecError::MissingLimitOvershootTolerance);
    }

    Ok(MultiReplicaAccountingVerificationPlan {
        topology: evidence.topology,
        gateway_replica_count: evidence.gateway_replica_count,
        verified_checks: spec.checks.clone(),
    })
}

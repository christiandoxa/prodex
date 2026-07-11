use super::*;

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

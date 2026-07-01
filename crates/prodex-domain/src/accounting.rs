use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{CallId, ReservationId, TenantId};

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageAmount {
    pub tokens: u64,
    pub cost_micros: u64,
}

impl fmt::Debug for UsageAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsageAmount")
            .field("tokens", &"<redacted>")
            .field("cost_micros", &"<redacted>")
            .finish()
    }
}

impl UsageAmount {
    pub const ZERO: Self = Self {
        tokens: 0,
        cost_micros: 0,
    };

    pub fn new(tokens: u64, cost_micros: u64) -> Self {
        Self {
            tokens,
            cost_micros,
        }
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            tokens: self.tokens.checked_add(other.tokens)?,
            cost_micros: self.cost_micros.checked_add(other.cost_micros)?,
        })
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self {
            tokens: self.tokens.saturating_sub(other.tokens),
            cost_micros: self.cost_micros.saturating_sub(other.cost_micros),
        }
    }

    pub fn exceeds(self, limit: Self) -> bool {
        self.tokens > limit.tokens || self.cost_micros > limit.cost_micros
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetLimit {
    pub max: UsageAmount,
}

impl fmt::Debug for BudgetLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BudgetLimit")
            .field("max", &"<redacted>")
            .finish()
    }
}

impl BudgetLimit {
    pub fn new(max_tokens: u64, max_cost_micros: u64) -> Self {
        Self {
            max: UsageAmount::new(max_tokens, max_cost_micros),
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    pub reserved: UsageAmount,
    pub committed: UsageAmount,
}

impl fmt::Debug for BudgetSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BudgetSnapshot")
            .field("reserved", &"<redacted>")
            .field("committed", &"<redacted>")
            .finish()
    }
}

impl BudgetSnapshot {
    pub fn total_held(self) -> Option<UsageAmount> {
        self.reserved.checked_add(self.committed)
    }

    pub fn available(self, limit: BudgetLimit) -> UsageAmount {
        match self.total_held() {
            Some(held) => limit.max.saturating_sub(held),
            None => UsageAmount::ZERO,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReservationRequest {
    pub tenant_id: TenantId,
    pub call_id: CallId,
    pub reservation_id: ReservationId,
    pub estimate: UsageAmount,
}

impl fmt::Debug for ReservationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReservationRequest")
            .field("tenant_id", &"<redacted>")
            .field("call_id", &"<redacted>")
            .field("reservation_id", &"<redacted>")
            .field("estimate", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReservationRecord {
    pub tenant_id: TenantId,
    pub call_id: CallId,
    pub reservation_id: ReservationId,
    pub reserved: UsageAmount,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl fmt::Debug for ReservationRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReservationRecord")
            .field("tenant_id", &"<redacted>")
            .field("call_id", &"<redacted>")
            .field("reservation_id", &"<redacted>")
            .field("reserved", &"<redacted>")
            .field("created_at_unix_ms", &"<redacted>")
            .field("expires_at_unix_ms", &"<redacted>")
            .finish()
    }
}

impl ReservationRecord {
    pub fn from_request(
        request: ReservationRequest,
        created_at_unix_ms: u64,
        ttl_ms: u64,
    ) -> Result<Self, ReservationRecoveryError> {
        if ttl_ms == 0 {
            return Err(ReservationRecoveryError::ZeroTtl);
        }
        if request.estimate == UsageAmount::ZERO {
            return Err(ReservationRecoveryError::ZeroReserved);
        }
        let expires_at_unix_ms = created_at_unix_ms
            .checked_add(ttl_ms)
            .ok_or(ReservationRecoveryError::ExpiryOverflow)?;
        Ok(Self {
            tenant_id: request.tenant_id,
            call_id: request.call_id,
            reservation_id: request.reservation_id,
            reserved: request.estimate,
            created_at_unix_ms,
            expires_at_unix_ms,
        })
    }

    pub fn is_expired_at(&self, now_unix_ms: u64) -> bool {
        now_unix_ms >= self.expires_at_unix_ms
    }

    pub fn release_event(&self) -> LedgerEvent {
        LedgerEvent {
            tenant_id: self.tenant_id,
            call_id: self.call_id,
            reservation_id: self.reservation_id,
            kind: LedgerEventKind::Released,
            amount: self.reserved,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReservationCommit {
    pub tenant_id: TenantId,
    pub call_id: CallId,
    pub reservation_id: ReservationId,
    pub reserved: UsageAmount,
    pub actual: UsageAmount,
}

impl fmt::Debug for ReservationCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReservationCommit")
            .field("tenant_id", &"<redacted>")
            .field("call_id", &"<redacted>")
            .field("reservation_id", &"<redacted>")
            .field("reserved", &"<redacted>")
            .field("actual", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReservationReconciliationReason {
    Completed,
    Cancelled,
    StreamInterrupted,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReservationReconciliation {
    pub reason: ReservationReconciliationReason,
    pub commit: ReservationCommit,
    pub committed_event: LedgerEvent,
    pub released_event: Option<LedgerEvent>,
}

impl fmt::Debug for ReservationReconciliation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReservationReconciliation")
            .field("reason", &self.reason)
            .field("commit", &"<redacted>")
            .field("committed_event", &"<redacted>")
            .field(
                "released_event",
                &self.released_event.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetRejectionReason {
    ArithmeticOverflow,
    ZeroEstimate,
    TokenLimitExceeded,
    CostLimitExceeded,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetRejection {
    pub reason: BudgetRejectionReason,
    pub available: UsageAmount,
    pub requested: UsageAmount,
}

impl fmt::Debug for BudgetRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BudgetRejection")
            .field("reason", &self.reason)
            .field("available", &"<redacted>")
            .field("requested", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for BudgetRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "budget reservation rejected")
    }
}

impl Error for BudgetRejection {}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationCommitMismatch {
    Tenant {
        expected: TenantId,
        actual: TenantId,
    },
    Call {
        expected: CallId,
        actual: CallId,
    },
    Reservation {
        expected: ReservationId,
        actual: ReservationId,
    },
    ReservedAmount {
        expected: UsageAmount,
        actual: UsageAmount,
    },
}

impl fmt::Debug for ReservationCommitMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tenant { .. } => f
                .debug_struct("Tenant")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::Call { .. } => f
                .debug_struct("Call")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::Reservation { .. } => f
                .debug_struct("Reservation")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::ReservedAmount { .. } => f
                .debug_struct("ReservedAmount")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationCommitError {
    Mismatch(ReservationCommitMismatch),
    ZeroActual,
    ReservedBalanceUnderflow {
        reserved: UsageAmount,
        available: UsageAmount,
    },
    ActualExceedsReserved {
        reserved: UsageAmount,
        actual: UsageAmount,
    },
    CommittedUsageOverflow {
        committed: UsageAmount,
        actual: UsageAmount,
    },
}

impl fmt::Debug for ReservationCommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch(error) => f.debug_tuple("Mismatch").field(error).finish(),
            Self::ZeroActual => f.write_str("ZeroActual"),
            Self::ReservedBalanceUnderflow { .. } => f
                .debug_struct("ReservedBalanceUnderflow")
                .field("reserved", &"<redacted>")
                .field("available", &"<redacted>")
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
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationRecoveryError {
    ExpiryOverflow,
    ZeroTtl,
    ZeroReserved,
    NotExpired,
    ReservedBalanceUnderflow {
        reserved: UsageAmount,
        available: UsageAmount,
    },
    Tenant {
        expected: TenantId,
        actual: TenantId,
    },
}

impl fmt::Debug for ReservationRecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpiryOverflow => f.write_str("ExpiryOverflow"),
            Self::ZeroTtl => f.write_str("ZeroTtl"),
            Self::ZeroReserved => f.write_str("ZeroReserved"),
            Self::NotExpired => f.write_str("NotExpired"),
            Self::ReservedBalanceUnderflow { .. } => f
                .debug_struct("ReservedBalanceUnderflow")
                .field("reserved", &"<redacted>")
                .field("available", &"<redacted>")
                .finish(),
            Self::Tenant { .. } => f
                .debug_struct("Tenant")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationReconciliationError {
    ReservedBalanceUnderflow {
        reserved: UsageAmount,
        available: UsageAmount,
    },
    ActualExceedsReserved {
        reserved: UsageAmount,
        actual: UsageAmount,
    },
    CommittedUsageOverflow {
        committed: UsageAmount,
        actual: UsageAmount,
    },
}

impl fmt::Debug for ReservationReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedBalanceUnderflow { .. } => f
                .debug_struct("ReservedBalanceUnderflow")
                .field("reserved", &"<redacted>")
                .field("available", &"<redacted>")
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
        }
    }
}

impl fmt::Display for ReservationCommitMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reservation commit rejected")
    }
}

impl Error for ReservationCommitMismatch {}

impl From<ReservationCommitMismatch> for ReservationCommitError {
    fn from(error: ReservationCommitMismatch) -> Self {
        Self::Mismatch(error)
    }
}

impl fmt::Display for ReservationCommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reservation commit rejected")
    }
}

impl Error for ReservationCommitError {}

impl fmt::Display for ReservationReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reservation reconciliation rejected")
    }
}

impl Error for ReservationReconciliationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountingErrorStatus {
    Forbidden,
    Conflict,
    UnprocessableRequest,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountingErrorResponsePlan {
    pub status: AccountingErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Debug for AccountingErrorResponsePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountingErrorResponsePlan")
            .field("status", &self.status)
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

pub fn plan_budget_rejection_response(error: &BudgetRejection) -> AccountingErrorResponsePlan {
    let code = match error.reason {
        BudgetRejectionReason::ArithmeticOverflow => "budget_accounting_overflow",
        BudgetRejectionReason::ZeroEstimate => "budget_estimate_invalid",
        BudgetRejectionReason::TokenLimitExceeded => "budget_token_limit_exceeded",
        BudgetRejectionReason::CostLimitExceeded => "budget_cost_limit_exceeded",
    };

    AccountingErrorResponsePlan {
        status: AccountingErrorStatus::Forbidden,
        code,
        message: "budget reservation rejected",
    }
}

pub fn plan_reservation_commit_mismatch_response(
    error: &ReservationCommitMismatch,
) -> AccountingErrorResponsePlan {
    let code = match error {
        ReservationCommitMismatch::Tenant { .. } => "reservation_tenant_mismatch",
        ReservationCommitMismatch::Call { .. } => "reservation_call_mismatch",
        ReservationCommitMismatch::Reservation { .. } => "reservation_id_mismatch",
        ReservationCommitMismatch::ReservedAmount { .. } => "reservation_reserved_amount_mismatch",
    };

    AccountingErrorResponsePlan {
        status: AccountingErrorStatus::Conflict,
        code,
        message: "reservation commit rejected",
    }
}

pub fn plan_reservation_commit_error_response(
    error: &ReservationCommitError,
) -> AccountingErrorResponsePlan {
    match error {
        ReservationCommitError::Mismatch(mismatch) => {
            plan_reservation_commit_mismatch_response(mismatch)
        }
        ReservationCommitError::ZeroActual => AccountingErrorResponsePlan {
            status: AccountingErrorStatus::UnprocessableRequest,
            code: "reservation_actual_usage_invalid",
            message: "reservation commit rejected",
        },
        ReservationCommitError::ActualExceedsReserved { .. } => AccountingErrorResponsePlan {
            status: AccountingErrorStatus::UnprocessableRequest,
            code: "reservation_actual_usage_exceeds_reserved",
            message: "reservation commit rejected",
        },
        ReservationCommitError::ReservedBalanceUnderflow { .. } => AccountingErrorResponsePlan {
            status: AccountingErrorStatus::Conflict,
            code: "reservation_reserved_balance_underflow",
            message: "reservation commit rejected",
        },
        ReservationCommitError::CommittedUsageOverflow { .. } => AccountingErrorResponsePlan {
            status: AccountingErrorStatus::UnprocessableRequest,
            code: "reservation_committed_usage_overflow",
            message: "reservation commit rejected",
        },
    }
}

pub fn plan_reservation_recovery_error_response(
    error: &ReservationRecoveryError,
) -> AccountingErrorResponsePlan {
    let code = match error {
        ReservationRecoveryError::ExpiryOverflow => "reservation_expiry_invalid",
        ReservationRecoveryError::ZeroTtl => "reservation_ttl_invalid",
        ReservationRecoveryError::ZeroReserved => "reservation_reserved_usage_invalid",
        ReservationRecoveryError::NotExpired => "reservation_not_expired",
        ReservationRecoveryError::ReservedBalanceUnderflow { .. } => {
            "reservation_reserved_balance_underflow"
        }
        ReservationRecoveryError::Tenant { .. } => "reservation_tenant_mismatch",
    };

    AccountingErrorResponsePlan {
        status: AccountingErrorStatus::Conflict,
        code,
        message: "reservation recovery rejected",
    }
}

pub fn plan_reservation_reconciliation_error_response(
    error: &ReservationReconciliationError,
) -> AccountingErrorResponsePlan {
    let code = match error {
        ReservationReconciliationError::ReservedBalanceUnderflow { .. } => {
            "reservation_reserved_balance_underflow"
        }
        ReservationReconciliationError::ActualExceedsReserved { .. } => {
            "reservation_actual_usage_exceeds_reserved"
        }
        ReservationReconciliationError::CommittedUsageOverflow { .. } => {
            "reservation_committed_usage_overflow"
        }
    };

    AccountingErrorResponsePlan {
        status: AccountingErrorStatus::UnprocessableRequest,
        code,
        message: "reservation reconciliation rejected",
    }
}

pub fn reserve_budget(
    snapshot: BudgetSnapshot,
    limit: BudgetLimit,
    request: ReservationRequest,
) -> Result<BudgetSnapshot, BudgetRejection> {
    if request.estimate == UsageAmount::ZERO {
        return Err(BudgetRejection {
            reason: BudgetRejectionReason::ZeroEstimate,
            available: snapshot.available(limit),
            requested: request.estimate,
        });
    }
    let held = snapshot.total_held().ok_or(BudgetRejection {
        reason: BudgetRejectionReason::ArithmeticOverflow,
        available: UsageAmount::ZERO,
        requested: request.estimate,
    })?;
    let next_held = held.checked_add(request.estimate).ok_or(BudgetRejection {
        reason: BudgetRejectionReason::ArithmeticOverflow,
        available: UsageAmount::ZERO,
        requested: request.estimate,
    })?;
    if next_held.tokens > limit.max.tokens {
        return Err(BudgetRejection {
            reason: BudgetRejectionReason::TokenLimitExceeded,
            available: snapshot.available(limit),
            requested: request.estimate,
        });
    }
    if next_held.cost_micros > limit.max.cost_micros {
        return Err(BudgetRejection {
            reason: BudgetRejectionReason::CostLimitExceeded,
            available: snapshot.available(limit),
            requested: request.estimate,
        });
    }

    Ok(BudgetSnapshot {
        reserved: snapshot
            .reserved
            .checked_add(request.estimate)
            .ok_or(BudgetRejection {
                reason: BudgetRejectionReason::ArithmeticOverflow,
                available: UsageAmount::ZERO,
                requested: request.estimate,
            })?,
        committed: snapshot.committed,
    })
}

pub fn validate_reservation_commit(
    request: ReservationRequest,
    commit: ReservationCommit,
) -> Result<(), ReservationCommitMismatch> {
    if request.tenant_id != commit.tenant_id {
        return Err(ReservationCommitMismatch::Tenant {
            expected: request.tenant_id,
            actual: commit.tenant_id,
        });
    }
    if request.call_id != commit.call_id {
        return Err(ReservationCommitMismatch::Call {
            expected: request.call_id,
            actual: commit.call_id,
        });
    }
    if request.reservation_id != commit.reservation_id {
        return Err(ReservationCommitMismatch::Reservation {
            expected: request.reservation_id,
            actual: commit.reservation_id,
        });
    }
    if request.estimate != commit.reserved {
        return Err(ReservationCommitMismatch::ReservedAmount {
            expected: request.estimate,
            actual: commit.reserved,
        });
    }
    Ok(())
}

pub fn commit_reservation_checked(
    snapshot: BudgetSnapshot,
    request: ReservationRequest,
    commit: ReservationCommit,
) -> Result<BudgetSnapshot, ReservationCommitError> {
    validate_reservation_commit(request, commit)?;
    commit_reservation(snapshot, commit)
}

pub fn commit_reservation(
    snapshot: BudgetSnapshot,
    commit: ReservationCommit,
) -> Result<BudgetSnapshot, ReservationCommitError> {
    if commit.actual == UsageAmount::ZERO {
        return Err(ReservationCommitError::ZeroActual);
    }
    if commit.actual.exceeds(commit.reserved) {
        return Err(ReservationCommitError::ActualExceedsReserved {
            reserved: commit.reserved,
            actual: commit.actual,
        });
    }
    if commit.reserved.exceeds(snapshot.reserved) {
        return Err(ReservationCommitError::ReservedBalanceUnderflow {
            reserved: commit.reserved,
            available: snapshot.reserved,
        });
    }
    let committed = snapshot.committed.checked_add(commit.actual).ok_or(
        ReservationCommitError::CommittedUsageOverflow {
            committed: snapshot.committed,
            actual: commit.actual,
        },
    )?;
    Ok(BudgetSnapshot {
        reserved: snapshot.reserved.saturating_sub(commit.reserved),
        committed,
    })
}

pub fn release_expired_reservation(
    snapshot: BudgetSnapshot,
    tenant_id: TenantId,
    record: ReservationRecord,
    now_unix_ms: u64,
) -> Result<(BudgetSnapshot, LedgerEvent), ReservationRecoveryError> {
    if tenant_id != record.tenant_id {
        return Err(ReservationRecoveryError::Tenant {
            expected: record.tenant_id,
            actual: tenant_id,
        });
    }
    if !record.is_expired_at(now_unix_ms) {
        return Err(ReservationRecoveryError::NotExpired);
    }
    if record.reserved.exceeds(snapshot.reserved) {
        return Err(ReservationRecoveryError::ReservedBalanceUnderflow {
            reserved: record.reserved,
            available: snapshot.reserved,
        });
    }
    Ok((
        BudgetSnapshot {
            reserved: snapshot.reserved.saturating_sub(record.reserved),
            committed: snapshot.committed,
        },
        record.release_event(),
    ))
}

pub fn reconcile_reserved_usage(
    snapshot: BudgetSnapshot,
    record: ReservationRecord,
    actual: UsageAmount,
    reason: ReservationReconciliationReason,
) -> Result<(BudgetSnapshot, ReservationReconciliation), ReservationReconciliationError> {
    if actual.exceeds(record.reserved) {
        return Err(ReservationReconciliationError::ActualExceedsReserved {
            reserved: record.reserved,
            actual,
        });
    }
    if record.reserved.exceeds(snapshot.reserved) {
        return Err(ReservationReconciliationError::ReservedBalanceUnderflow {
            reserved: record.reserved,
            available: snapshot.reserved,
        });
    }
    let committed = snapshot.committed.checked_add(actual).ok_or(
        ReservationReconciliationError::CommittedUsageOverflow {
            committed: snapshot.committed,
            actual,
        },
    )?;

    let commit = ReservationCommit {
        tenant_id: record.tenant_id,
        call_id: record.call_id,
        reservation_id: record.reservation_id,
        reserved: record.reserved,
        actual,
    };
    let committed_event = LedgerEvent {
        tenant_id: record.tenant_id,
        call_id: record.call_id,
        reservation_id: record.reservation_id,
        kind: LedgerEventKind::Committed,
        amount: actual,
    };
    let released = record.reserved.saturating_sub(actual);
    let released_event = (released != UsageAmount::ZERO).then_some(LedgerEvent {
        tenant_id: record.tenant_id,
        call_id: record.call_id,
        reservation_id: record.reservation_id,
        kind: LedgerEventKind::Released,
        amount: released,
    });
    Ok((
        BudgetSnapshot {
            reserved: snapshot.reserved.saturating_sub(record.reserved),
            committed,
        },
        ReservationReconciliation {
            reason,
            commit,
            committed_event,
            released_event,
        },
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerEventKind {
    Reserved,
    Committed,
    Released,
    Rejected,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEvent {
    pub tenant_id: TenantId,
    pub call_id: CallId,
    pub reservation_id: ReservationId,
    pub kind: LedgerEventKind,
    pub amount: UsageAmount,
}

impl fmt::Debug for LedgerEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LedgerEvent")
            .field("tenant_id", &"<redacted>")
            .field("call_id", &"<redacted>")
            .field("reservation_id", &"<redacted>")
            .field("kind", &self.kind)
            .field("amount", &"<redacted>")
            .finish()
    }
}

impl LedgerEvent {
    pub fn idempotency_key(self) -> (TenantId, CallId, ReservationId, LedgerEventKind) {
        (self.tenant_id, self.call_id, self.reservation_id, self.kind)
    }
}

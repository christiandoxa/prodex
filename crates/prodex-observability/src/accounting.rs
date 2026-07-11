use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservationRecoveryOperation {
    ScanExpired,
    AcquireLease,
    ReleaseBudget,
    WriteLedger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservationRecoveryResult {
    Recovered,
    Skipped,
    LeaseUnavailable,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReservationRecoveryMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountingOperation {
    Reservation,
    Commit,
    Release,
    Expire,
    Reconciliation,
    BudgetRejection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountingResult {
    Accepted,
    Rejected,
    Committed,
    Released,
    Expired,
    Reconciled,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountingMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BillingLedgerOperation {
    ReserveAppend,
    CommitAppend,
    ReleaseAppend,
    ReconciliationAppend,
    Query,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BillingLedgerResult {
    Written,
    Read,
    Skipped,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BillingLedgerMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetRejectionReason {
    TenantBudgetExceeded,
    VirtualKeyBudgetExceeded,
    RateLimited,
    ReservationUnavailable,
    PolicyDenied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetRejectionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub reason_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateLimitScope {
    Tenant,
    VirtualKey,
    Principal,
    Provider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allowed,
    Delayed,
    Rejected,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimitDecisionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub scope_label: TelemetryAttribute,
    pub decision_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedisCoordinationOperation {
    RateLimitCheck,
    RateLimitCommit,
    RecoveryLeaseAcquire,
    RecoveryLeaseRelease,
    CacheRead,
    CacheWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedisCoordinationResult {
    Success,
    Limited,
    LeaseUnavailable,
    CacheMiss,
    Unavailable,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedisCoordinationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotaCorrectnessEvent {
    ReservationOvershoot,
    DuplicateChargePrevented,
    MissingCommitRecovered,
    MissingReleaseRecovered,
    LedgerMismatchDetected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuotaCorrectnessMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub event_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SloAlertSli {
    Availability,
    LatencyP95,
    ErrorRate,
    QuotaCorrectness,
    ProviderDegradation,
    PersistenceFailure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SloAlertSeverity {
    Warning,
    Critical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SloAlertMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub sli_label: TelemetryAttribute,
    pub severity_label: TelemetryAttribute,
}

pub fn plan_reservation_recovery_metric(
    operation: ReservationRecoveryOperation,
    result: ReservationRecoveryResult,
) -> Result<ReservationRecoveryMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "reservation_recovery_operation",
        reservation_recovery_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "reservation_recovery_result",
        reservation_recovery_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ReservationRecoveryMetricPlan {
        metric_name: "prodex_reservation_recovery_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_accounting_metric(
    operation: AccountingOperation,
    result: AccountingResult,
) -> Result<AccountingMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "accounting_operation",
        accounting_operation_label(operation),
    );
    let result_label =
        TelemetryAttribute::metric_label("accounting_result", accounting_result_label(result));
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AccountingMetricPlan {
        metric_name: "prodex_accounting_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_billing_ledger_metric(
    operation: BillingLedgerOperation,
    result: BillingLedgerResult,
) -> Result<BillingLedgerMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "billing_ledger_operation",
        billing_ledger_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "billing_ledger_result",
        billing_ledger_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BillingLedgerMetricPlan {
        metric_name: "prodex_billing_ledger_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_budget_rejection_metric(
    reason: BudgetRejectionReason,
) -> Result<BudgetRejectionMetricPlan, TelemetryAttributeError> {
    let reason_label = TelemetryAttribute::metric_label(
        "budget_rejection_reason",
        budget_rejection_reason_label(reason),
    );
    reason_label.as_metric_label()?;
    Ok(BudgetRejectionMetricPlan {
        metric_name: "prodex_budget_rejections_total",
        increment: 1,
        reason_label,
    })
}

pub fn plan_rate_limit_decision_metric(
    scope: RateLimitScope,
    decision: RateLimitDecision,
) -> Result<RateLimitDecisionMetricPlan, TelemetryAttributeError> {
    let scope_label =
        TelemetryAttribute::metric_label("rate_limit_scope", rate_limit_scope_label(scope));
    let decision_label = TelemetryAttribute::metric_label(
        "rate_limit_decision",
        rate_limit_decision_label(decision),
    );
    scope_label.as_metric_label()?;
    decision_label.as_metric_label()?;
    Ok(RateLimitDecisionMetricPlan {
        metric_name: "prodex_rate_limit_decisions_total",
        increment: 1,
        scope_label,
        decision_label,
    })
}

pub fn plan_redis_coordination_metric(
    operation: RedisCoordinationOperation,
    result: RedisCoordinationResult,
) -> Result<RedisCoordinationMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "redis_coordination_operation",
        redis_coordination_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "redis_coordination_result",
        redis_coordination_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(RedisCoordinationMetricPlan {
        metric_name: "prodex_redis_coordination_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_quota_correctness_metric(
    event: QuotaCorrectnessEvent,
) -> Result<QuotaCorrectnessMetricPlan, TelemetryAttributeError> {
    let event_label = TelemetryAttribute::metric_label(
        "quota_correctness_event",
        quota_correctness_event_label(event),
    );
    event_label.as_metric_label()?;
    Ok(QuotaCorrectnessMetricPlan {
        metric_name: "prodex_quota_correctness_events_total",
        increment: 1,
        event_label,
    })
}

pub fn plan_slo_alert_metric(
    sli: SloAlertSli,
    severity: SloAlertSeverity,
) -> Result<SloAlertMetricPlan, TelemetryAttributeError> {
    let sli_label = TelemetryAttribute::metric_label("slo_sli", slo_alert_sli_label(sli));
    let severity_label =
        TelemetryAttribute::metric_label("slo_severity", slo_alert_severity_label(severity));
    sli_label.as_metric_label()?;
    severity_label.as_metric_label()?;
    Ok(SloAlertMetricPlan {
        metric_name: "prodex_slo_alert_events_total",
        increment: 1,
        sli_label,
        severity_label,
    })
}

fn reservation_recovery_operation_label(operation: ReservationRecoveryOperation) -> &'static str {
    match operation {
        ReservationRecoveryOperation::ScanExpired => "scan_expired",
        ReservationRecoveryOperation::AcquireLease => "acquire_lease",
        ReservationRecoveryOperation::ReleaseBudget => "release_budget",
        ReservationRecoveryOperation::WriteLedger => "write_ledger",
    }
}

fn reservation_recovery_result_label(result: ReservationRecoveryResult) -> &'static str {
    match result {
        ReservationRecoveryResult::Recovered => "recovered",
        ReservationRecoveryResult::Skipped => "skipped",
        ReservationRecoveryResult::LeaseUnavailable => "lease_unavailable",
        ReservationRecoveryResult::Failed => "failed",
    }
}

fn accounting_operation_label(operation: AccountingOperation) -> &'static str {
    match operation {
        AccountingOperation::Reservation => "reservation",
        AccountingOperation::Commit => "commit",
        AccountingOperation::Release => "release",
        AccountingOperation::Expire => "expire",
        AccountingOperation::Reconciliation => "reconciliation",
        AccountingOperation::BudgetRejection => "budget_rejection",
    }
}

fn accounting_result_label(result: AccountingResult) -> &'static str {
    match result {
        AccountingResult::Accepted => "accepted",
        AccountingResult::Rejected => "rejected",
        AccountingResult::Committed => "committed",
        AccountingResult::Released => "released",
        AccountingResult::Expired => "expired",
        AccountingResult::Reconciled => "reconciled",
        AccountingResult::Failed => "failed",
    }
}

fn billing_ledger_operation_label(operation: BillingLedgerOperation) -> &'static str {
    match operation {
        BillingLedgerOperation::ReserveAppend => "reserve_append",
        BillingLedgerOperation::CommitAppend => "commit_append",
        BillingLedgerOperation::ReleaseAppend => "release_append",
        BillingLedgerOperation::ReconciliationAppend => "reconciliation_append",
        BillingLedgerOperation::Query => "query",
    }
}

fn billing_ledger_result_label(result: BillingLedgerResult) -> &'static str {
    match result {
        BillingLedgerResult::Written => "written",
        BillingLedgerResult::Read => "read",
        BillingLedgerResult::Skipped => "skipped",
        BillingLedgerResult::Failed => "failed",
    }
}

fn budget_rejection_reason_label(reason: BudgetRejectionReason) -> &'static str {
    match reason {
        BudgetRejectionReason::TenantBudgetExceeded => "tenant_budget_exceeded",
        BudgetRejectionReason::VirtualKeyBudgetExceeded => "virtual_key_budget_exceeded",
        BudgetRejectionReason::RateLimited => "rate_limited",
        BudgetRejectionReason::ReservationUnavailable => "reservation_unavailable",
        BudgetRejectionReason::PolicyDenied => "policy_denied",
    }
}

fn rate_limit_scope_label(scope: RateLimitScope) -> &'static str {
    match scope {
        RateLimitScope::Tenant => "tenant",
        RateLimitScope::VirtualKey => "virtual_key",
        RateLimitScope::Principal => "principal",
        RateLimitScope::Provider => "provider",
    }
}

fn rate_limit_decision_label(decision: RateLimitDecision) -> &'static str {
    match decision {
        RateLimitDecision::Allowed => "allowed",
        RateLimitDecision::Delayed => "delayed",
        RateLimitDecision::Rejected => "rejected",
        RateLimitDecision::Unavailable => "unavailable",
    }
}

fn redis_coordination_operation_label(operation: RedisCoordinationOperation) -> &'static str {
    match operation {
        RedisCoordinationOperation::RateLimitCheck => "rate_limit_check",
        RedisCoordinationOperation::RateLimitCommit => "rate_limit_commit",
        RedisCoordinationOperation::RecoveryLeaseAcquire => "recovery_lease_acquire",
        RedisCoordinationOperation::RecoveryLeaseRelease => "recovery_lease_release",
        RedisCoordinationOperation::CacheRead => "cache_read",
        RedisCoordinationOperation::CacheWrite => "cache_write",
    }
}

fn redis_coordination_result_label(result: RedisCoordinationResult) -> &'static str {
    match result {
        RedisCoordinationResult::Success => "success",
        RedisCoordinationResult::Limited => "limited",
        RedisCoordinationResult::LeaseUnavailable => "lease_unavailable",
        RedisCoordinationResult::CacheMiss => "cache_miss",
        RedisCoordinationResult::Unavailable => "unavailable",
        RedisCoordinationResult::Failed => "failed",
    }
}

fn quota_correctness_event_label(event: QuotaCorrectnessEvent) -> &'static str {
    match event {
        QuotaCorrectnessEvent::ReservationOvershoot => "reservation_overshoot",
        QuotaCorrectnessEvent::DuplicateChargePrevented => "duplicate_charge_prevented",
        QuotaCorrectnessEvent::MissingCommitRecovered => "missing_commit_recovered",
        QuotaCorrectnessEvent::MissingReleaseRecovered => "missing_release_recovered",
        QuotaCorrectnessEvent::LedgerMismatchDetected => "ledger_mismatch_detected",
    }
}

fn slo_alert_sli_label(sli: SloAlertSli) -> &'static str {
    match sli {
        SloAlertSli::Availability => "availability",
        SloAlertSli::LatencyP95 => "latency_p95",
        SloAlertSli::ErrorRate => "error_rate",
        SloAlertSli::QuotaCorrectness => "quota_correctness",
        SloAlertSli::ProviderDegradation => "provider_degradation",
        SloAlertSli::PersistenceFailure => "persistence_failure",
    }
}

fn slo_alert_severity_label(severity: SloAlertSeverity) -> &'static str {
    match severity {
        SloAlertSeverity::Warning => "warning",
        SloAlertSeverity::Critical => "critical",
    }
}

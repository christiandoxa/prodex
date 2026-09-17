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

#[cfg(not(feature = "mojo"))]
mod rust_compat {
    use super::*;
    pub fn plan_reservation_recovery_metric(
        operation: ReservationRecoveryOperation,
        result: ReservationRecoveryResult,
    ) -> Result<ReservationRecoveryMetricPlan, TelemetryAttributeError> {
        #[cfg(feature = "mojo")]
        let operation_label =
            crate::planning_support::planned_metric_label(6, 0, (operation) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(121, "reservation_recovery_operation"),
            reservation_recovery_operation_label(operation),
        )?;
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(6, 1, (result) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(122, "reservation_recovery_result"),
            reservation_recovery_result_label(result),
        )?;
        Ok(ReservationRecoveryMetricPlan {
            metric_name: crate::planning_support::metric_name(
                6,
                0,
                "prodex_reservation_recovery_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_accounting_metric(
        operation: AccountingOperation,
        result: AccountingResult,
    ) -> Result<AccountingMetricPlan, TelemetryAttributeError> {
        #[cfg(feature = "mojo")]
        let operation_label =
            crate::planning_support::planned_metric_label(0, 0, (operation) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(2, "accounting_operation"),
            accounting_operation_label(operation),
        )?;
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(0, 1, (result) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(3, "accounting_result"),
            accounting_result_label(result),
        )?;
        Ok(AccountingMetricPlan {
            metric_name: crate::planning_support::metric_name(
                0,
                0,
                "prodex_accounting_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_billing_ledger_metric(
        operation: BillingLedgerOperation,
        result: BillingLedgerResult,
    ) -> Result<BillingLedgerMetricPlan, TelemetryAttributeError> {
        #[cfg(feature = "mojo")]
        let operation_label =
            crate::planning_support::planned_metric_label(1, 0, (operation) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(49, "billing_ledger_operation"),
            billing_ledger_operation_label(operation),
        )?;
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(1, 1, (result) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(50, "billing_ledger_result"),
            billing_ledger_result_label(result),
        )?;
        Ok(BillingLedgerMetricPlan {
            metric_name: crate::planning_support::metric_name(
                1,
                0,
                "prodex_billing_ledger_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_budget_rejection_metric(
        reason: BudgetRejectionReason,
    ) -> Result<BudgetRejectionMetricPlan, TelemetryAttributeError> {
        #[cfg(feature = "mojo")]
        let reason_label = crate::planning_support::planned_metric_label(2, 0, (reason) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let reason_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(55, "budget_rejection_reason"),
            budget_rejection_reason_label(reason),
        )?;
        Ok(BudgetRejectionMetricPlan {
            metric_name: crate::planning_support::metric_name(
                2,
                0,
                "prodex_budget_rejections_total",
            ),
            increment: 1,
            reason_label,
        })
    }

    pub fn plan_rate_limit_decision_metric(
        scope: RateLimitScope,
        decision: RateLimitDecision,
    ) -> Result<RateLimitDecisionMetricPlan, TelemetryAttributeError> {
        #[cfg(feature = "mojo")]
        let scope_label = crate::planning_support::planned_metric_label(4, 0, (scope) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let scope_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(118, "rate_limit_scope"),
            rate_limit_scope_label(scope),
        )?;
        #[cfg(feature = "mojo")]
        let decision_label =
            crate::planning_support::planned_metric_label(4, 1, (decision) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let decision_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(117, "rate_limit_decision"),
            rate_limit_decision_label(decision),
        )?;
        Ok(RateLimitDecisionMetricPlan {
            metric_name: crate::planning_support::metric_name(
                4,
                0,
                "prodex_rate_limit_decisions_total",
            ),
            increment: 1,
            scope_label,
            decision_label,
        })
    }

    pub fn plan_redis_coordination_metric(
        operation: RedisCoordinationOperation,
        result: RedisCoordinationResult,
    ) -> Result<RedisCoordinationMetricPlan, TelemetryAttributeError> {
        #[cfg(feature = "mojo")]
        let operation_label =
            crate::planning_support::planned_metric_label(5, 0, (operation) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(119, "redis_coordination_operation"),
            redis_coordination_operation_label(operation),
        )?;
        #[cfg(feature = "mojo")]
        let result_label = crate::planning_support::planned_metric_label(5, 1, (result) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(120, "redis_coordination_result"),
            redis_coordination_result_label(result),
        )?;
        Ok(RedisCoordinationMetricPlan {
            metric_name: crate::planning_support::metric_name(
                5,
                0,
                "prodex_redis_coordination_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_quota_correctness_metric(
        event: QuotaCorrectnessEvent,
    ) -> Result<QuotaCorrectnessMetricPlan, TelemetryAttributeError> {
        #[cfg(feature = "mojo")]
        let event_label = crate::planning_support::planned_metric_label(3, 0, (event) as i64)?;
        #[cfg(not(feature = "mojo"))]
        let event_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(116, "quota_correctness_event"),
            quota_correctness_event_label(event),
        )?;
        Ok(QuotaCorrectnessMetricPlan {
            metric_name: crate::planning_support::metric_name(
                3,
                0,
                "prodex_quota_correctness_events_total",
            ),
            increment: 1,
            event_label,
        })
    }

    #[cfg(not(feature = "mojo"))]
    fn reservation_recovery_operation_label(operation: ReservationRecoveryOperation) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(10, operation as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match operation {
                ReservationRecoveryOperation::ScanExpired => "scan_expired",
                ReservationRecoveryOperation::AcquireLease => "acquire_lease",
                ReservationRecoveryOperation::ReleaseBudget => "release_budget",
                ReservationRecoveryOperation::WriteLedger => "write_ledger",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn reservation_recovery_result_label(result: ReservationRecoveryResult) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(11, result as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match result {
                ReservationRecoveryResult::Recovered => "recovered",
                ReservationRecoveryResult::Skipped => "skipped",
                ReservationRecoveryResult::LeaseUnavailable => "lease_unavailable",
                ReservationRecoveryResult::Failed => "failed",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn accounting_operation_label(operation: AccountingOperation) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(0, operation as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match operation {
                AccountingOperation::Reservation => "reservation",
                AccountingOperation::Commit => "commit",
                AccountingOperation::Release => "release",
                AccountingOperation::Expire => "expire",
                AccountingOperation::Reconciliation => "reconciliation",
                AccountingOperation::BudgetRejection => "budget_rejection",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn accounting_result_label(result: AccountingResult) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(1, result as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match result {
                AccountingResult::Accepted => "accepted",
                AccountingResult::Rejected => "rejected",
                AccountingResult::Committed => "committed",
                AccountingResult::Released => "released",
                AccountingResult::Expired => "expired",
                AccountingResult::Reconciled => "reconciled",
                AccountingResult::Failed => "failed",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn billing_ledger_operation_label(operation: BillingLedgerOperation) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(2, operation as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match operation {
                BillingLedgerOperation::ReserveAppend => "reserve_append",
                BillingLedgerOperation::CommitAppend => "commit_append",
                BillingLedgerOperation::ReleaseAppend => "release_append",
                BillingLedgerOperation::ReconciliationAppend => "reconciliation_append",
                BillingLedgerOperation::Query => "query",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn billing_ledger_result_label(result: BillingLedgerResult) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(3, result as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match result {
                BillingLedgerResult::Written => "written",
                BillingLedgerResult::Read => "read",
                BillingLedgerResult::Skipped => "skipped",
                BillingLedgerResult::Failed => "failed",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn budget_rejection_reason_label(reason: BudgetRejectionReason) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(4, reason as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match reason {
                BudgetRejectionReason::TenantBudgetExceeded => "tenant_budget_exceeded",
                BudgetRejectionReason::VirtualKeyBudgetExceeded => "virtual_key_budget_exceeded",
                BudgetRejectionReason::RateLimited => "rate_limited",
                BudgetRejectionReason::ReservationUnavailable => "reservation_unavailable",
                BudgetRejectionReason::PolicyDenied => "policy_denied",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn rate_limit_scope_label(scope: RateLimitScope) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(7, scope as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match scope {
                RateLimitScope::Tenant => "tenant",
                RateLimitScope::VirtualKey => "virtual_key",
                RateLimitScope::Principal => "principal",
                RateLimitScope::Provider => "provider",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn rate_limit_decision_label(decision: RateLimitDecision) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(6, decision as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match decision {
                RateLimitDecision::Allowed => "allowed",
                RateLimitDecision::Delayed => "delayed",
                RateLimitDecision::Rejected => "rejected",
                RateLimitDecision::Unavailable => "unavailable",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn redis_coordination_operation_label(operation: RedisCoordinationOperation) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(8, operation as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match operation {
                RedisCoordinationOperation::RateLimitCheck => "rate_limit_check",
                RedisCoordinationOperation::RateLimitCommit => "rate_limit_commit",
                RedisCoordinationOperation::RecoveryLeaseAcquire => "recovery_lease_acquire",
                RedisCoordinationOperation::RecoveryLeaseRelease => "recovery_lease_release",
                RedisCoordinationOperation::CacheRead => "cache_read",
                RedisCoordinationOperation::CacheWrite => "cache_write",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn redis_coordination_result_label(result: RedisCoordinationResult) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(9, result as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match result {
                RedisCoordinationResult::Success => "success",
                RedisCoordinationResult::Limited => "limited",
                RedisCoordinationResult::LeaseUnavailable => "lease_unavailable",
                RedisCoordinationResult::CacheMiss => "cache_miss",
                RedisCoordinationResult::Unavailable => "unavailable",
                RedisCoordinationResult::Failed => "failed",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn quota_correctness_event_label(event: QuotaCorrectnessEvent) -> String {
        #[cfg(feature = "mojo")]
        {
            prodex_mojo_core::observability::label(5, event as i64)
                .expect("Mojo observability label planner returned invalid output")
        }
        #[cfg(not(feature = "mojo"))]
        {
            (match event {
                QuotaCorrectnessEvent::ReservationOvershoot => "reservation_overshoot",
                QuotaCorrectnessEvent::DuplicateChargePrevented => "duplicate_charge_prevented",
                QuotaCorrectnessEvent::MissingCommitRecovered => "missing_commit_recovered",
                QuotaCorrectnessEvent::MissingReleaseRecovered => "missing_release_recovered",
                QuotaCorrectnessEvent::LedgerMismatchDetected => "ledger_mismatch_detected",
            })
            .to_string()
        }
    }
}

#[cfg(not(feature = "mojo"))]
pub use rust_compat::*;

#[cfg(feature = "mojo")]
mod mojo_impl {
    use super::*;

    macro_rules! one_label_plan {
        ($name:ident, $return_type:ident, $plan:literal, $value:ident : $value_type:ty => $label:ident) => {
            pub fn $name($value: $value_type) -> Result<$return_type, TelemetryAttributeError> {
                Ok($return_type {
                    metric_name: crate::planning_support::metric_name($plan, 0, ""),
                    increment: 1,
                    $label: crate::planning_support::planned_metric_label($plan, 0, $value as i64)?,
                })
            }
        };
    }
    macro_rules! two_label_plan {
        ($name:ident, $return_type:ident, $plan:literal, $left:ident : $left_type:ty => $left_label:ident, $right:ident : $right_type:ty => $right_label:ident) => {
            pub fn $name(
                $left: $left_type,
                $right: $right_type,
            ) -> Result<$return_type, TelemetryAttributeError> {
                Ok($return_type {
                    metric_name: crate::planning_support::metric_name($plan, 0, ""),
                    increment: 1,
                    $left_label: crate::planning_support::planned_metric_label(
                        $plan,
                        0,
                        $left as i64,
                    )?,
                    $right_label: crate::planning_support::planned_metric_label(
                        $plan,
                        1,
                        $right as i64,
                    )?,
                })
            }
        };
    }

    two_label_plan!(plan_reservation_recovery_metric, ReservationRecoveryMetricPlan, 6, operation: ReservationRecoveryOperation => operation_label, result: ReservationRecoveryResult => result_label);
    two_label_plan!(plan_accounting_metric, AccountingMetricPlan, 0, operation: AccountingOperation => operation_label, result: AccountingResult => result_label);
    two_label_plan!(plan_billing_ledger_metric, BillingLedgerMetricPlan, 1, operation: BillingLedgerOperation => operation_label, result: BillingLedgerResult => result_label);
    one_label_plan!(plan_budget_rejection_metric, BudgetRejectionMetricPlan, 2, reason: BudgetRejectionReason => reason_label);
    two_label_plan!(plan_rate_limit_decision_metric, RateLimitDecisionMetricPlan, 4, scope: RateLimitScope => scope_label, decision: RateLimitDecision => decision_label);
    two_label_plan!(plan_redis_coordination_metric, RedisCoordinationMetricPlan, 5, operation: RedisCoordinationOperation => operation_label, result: RedisCoordinationResult => result_label);
    one_label_plan!(plan_quota_correctness_metric, QuotaCorrectnessMetricPlan, 3, event: QuotaCorrectnessEvent => event_label);
}

#[cfg(feature = "mojo")]
pub use mojo_impl::*;

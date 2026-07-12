use std::{fmt, ops::Range, time::Duration};

use prodex_domain::ReservationReconciliationReason;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationUsageReconciliationBackend {
    File,
    Sqlite,
    Postgres,
    Redis,
}

impl ApplicationUsageReconciliationBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
            Self::Redis => "redis",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationUsageReconciliationAuditOutcome {
    Success,
    Failure,
}

impl ApplicationUsageReconciliationAuditOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ApplicationUsageReconciliationRetryPlan {
    max_attempts: u8,
    backoff: Duration,
}

impl fmt::Debug for ApplicationUsageReconciliationRetryPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationUsageReconciliationRetryPlan")
            .field("max_attempts", &"<redacted>")
            .field("backoff", &"<redacted>")
            .finish()
    }
}

impl ApplicationUsageReconciliationRetryPlan {
    pub fn attempts(self) -> Range<u8> {
        0..self.max_attempts
    }

    pub fn backoff_after(self, attempt: u8) -> Option<Duration> {
        attempt
            .checked_add(1)
            .is_some_and(|next| next < self.max_attempts)
            .then_some(self.backoff)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ApplicationUsageReconciliationAuditPlan {
    backend: ApplicationUsageReconciliationBackend,
    reason: ReservationReconciliationReason,
    outcome: ApplicationUsageReconciliationAuditOutcome,
}

impl fmt::Debug for ApplicationUsageReconciliationAuditPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationUsageReconciliationAuditPlan")
            .field("backend", &"<redacted>")
            .field("reason", &"<redacted>")
            .field("outcome", &"<redacted>")
            .finish()
    }
}

impl ApplicationUsageReconciliationAuditPlan {
    pub fn component(self) -> &'static str {
        "gateway_data_plane"
    }

    pub fn action(self) -> &'static str {
        "usage_reconciliation"
    }

    pub fn backend(self) -> &'static str {
        self.backend.as_str()
    }

    pub fn reason(self) -> &'static str {
        match self.reason {
            ReservationReconciliationReason::Completed => "completed",
            ReservationReconciliationReason::Cancelled => "cancelled",
            ReservationReconciliationReason::StreamInterrupted => "stream_interrupted",
        }
    }

    pub fn outcome(self) -> &'static str {
        self.outcome.as_str()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationUsageReconciliationExecutionRequest {
    pub backend: ApplicationUsageReconciliationBackend,
    pub reason: ReservationReconciliationReason,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ApplicationUsageReconciliationExecutionPlan {
    pub retry: ApplicationUsageReconciliationRetryPlan,
    backend: ApplicationUsageReconciliationBackend,
    reason: ReservationReconciliationReason,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ApplicationUsageReconciliationExhaustionPlan {
    error_kind: &'static str,
}

impl fmt::Debug for ApplicationUsageReconciliationExhaustionPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationUsageReconciliationExhaustionPlan")
            .field("error_kind", &"<redacted>")
            .finish()
    }
}

impl ApplicationUsageReconciliationExhaustionPlan {
    pub fn error_kind(self) -> &'static str {
        self.error_kind
    }
}

impl fmt::Debug for ApplicationUsageReconciliationExecutionPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationUsageReconciliationExecutionPlan")
            .field("retry", &self.retry)
            .field("backend", &"<redacted>")
            .field("reason", &"<redacted>")
            .finish()
    }
}

impl ApplicationUsageReconciliationExecutionPlan {
    pub fn audit(
        self,
        outcome: ApplicationUsageReconciliationAuditOutcome,
    ) -> ApplicationUsageReconciliationAuditPlan {
        ApplicationUsageReconciliationAuditPlan {
            backend: self.backend,
            reason: self.reason,
            outcome,
        }
    }

    pub fn exhausted(
        self,
        storage_error_observed: bool,
    ) -> ApplicationUsageReconciliationExhaustionPlan {
        ApplicationUsageReconciliationExhaustionPlan {
            error_kind: if storage_error_observed {
                "gateway_ledger_persistence_failed"
            } else {
                "gateway_reconciliation_retry_exhausted"
            },
        }
    }
}

pub fn plan_application_usage_reconciliation_execution(
    request: ApplicationUsageReconciliationExecutionRequest,
) -> ApplicationUsageReconciliationExecutionPlan {
    ApplicationUsageReconciliationExecutionPlan {
        retry: ApplicationUsageReconciliationRetryPlan {
            max_attempts: 25,
            backoff: Duration::from_millis(20),
        },
        backend: request.backend,
        reason: request.reason,
    }
}

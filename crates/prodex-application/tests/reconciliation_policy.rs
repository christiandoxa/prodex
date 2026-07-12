use std::time::Duration;

use prodex_application::{
    ApplicationUsageReconciliationAuditOutcome, ApplicationUsageReconciliationBackend,
    ApplicationUsageReconciliationExecutionRequest,
    plan_application_usage_reconciliation_execution,
};
use prodex_domain::ReservationReconciliationReason;

#[test]
fn reconciliation_retry_schedule_preserves_twenty_five_attempts_and_backoff() {
    let plan = plan_application_usage_reconciliation_execution(
        ApplicationUsageReconciliationExecutionRequest {
            backend: ApplicationUsageReconciliationBackend::File,
            reason: ReservationReconciliationReason::Completed,
        },
    );

    assert_eq!(
        plan.retry.attempts().collect::<Vec<_>>(),
        (0..25).collect::<Vec<_>>()
    );
    for attempt in 0..24 {
        assert_eq!(
            plan.retry.backoff_after(attempt),
            Some(Duration::from_millis(20))
        );
    }
    assert_eq!(plan.retry.backoff_after(24), None);
    assert_eq!(plan.retry.backoff_after(u8::MAX), None);
    assert_eq!(
        plan.exhausted(false).error_kind(),
        "gateway_reconciliation_retry_exhausted"
    );
    assert_eq!(
        plan.exhausted(true).error_kind(),
        "gateway_ledger_persistence_failed"
    );
}

#[test]
fn reconciliation_audit_semantics_preserve_backend_reason_and_outcome_labels() {
    for (backend, backend_label) in [
        (ApplicationUsageReconciliationBackend::File, "file"),
        (ApplicationUsageReconciliationBackend::Sqlite, "sqlite"),
        (ApplicationUsageReconciliationBackend::Postgres, "postgres"),
        (ApplicationUsageReconciliationBackend::Redis, "redis"),
    ] {
        for (reason, reason_label) in [
            (ReservationReconciliationReason::Completed, "completed"),
            (ReservationReconciliationReason::Cancelled, "cancelled"),
            (
                ReservationReconciliationReason::StreamInterrupted,
                "stream_interrupted",
            ),
        ] {
            let plan = plan_application_usage_reconciliation_execution(
                ApplicationUsageReconciliationExecutionRequest { backend, reason },
            );
            for (outcome, outcome_label) in [
                (
                    ApplicationUsageReconciliationAuditOutcome::Success,
                    "success",
                ),
                (
                    ApplicationUsageReconciliationAuditOutcome::Failure,
                    "failure",
                ),
            ] {
                let audit = plan.audit(outcome);
                assert_eq!(audit.component(), "gateway_data_plane");
                assert_eq!(audit.action(), "usage_reconciliation");
                assert_eq!(audit.backend(), backend_label);
                assert_eq!(audit.reason(), reason_label);
                assert_eq!(audit.outcome(), outcome_label);
            }
        }
    }
}

#[test]
fn reconciliation_plans_have_redacted_debug_output() {
    let plan = plan_application_usage_reconciliation_execution(
        ApplicationUsageReconciliationExecutionRequest {
            backend: ApplicationUsageReconciliationBackend::Postgres,
            reason: ReservationReconciliationReason::StreamInterrupted,
        },
    );
    let debug = format!(
        "{plan:?} {:?}",
        plan.audit(ApplicationUsageReconciliationAuditOutcome::Failure)
    );

    assert!(!debug.contains("postgres"));
    assert!(!debug.contains("stream_interrupted"));
    assert!(!debug.contains("failure"));
    assert!(debug.contains("<redacted>"));
}

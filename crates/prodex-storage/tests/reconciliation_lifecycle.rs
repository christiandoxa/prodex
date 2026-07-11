use prodex_domain::{
    BudgetSnapshot, CallId, LedgerEventKind, ReservationId, ReservationReconciliationReason,
    ReservationRecord, ReservationRequest, TenantId, UsageAmount, VirtualKeyId,
};
use prodex_storage::{TenantStorageKey, UsageReconciliationCommand, plan_usage_reconciliation};

fn interrupted_command(reason: ReservationReconciliationReason) -> UsageReconciliationCommand {
    let tenant_id = TenantId::new();
    let reserved = UsageAmount::new(100, 1_000);
    let request = ReservationRequest {
        tenant_id,
        call_id: CallId::new(),
        reservation_id: ReservationId::new(),
        estimate: reserved,
    };
    UsageReconciliationCommand {
        storage_key: TenantStorageKey::virtual_key(tenant_id, VirtualKeyId::new()),
        snapshot: BudgetSnapshot {
            reserved,
            committed: UsageAmount::new(10, 100),
        },
        record: ReservationRecord::from_request(request, 1_000, 60_000).unwrap(),
        actual: UsageAmount::new(40, 400),
        reason,
    }
}

#[test]
fn cancellation_commits_partial_usage_and_releases_the_unconsumed_reservation() {
    assert_interrupted_reconciliation(ReservationReconciliationReason::Cancelled);
}

#[test]
fn interrupted_stream_commits_partial_usage_and_releases_the_unconsumed_reservation() {
    assert_interrupted_reconciliation(ReservationReconciliationReason::StreamInterrupted);
}

fn assert_interrupted_reconciliation(reason: ReservationReconciliationReason) {
    let plan = plan_usage_reconciliation(interrupted_command(reason)).unwrap();

    assert_eq!(plan.reconciliation.reason, reason);
    assert_eq!(plan.reconciliation.commit.actual, UsageAmount::new(40, 400));
    assert_eq!(plan.updated_snapshot.reserved, UsageAmount::ZERO);
    assert_eq!(plan.updated_snapshot.committed, UsageAmount::new(50, 500));
    assert_eq!(plan.ledger_events.len(), 2);
    assert_eq!(plan.ledger_events[0].kind, LedgerEventKind::Committed);
    assert_eq!(plan.ledger_events[0].amount, UsageAmount::new(40, 400));
    assert_eq!(plan.ledger_events[1].kind, LedgerEventKind::Released);
    assert_eq!(plan.ledger_events[1].amount, UsageAmount::new(60, 600));
}

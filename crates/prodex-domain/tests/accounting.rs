use prodex_domain::{
    AccountingErrorStatus, BudgetLimit, BudgetRejectionReason, BudgetSnapshot, CallId, LedgerEvent,
    LedgerEventKind, ReservationCommit, ReservationCommitError, ReservationCommitMismatch,
    ReservationId, ReservationReconciliationError, ReservationReconciliationReason,
    ReservationRecord, ReservationRecoveryError, ReservationRequest, TenantId, UsageAmount,
    commit_reservation, commit_reservation_checked, plan_budget_rejection_response,
    plan_reservation_commit_error_response, plan_reservation_commit_mismatch_response,
    plan_reservation_reconciliation_error_response, plan_reservation_recovery_error_response,
    reconcile_reserved_usage, release_expired_reservation, reserve_budget,
};

fn reservation(tenant_id: TenantId, estimate: UsageAmount) -> ReservationRequest {
    ReservationRequest {
        tenant_id,
        call_id: CallId::new(),
        reservation_id: ReservationId::new(),
        estimate,
    }
}

#[test]
fn usage_amount_debug_output_is_stable_and_redacted() {
    let amount = UsageAmount::new(123_456, 987_654);

    let rendered = format!("{amount:?}");
    for sensitive in ["123456", "987654"] {
        assert!(
            !rendered.contains(sensitive),
            "usage amount debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert_eq!(
        rendered,
        "UsageAmount { tokens: \"<redacted>\", cost_micros: \"<redacted>\" }"
    );
}

#[test]
fn budget_limit_debug_output_is_stable_and_redacted() {
    let limit = BudgetLimit::new(100, 10_000);

    let rendered = format!("{limit:?}");
    for sensitive in ["100", "10000"] {
        assert!(
            !rendered.contains(sensitive),
            "budget limit debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert_eq!(rendered, "BudgetLimit { max: \"<redacted>\" }");
}

#[test]
fn budget_snapshot_debug_output_is_stable_and_redacted() {
    let snapshot = BudgetSnapshot {
        reserved: UsageAmount::new(80, 800),
        committed: UsageAmount::new(7, 70),
    };

    let rendered = format!("{snapshot:?}");
    for sensitive in ["80", "800", "7", "70"] {
        assert!(
            !rendered.contains(sensitive),
            "budget snapshot debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert_eq!(
        rendered,
        "BudgetSnapshot { reserved: \"<redacted>\", committed: \"<redacted>\" }"
    );
}

#[test]
fn reservation_request_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let request = reservation(tenant_id, UsageAmount::new(11, 100));

    let rendered = format!("{request:?}");
    for sensitive in [
        tenant_id.to_string(),
        request.call_id.to_string(),
        request.reservation_id.to_string(),
        "tokens=11".to_string(),
        "cost_micros=100".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "reservation request debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("ReservationRequest"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn reservation_rejects_before_upstream_when_token_limit_is_insufficient() {
    let tenant_id = TenantId::new();
    let snapshot = BudgetSnapshot {
        reserved: UsageAmount::new(80, 0),
        committed: UsageAmount::new(10, 0),
    };
    let limit = BudgetLimit::new(100, 10_000);

    let rejection = reserve_budget(
        snapshot,
        limit,
        reservation(tenant_id, UsageAmount::new(11, 1)),
    )
    .unwrap_err();

    assert_eq!(rejection.reason, BudgetRejectionReason::TokenLimitExceeded);
    assert_eq!(rejection.available.tokens, 10);
}

#[test]
fn reservation_rejects_before_upstream_when_cost_limit_is_insufficient() {
    let tenant_id = TenantId::new();
    let snapshot = BudgetSnapshot {
        reserved: UsageAmount::new(0, 900),
        committed: UsageAmount::new(0, 50),
    };
    let limit = BudgetLimit::new(10_000, 1_000);

    let rejection = reserve_budget(
        snapshot,
        limit,
        reservation(tenant_id, UsageAmount::new(1, 51)),
    )
    .unwrap_err();

    assert_eq!(rejection.reason, BudgetRejectionReason::CostLimitExceeded);
    assert_eq!(rejection.available.cost_micros, 50);
}

#[test]
fn reservation_adds_estimate_to_reserved_balance() {
    let tenant_id = TenantId::new();
    let snapshot = BudgetSnapshot::default();
    let limit = BudgetLimit::new(100, 1_000);

    let next = reserve_budget(
        snapshot,
        limit,
        reservation(tenant_id, UsageAmount::new(12, 34)),
    )
    .unwrap();

    assert_eq!(next.reserved, UsageAmount::new(12, 34));
    assert_eq!(next.committed, UsageAmount::ZERO);
}

#[test]
fn reservation_rejects_zero_estimate_before_upstream() {
    let tenant_id = TenantId::new();
    let rejection = reserve_budget(
        BudgetSnapshot::default(),
        BudgetLimit::new(100, 1_000),
        reservation(tenant_id, UsageAmount::ZERO),
    )
    .unwrap_err();

    assert_eq!(rejection.reason, BudgetRejectionReason::ZeroEstimate);
    assert_eq!(rejection.requested, UsageAmount::ZERO);
}

#[test]
fn budget_rejection_debug_output_is_stable_and_redacted() {
    let rejection = prodex_domain::BudgetRejection {
        reason: BudgetRejectionReason::TokenLimitExceeded,
        available: UsageAmount::new(10, 1_000),
        requested: UsageAmount::new(11, 1),
    };

    let rendered = format!("{rejection:?}");
    for sensitive in [
        "tokens=10",
        "cost_micros=1000",
        "tokens=11",
        "cost_micros=1",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "budget rejection debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("TokenLimitExceeded"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn commit_releases_reserved_delta_and_commits_actual_usage() {
    let tenant_id = TenantId::new();
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();
    let snapshot = BudgetSnapshot {
        reserved: UsageAmount::new(100, 1_000),
        committed: UsageAmount::new(7, 70),
    };

    let next = commit_reservation(
        snapshot,
        ReservationCommit {
            tenant_id,
            call_id,
            reservation_id,
            reserved: UsageAmount::new(100, 1_000),
            actual: UsageAmount::new(80, 800),
        },
    )
    .unwrap();

    assert_eq!(next.reserved, UsageAmount::ZERO);
    assert_eq!(next.committed, UsageAmount::new(87, 870));
}

#[test]
fn ledger_event_idempotency_key_includes_tenant_call_reservation_and_kind() {
    let tenant_id = TenantId::new();
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();
    let reserved = LedgerEvent {
        tenant_id,
        call_id,
        reservation_id,
        kind: LedgerEventKind::Reserved,
        amount: UsageAmount::new(10, 20),
    };
    let committed = LedgerEvent {
        kind: LedgerEventKind::Committed,
        ..reserved
    };

    assert_ne!(reserved.idempotency_key(), committed.idempotency_key());
    assert_eq!(reserved.idempotency_key().0, tenant_id);
    assert_eq!(reserved.idempotency_key().1, call_id);
    assert_eq!(reserved.idempotency_key().2, reservation_id);
}

#[test]
fn ledger_event_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();
    let event = LedgerEvent {
        tenant_id,
        call_id,
        reservation_id,
        kind: LedgerEventKind::Committed,
        amount: UsageAmount::new(10, 20),
    };

    let rendered = format!("{event:?}");
    for sensitive in [
        tenant_id.to_string(),
        call_id.to_string(),
        reservation_id.to_string(),
        "tokens=10".to_string(),
        "cost_micros=20".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "ledger event debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("Committed"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn checked_commit_rejects_cross_tenant_reservation_commit() {
    let tenant_id = TenantId::new();
    let request = reservation(tenant_id, UsageAmount::new(10, 100));
    let snapshot = BudgetSnapshot {
        committed: UsageAmount::ZERO,
        reserved: UsageAmount::new(10, 100),
    };
    let commit = ReservationCommit {
        tenant_id: TenantId::new(),
        call_id: request.call_id,
        reservation_id: request.reservation_id,
        reserved: request.estimate,
        actual: UsageAmount::new(4, 40),
    };

    assert_eq!(
        commit_reservation_checked(snapshot, request, commit),
        Err(ReservationCommitError::Mismatch(
            ReservationCommitMismatch::Tenant {
                expected: tenant_id,
                actual: commit.tenant_id,
            }
        ))
    );
}

#[test]
fn checked_commit_rejects_wrong_call_or_reservation_id() {
    let request = reservation(TenantId::new(), UsageAmount::new(10, 100));
    let snapshot = BudgetSnapshot {
        committed: UsageAmount::ZERO,
        reserved: UsageAmount::new(10, 100),
    };
    let wrong_call = ReservationCommit {
        tenant_id: request.tenant_id,
        call_id: CallId::new(),
        reservation_id: request.reservation_id,
        reserved: request.estimate,
        actual: UsageAmount::new(4, 40),
    };
    assert_eq!(
        commit_reservation_checked(snapshot, request, wrong_call),
        Err(ReservationCommitError::Mismatch(
            ReservationCommitMismatch::Call {
                expected: request.call_id,
                actual: wrong_call.call_id,
            }
        ))
    );

    let wrong_reservation = ReservationCommit {
        tenant_id: request.tenant_id,
        call_id: request.call_id,
        reservation_id: ReservationId::new(),
        reserved: request.estimate,
        actual: UsageAmount::new(4, 40),
    };
    assert_eq!(
        commit_reservation_checked(snapshot, request, wrong_reservation),
        Err(ReservationCommitError::Mismatch(
            ReservationCommitMismatch::Reservation {
                expected: request.reservation_id,
                actual: wrong_reservation.reservation_id,
            }
        ))
    );
}

#[test]
fn checked_commit_rejects_reserved_amount_mismatch() {
    let request = reservation(TenantId::new(), UsageAmount::new(10, 100));
    let snapshot = BudgetSnapshot {
        committed: UsageAmount::ZERO,
        reserved: UsageAmount::new(10, 100),
    };
    let commit = ReservationCommit {
        tenant_id: request.tenant_id,
        call_id: request.call_id,
        reservation_id: request.reservation_id,
        reserved: UsageAmount::new(9, 100),
        actual: UsageAmount::new(4, 40),
    };

    assert_eq!(
        commit_reservation_checked(snapshot, request, commit),
        Err(ReservationCommitError::Mismatch(
            ReservationCommitMismatch::ReservedAmount {
                expected: request.estimate,
                actual: commit.reserved,
            }
        ))
    );
}

#[test]
fn checked_commit_applies_matching_reservation_commit() {
    let request = reservation(TenantId::new(), UsageAmount::new(10, 100));
    let snapshot = BudgetSnapshot {
        committed: UsageAmount::new(1, 10),
        reserved: UsageAmount::new(10, 100),
    };
    let commit = ReservationCommit {
        tenant_id: request.tenant_id,
        call_id: request.call_id,
        reservation_id: request.reservation_id,
        reserved: request.estimate,
        actual: UsageAmount::new(4, 40),
    };

    let updated = commit_reservation_checked(snapshot, request, commit).unwrap();
    assert_eq!(updated.committed, UsageAmount::new(5, 50));
    assert_eq!(updated.reserved, UsageAmount::ZERO);
}

#[test]
fn commit_rejects_actual_above_reserved_and_committed_overflow() {
    let tenant_id = TenantId::new();
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();

    assert_eq!(
        commit_reservation(
            BudgetSnapshot {
                committed: UsageAmount::ZERO,
                reserved: UsageAmount::new(10, 100),
            },
            ReservationCommit {
                tenant_id,
                call_id,
                reservation_id,
                reserved: UsageAmount::new(10, 100),
                actual: UsageAmount::new(11, 100),
            },
        ),
        Err(ReservationCommitError::ActualExceedsReserved {
            reserved: UsageAmount::new(10, 100),
            actual: UsageAmount::new(11, 100),
        })
    );

    assert_eq!(
        commit_reservation(
            BudgetSnapshot {
                committed: UsageAmount::new(u64::MAX, 0),
                reserved: UsageAmount::new(1, 1),
            },
            ReservationCommit {
                tenant_id,
                call_id,
                reservation_id,
                reserved: UsageAmount::new(1, 1),
                actual: UsageAmount::new(1, 1),
            },
        ),
        Err(ReservationCommitError::CommittedUsageOverflow {
            committed: UsageAmount::new(u64::MAX, 0),
            actual: UsageAmount::new(1, 1),
        })
    );
}

#[test]
fn reservation_commit_mismatch_debug_output_is_stable_and_redacted() {
    let expected_tenant = TenantId::new();
    let actual_tenant = TenantId::new();
    let expected_call = CallId::new();
    let actual_call = CallId::new();
    let expected_reservation = ReservationId::new();
    let actual_reservation = ReservationId::new();
    let mismatches = [
        ReservationCommitMismatch::Tenant {
            expected: expected_tenant,
            actual: actual_tenant,
        },
        ReservationCommitMismatch::Call {
            expected: expected_call,
            actual: actual_call,
        },
        ReservationCommitMismatch::Reservation {
            expected: expected_reservation,
            actual: actual_reservation,
        },
        ReservationCommitMismatch::ReservedAmount {
            expected: UsageAmount::new(10, 100),
            actual: UsageAmount::new(9, 90),
        },
    ];

    for mismatch in &mismatches {
        assert_eq!(mismatch.to_string(), "reservation commit rejected");
    }

    let rendered = format!("{mismatches:?}");
    for sensitive in [
        expected_tenant.to_string(),
        actual_tenant.to_string(),
        expected_call.to_string(),
        actual_call.to_string(),
        expected_reservation.to_string(),
        actual_reservation.to_string(),
        "tokens=10".to_string(),
        "cost_micros=100".to_string(),
        "tokens=9".to_string(),
        "cost_micros=90".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "reservation commit mismatch debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("Tenant"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn reservation_commit_error_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let actual_tenant = TenantId::new();
    let errors = [
        ReservationCommitError::Mismatch(ReservationCommitMismatch::Tenant {
            expected: tenant_id,
            actual: actual_tenant,
        }),
        ReservationCommitError::ZeroActual,
        ReservationCommitError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(10, 100),
            available: UsageAmount::new(9, 90),
        },
        ReservationCommitError::ActualExceedsReserved {
            reserved: UsageAmount::new(10, 100),
            actual: UsageAmount::new(11, 110),
        },
        ReservationCommitError::CommittedUsageOverflow {
            committed: UsageAmount::new(u64::MAX, 0),
            actual: UsageAmount::new(1, 1),
        },
    ];

    let rendered = format!("{errors:?}");
    for sensitive in [
        tenant_id.to_string(),
        actual_tenant.to_string(),
        "tokens=10".to_string(),
        "cost_micros=100".to_string(),
        "tokens=9".to_string(),
        "cost_micros=90".to_string(),
        "tokens=11".to_string(),
        "cost_micros=110".to_string(),
        u64::MAX.to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "reservation commit error debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("ActualExceedsReserved"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn reservation_commit_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();
    let commit = ReservationCommit {
        tenant_id,
        call_id,
        reservation_id,
        reserved: UsageAmount::new(10, 100),
        actual: UsageAmount::new(9, 90),
    };

    let rendered = format!("{commit:?}");
    for sensitive in [
        tenant_id.to_string(),
        call_id.to_string(),
        reservation_id.to_string(),
        "tokens=10".to_string(),
        "cost_micros=100".to_string(),
        "tokens=9".to_string(),
        "cost_micros=90".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "reservation commit debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("ReservationCommit"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn commit_rejects_zero_actual_usage() {
    let tenant_id = TenantId::new();
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();

    assert_eq!(
        commit_reservation(
            BudgetSnapshot {
                committed: UsageAmount::ZERO,
                reserved: UsageAmount::new(10, 100),
            },
            ReservationCommit {
                tenant_id,
                call_id,
                reservation_id,
                reserved: UsageAmount::new(10, 100),
                actual: UsageAmount::ZERO,
            },
        ),
        Err(ReservationCommitError::ZeroActual)
    );
}

#[test]
fn commit_rejects_reserved_balance_underflow_instead_of_saturating() {
    let tenant_id = TenantId::new();
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();

    assert_eq!(
        commit_reservation(
            BudgetSnapshot {
                committed: UsageAmount::ZERO,
                reserved: UsageAmount::new(5, 100),
            },
            ReservationCommit {
                tenant_id,
                call_id,
                reservation_id,
                reserved: UsageAmount::new(10, 100),
                actual: UsageAmount::new(1, 1),
            },
        ),
        Err(ReservationCommitError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(10, 100),
            available: UsageAmount::new(5, 100),
        })
    );
}

#[test]
fn reservation_record_tracks_expiry_and_release_idempotency_key() {
    let request = reservation(TenantId::new(), UsageAmount::new(10, 100));
    let record = ReservationRecord::from_request(request, 1_000, 500).unwrap();

    assert!(!record.is_expired_at(1_499));
    assert!(record.is_expired_at(1_500));
    assert_eq!(record.expires_at_unix_ms, 1_500);
    assert_eq!(
        record.release_event().idempotency_key(),
        (
            request.tenant_id,
            request.call_id,
            request.reservation_id,
            LedgerEventKind::Released,
        )
    );
}

#[test]
fn reservation_record_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let request = reservation(tenant_id, UsageAmount::new(10, 100));
    let record = ReservationRecord::from_request(request, 1_000, 500).unwrap();

    let rendered = format!("{record:?}");
    for sensitive in [
        tenant_id.to_string(),
        request.call_id.to_string(),
        request.reservation_id.to_string(),
        "tokens=10".to_string(),
        "cost_micros=100".to_string(),
        "1000".to_string(),
        "1500".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "reservation record debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("ReservationRecord"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn reservation_record_rejects_expiry_overflow() {
    let request = reservation(TenantId::new(), UsageAmount::new(10, 100));

    assert_eq!(
        ReservationRecord::from_request(request, 1_000, 0),
        Err(ReservationRecoveryError::ZeroTtl)
    );
    assert_eq!(
        ReservationRecord::from_request(request, u64::MAX, 1),
        Err(ReservationRecoveryError::ExpiryOverflow)
    );
}

#[test]
fn reservation_record_rejects_zero_reserved_usage() {
    let request = reservation(TenantId::new(), UsageAmount::ZERO);

    assert_eq!(
        ReservationRecord::from_request(request, 1_000, 500),
        Err(ReservationRecoveryError::ZeroReserved)
    );
}

#[test]
fn expired_reservation_release_recovers_reserved_balance_without_charging() {
    let tenant_id = TenantId::new();
    let request = reservation(tenant_id, UsageAmount::new(10, 100));
    let record = ReservationRecord::from_request(request, 1_000, 500).unwrap();
    let snapshot = BudgetSnapshot {
        reserved: UsageAmount::new(10, 100),
        committed: UsageAmount::new(3, 30),
    };

    let (updated, event) = release_expired_reservation(snapshot, tenant_id, record, 1_500).unwrap();

    assert_eq!(updated.reserved, UsageAmount::ZERO);
    assert_eq!(updated.committed, UsageAmount::new(3, 30));
    assert_eq!(event.kind, LedgerEventKind::Released);
    assert_eq!(event.amount, UsageAmount::new(10, 100));
}

#[test]
fn expired_reservation_release_rejects_wrong_tenant_or_not_expired() {
    let tenant_id = TenantId::new();
    let request = reservation(tenant_id, UsageAmount::new(10, 100));
    let record = ReservationRecord::from_request(request, 1_000, 500).unwrap();
    let snapshot = BudgetSnapshot {
        reserved: UsageAmount::new(10, 100),
        committed: UsageAmount::ZERO,
    };

    assert_eq!(
        release_expired_reservation(snapshot, tenant_id, record, 1_499),
        Err(ReservationRecoveryError::NotExpired)
    );
    let wrong_tenant = TenantId::new();
    assert_eq!(
        release_expired_reservation(snapshot, wrong_tenant, record, 1_500),
        Err(ReservationRecoveryError::Tenant {
            expected: tenant_id,
            actual: wrong_tenant,
        })
    );
}

#[test]
fn expired_reservation_release_rejects_reserved_balance_underflow() {
    let tenant_id = TenantId::new();
    let request = reservation(tenant_id, UsageAmount::new(10, 100));
    let record = ReservationRecord::from_request(request, 1_000, 500).unwrap();

    assert_eq!(
        release_expired_reservation(
            BudgetSnapshot {
                reserved: UsageAmount::new(9, 100),
                committed: UsageAmount::ZERO,
            },
            tenant_id,
            record,
            1_500,
        ),
        Err(ReservationRecoveryError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(10, 100),
            available: UsageAmount::new(9, 100),
        })
    );
}

#[test]
fn reservation_recovery_error_debug_output_is_stable_and_redacted() {
    let expected_tenant = TenantId::new();
    let actual_tenant = TenantId::new();
    let errors = [
        ReservationRecoveryError::ExpiryOverflow,
        ReservationRecoveryError::ZeroTtl,
        ReservationRecoveryError::ZeroReserved,
        ReservationRecoveryError::NotExpired,
        ReservationRecoveryError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(10, 100),
            available: UsageAmount::new(9, 90),
        },
        ReservationRecoveryError::Tenant {
            expected: expected_tenant,
            actual: actual_tenant,
        },
    ];

    let rendered = format!("{errors:?}");
    for sensitive in [
        expected_tenant.to_string(),
        actual_tenant.to_string(),
        "tokens=10".to_string(),
        "cost_micros=100".to_string(),
        "tokens=9".to_string(),
        "cost_micros=90".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "reservation recovery error debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("NotExpired"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn cancellation_reconciliation_commits_partial_usage_and_releases_delta() {
    let request = reservation(TenantId::new(), UsageAmount::new(100, 1_000));
    let record = ReservationRecord::from_request(request, 1_000, 60_000).unwrap();
    let snapshot = BudgetSnapshot {
        reserved: UsageAmount::new(100, 1_000),
        committed: UsageAmount::new(5, 50),
    };

    let (updated, reconciliation) = reconcile_reserved_usage(
        snapshot,
        record,
        UsageAmount::new(37, 370),
        ReservationReconciliationReason::Cancelled,
    )
    .unwrap();

    assert_eq!(updated.reserved, UsageAmount::ZERO);
    assert_eq!(updated.committed, UsageAmount::new(42, 420));
    assert_eq!(
        reconciliation.reason,
        ReservationReconciliationReason::Cancelled
    );
    assert_eq!(reconciliation.commit.actual, UsageAmount::new(37, 370));
    assert_eq!(
        reconciliation.committed_event.idempotency_key(),
        (
            request.tenant_id,
            request.call_id,
            request.reservation_id,
            LedgerEventKind::Committed,
        )
    );
    let released_event = reconciliation.released_event.unwrap();
    assert_eq!(released_event.kind, LedgerEventKind::Released);
    assert_eq!(released_event.amount, UsageAmount::new(63, 630));
}

#[test]
fn reservation_reconciliation_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let request = reservation(tenant_id, UsageAmount::new(100, 1_000));
    let record = ReservationRecord::from_request(request, 1_000, 60_000).unwrap();
    let (_updated, reconciliation) = reconcile_reserved_usage(
        BudgetSnapshot {
            reserved: UsageAmount::new(100, 1_000),
            committed: UsageAmount::new(5, 50),
        },
        record,
        UsageAmount::new(37, 370),
        ReservationReconciliationReason::Cancelled,
    )
    .unwrap();

    let rendered = format!("{reconciliation:?}");
    for sensitive in [
        tenant_id.to_string(),
        request.call_id.to_string(),
        request.reservation_id.to_string(),
        "tokens=100".to_string(),
        "cost_micros=1000".to_string(),
        "tokens=37".to_string(),
        "cost_micros=370".to_string(),
        "tokens=63".to_string(),
        "cost_micros=630".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "reservation reconciliation debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("Cancelled"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn stream_interruption_reconciliation_without_delta_has_no_release_event() {
    let request = reservation(TenantId::new(), UsageAmount::new(100, 1_000));
    let record = ReservationRecord::from_request(request, 1_000, 60_000).unwrap();
    let snapshot = BudgetSnapshot {
        reserved: UsageAmount::new(100, 1_000),
        committed: UsageAmount::ZERO,
    };

    let (updated, reconciliation) = reconcile_reserved_usage(
        snapshot,
        record,
        UsageAmount::new(100, 1_000),
        ReservationReconciliationReason::StreamInterrupted,
    )
    .unwrap();

    assert_eq!(updated.reserved, UsageAmount::ZERO);
    assert_eq!(updated.committed, UsageAmount::new(100, 1_000));
    assert_eq!(reconciliation.released_event, None);
}

#[test]
fn reconciliation_commits_actual_usage_above_reserved_amount() {
    let request = reservation(TenantId::new(), UsageAmount::new(100, 1_000));
    let record = ReservationRecord::from_request(request, 1_000, 60_000).unwrap();

    let (updated, reconciliation) = reconcile_reserved_usage(
        BudgetSnapshot {
            reserved: UsageAmount::new(100, 1_000),
            committed: UsageAmount::ZERO,
        },
        record,
        UsageAmount::new(101, 1_100),
        ReservationReconciliationReason::Completed,
    )
    .unwrap();

    assert_eq!(updated.reserved, UsageAmount::ZERO);
    assert_eq!(updated.committed, UsageAmount::new(101, 1_100));
    assert_eq!(
        reconciliation.committed_event.amount,
        UsageAmount::new(101, 1_100)
    );
    assert_eq!(reconciliation.released_event, None);
}

#[test]
fn reconciliation_rejects_reserved_balance_underflow_instead_of_saturating() {
    let request = reservation(TenantId::new(), UsageAmount::new(100, 1_000));
    let record = ReservationRecord::from_request(request, 1_000, 60_000).unwrap();

    assert_eq!(
        reconcile_reserved_usage(
            BudgetSnapshot {
                reserved: UsageAmount::new(99, 1_000),
                committed: UsageAmount::ZERO,
            },
            record,
            UsageAmount::new(1, 1),
            ReservationReconciliationReason::Completed,
        ),
        Err(ReservationReconciliationError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(100, 1_000),
            available: UsageAmount::new(99, 1_000),
        })
    );
}

#[test]
fn reconciliation_rejects_committed_usage_overflow_instead_of_saturating() {
    let request = reservation(TenantId::new(), UsageAmount::new(10, 10));
    let record = ReservationRecord::from_request(request, 1_000, 60_000).unwrap();

    assert_eq!(
        reconcile_reserved_usage(
            BudgetSnapshot {
                reserved: UsageAmount::new(10, 10),
                committed: UsageAmount::new(u64::MAX, 0),
            },
            record,
            UsageAmount::new(1, 1),
            ReservationReconciliationReason::Completed,
        ),
        Err(ReservationReconciliationError::CommittedUsageOverflow {
            committed: UsageAmount::new(u64::MAX, 0),
            actual: UsageAmount::new(1, 1),
        })
    );
}

#[test]
fn reservation_reconciliation_error_debug_output_is_stable_and_redacted() {
    let errors = [
        ReservationReconciliationError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(100, 1_000),
            available: UsageAmount::new(99, 990),
        },
        ReservationReconciliationError::ActualExceedsReserved {
            reserved: UsageAmount::new(100, 1_000),
            actual: UsageAmount::new(101, 900),
        },
        ReservationReconciliationError::CommittedUsageOverflow {
            committed: UsageAmount::new(u64::MAX, 0),
            actual: UsageAmount::new(1, 1),
        },
    ];

    let rendered = format!("{errors:?}");
    for sensitive in [
        "tokens=100".to_string(),
        "cost_micros=1000".to_string(),
        "tokens=99".to_string(),
        "cost_micros=990".to_string(),
        "tokens=101".to_string(),
        "cost_micros=900".to_string(),
        u64::MAX.to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "reservation reconciliation error debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("ActualExceedsReserved"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn accounting_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let request = reservation(tenant_id, UsageAmount::new(11, 1));
    let budget_error = reserve_budget(
        BudgetSnapshot {
            reserved: UsageAmount::new(90, 0),
            committed: UsageAmount::ZERO,
        },
        BudgetLimit::new(100, 10_000),
        request,
    )
    .unwrap_err();
    assert!(!budget_error.to_string().contains("11"));
    assert!(!budget_error.to_string().contains("90"));
    let budget = plan_budget_rejection_response(&budget_error);
    assert_eq!(budget.status, AccountingErrorStatus::Forbidden);
    assert_eq!(budget.code, "budget_token_limit_exceeded");
    assert_eq!(budget.message, "budget reservation rejected");
    let zero_estimate = plan_budget_rejection_response(&prodex_domain::BudgetRejection {
        reason: BudgetRejectionReason::ZeroEstimate,
        available: UsageAmount::new(100, 1_000),
        requested: UsageAmount::ZERO,
    });
    assert_eq!(zero_estimate.code, "budget_estimate_invalid");

    let mismatch = plan_reservation_commit_mismatch_response(&ReservationCommitMismatch::Tenant {
        expected: tenant_id,
        actual: TenantId::new(),
    });
    assert_eq!(mismatch.status, AccountingErrorStatus::Conflict);
    assert_eq!(mismatch.code, "reservation_tenant_mismatch");
    assert_eq!(mismatch.message, "reservation commit rejected");
    let expected_call = CallId::new();
    let actual_call = CallId::new();
    let call_mismatch = ReservationCommitMismatch::Call {
        expected: expected_call,
        actual: actual_call,
    };
    assert!(
        !call_mismatch
            .to_string()
            .contains(&expected_call.to_string())
    );
    assert!(!call_mismatch.to_string().contains(&actual_call.to_string()));
    let reserved_amount_mismatch_error = ReservationCommitMismatch::ReservedAmount {
        expected: UsageAmount::new(2, 1),
        actual: UsageAmount::new(1, 1),
    };
    assert!(!reserved_amount_mismatch_error.to_string().contains("2"));
    let reserved_amount_mismatch =
        plan_reservation_commit_mismatch_response(&reserved_amount_mismatch_error);
    assert_eq!(
        reserved_amount_mismatch.code,
        "reservation_reserved_amount_mismatch"
    );
    let commit_overflow_error = ReservationCommitError::CommittedUsageOverflow {
        committed: UsageAmount::new(u64::MAX, 0),
        actual: UsageAmount::new(1, 1),
    };
    assert_eq!(
        commit_overflow_error.to_string(),
        "reservation commit rejected"
    );
    assert!(
        !commit_overflow_error
            .to_string()
            .contains(&u64::MAX.to_string())
    );
    let commit_overflow = plan_reservation_commit_error_response(&commit_overflow_error);
    assert_eq!(
        commit_overflow.status,
        AccountingErrorStatus::UnprocessableRequest
    );
    assert_eq!(commit_overflow.code, "reservation_committed_usage_overflow");
    let zero_actual = plan_reservation_commit_error_response(&ReservationCommitError::ZeroActual);
    assert_eq!(zero_actual.code, "reservation_actual_usage_invalid");
    let commit_actual_exceeds =
        plan_reservation_commit_error_response(&ReservationCommitError::ActualExceedsReserved {
            reserved: UsageAmount::new(1, 1),
            actual: UsageAmount::new(2, 1),
        });
    assert_eq!(
        commit_actual_exceeds.status,
        AccountingErrorStatus::UnprocessableRequest
    );
    assert_eq!(
        commit_actual_exceeds.code,
        "reservation_actual_usage_exceeds_reserved"
    );
    let commit_reserved_underflow =
        plan_reservation_commit_error_response(&ReservationCommitError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(2, 1),
            available: UsageAmount::new(1, 1),
        });
    assert_eq!(
        ReservationCommitError::ZeroActual.to_string(),
        "reservation commit rejected"
    );
    assert_eq!(
        ReservationCommitError::ActualExceedsReserved {
            reserved: UsageAmount::new(1, 1),
            actual: UsageAmount::new(2, 1),
        }
        .to_string(),
        "reservation commit rejected"
    );
    assert_eq!(
        commit_reserved_underflow.status,
        AccountingErrorStatus::Conflict
    );
    assert_eq!(
        commit_reserved_underflow.code,
        "reservation_reserved_balance_underflow"
    );

    let recovery = plan_reservation_recovery_error_response(&ReservationRecoveryError::NotExpired);
    assert_eq!(recovery.status, AccountingErrorStatus::Conflict);
    assert_eq!(recovery.code, "reservation_not_expired");
    let zero_ttl = plan_reservation_recovery_error_response(&ReservationRecoveryError::ZeroTtl);
    assert_eq!(zero_ttl.code, "reservation_ttl_invalid");
    let zero_reserved =
        plan_reservation_recovery_error_response(&ReservationRecoveryError::ZeroReserved);
    assert_eq!(zero_reserved.code, "reservation_reserved_usage_invalid");
    let recovery_underflow = plan_reservation_recovery_error_response(
        &ReservationRecoveryError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(2, 1),
            available: UsageAmount::new(1, 1),
        },
    );
    assert_eq!(
        recovery_underflow.code,
        "reservation_reserved_balance_underflow"
    );

    let reconciliation_error = ReservationReconciliationError::ActualExceedsReserved {
        reserved: UsageAmount::new(10, 100),
        actual: UsageAmount::new(11, 101),
    };
    assert_eq!(
        reconciliation_error.to_string(),
        "reservation reconciliation rejected"
    );
    assert!(!reconciliation_error.to_string().contains("101"));
    let reconciliation = plan_reservation_reconciliation_error_response(&reconciliation_error);
    assert_eq!(
        reconciliation.status,
        AccountingErrorStatus::UnprocessableRequest
    );
    assert_eq!(
        reconciliation.code,
        "reservation_actual_usage_exceeds_reserved"
    );
    let overflow_error = ReservationReconciliationError::CommittedUsageOverflow {
        committed: UsageAmount::new(u64::MAX, 0),
        actual: UsageAmount::new(1, 1),
    };
    assert_eq!(
        overflow_error.to_string(),
        "reservation reconciliation rejected"
    );
    assert!(!overflow_error.to_string().contains(&u64::MAX.to_string()));
    let overflow = plan_reservation_reconciliation_error_response(&overflow_error);
    assert_eq!(overflow.status, AccountingErrorStatus::UnprocessableRequest);
    assert_eq!(overflow.code, "reservation_committed_usage_overflow");
    let reconciliation_underflow = plan_reservation_reconciliation_error_response(
        &ReservationReconciliationError::ReservedBalanceUnderflow {
            reserved: UsageAmount::new(2, 1),
            available: UsageAmount::new(1, 1),
        },
    );
    assert_eq!(
        reconciliation_underflow.code,
        "reservation_reserved_balance_underflow"
    );

    let rendered = format!(
        "{budget:?} {mismatch:?} {reserved_amount_mismatch:?} {commit_overflow:?} {zero_actual:?} {commit_actual_exceeds:?} {commit_reserved_underflow:?} {recovery:?} {zero_ttl:?} {zero_reserved:?} {recovery_underflow:?} {reconciliation:?} {overflow:?} {reconciliation_underflow:?}"
    );
    for sensitive in [
        &tenant_id.to_string(),
        &request.call_id.to_string(),
        &request.reservation_id.to_string(),
        "tokens=",
        "cost_micros=",
        "reserved tokens",
        "actual tokens",
        "available",
        "requested",
        "90",
        "101",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "accounting response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn accounting_error_response_plan_debug_output_is_stable_and_redacted() {
    let response = plan_budget_rejection_response(&prodex_domain::BudgetRejection {
        reason: BudgetRejectionReason::TokenLimitExceeded,
        available: UsageAmount::new(90, 1_000),
        requested: UsageAmount::new(11, 1),
    });

    assert_eq!(
        format!("{response:?}"),
        "AccountingErrorResponsePlan { status: Forbidden, code: \"budget_token_limit_exceeded\", message: \"budget reservation rejected\" }"
    );
}

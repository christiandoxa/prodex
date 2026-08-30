use super::*;

pub fn reserve_budget(
    snapshot: BudgetSnapshot,
    limit: BudgetLimit,
    request: ReservationRequest,
) -> Result<BudgetSnapshot, BudgetRejection> {
    #[cfg(feature = "mojo")]
    {
        let result = prodex_mojo_core::policy::accounting_operation(
            prodex_mojo_core::policy::ACCOUNTING_RESERVE,
            &[
                snapshot.reserved.tokens,
                snapshot.reserved.cost_micros,
                snapshot.committed.tokens,
                snapshot.committed.cost_micros,
                limit.max.tokens,
                limit.max.cost_micros,
                request.estimate.tokens,
                request.estimate.cost_micros,
            ],
        )
        .unwrap_or_else(|_| panic!("Mojo budget reservation returned invalid output"));
        let rejection = |reason| BudgetRejection {
            reason,
            available: snapshot.available(limit),
            requested: request.estimate,
        };
        match result.result_code {
            0 => Ok(BudgetSnapshot {
                reserved: UsageAmount::new(result.values[0], result.values[1]),
                committed: snapshot.committed,
            }),
            1 => Err(BudgetRejection {
                reason: BudgetRejectionReason::ArithmeticOverflow,
                available: UsageAmount::ZERO,
                requested: request.estimate,
            }),
            2 => Err(rejection(BudgetRejectionReason::ZeroEstimate)),
            3 => Err(rejection(BudgetRejectionReason::TokenLimitExceeded)),
            4 => Err(rejection(BudgetRejectionReason::CostLimitExceeded)),
            _ => Err(rejection(BudgetRejectionReason::ArithmeticOverflow)),
        }
    }

    #[cfg(not(feature = "mojo"))]
    {
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
    #[cfg(feature = "mojo")]
    {
        let result = prodex_mojo_core::policy::accounting_operation(
            prodex_mojo_core::policy::ACCOUNTING_COMMIT,
            &[
                snapshot.reserved.tokens,
                snapshot.reserved.cost_micros,
                snapshot.committed.tokens,
                snapshot.committed.cost_micros,
                commit.reserved.tokens,
                commit.reserved.cost_micros,
                commit.actual.tokens,
                commit.actual.cost_micros,
            ],
        )
        .unwrap_or_else(|_| panic!("Mojo reservation commit returned invalid output"));
        match result.result_code {
            0 => Ok(BudgetSnapshot {
                reserved: UsageAmount::new(result.values[0], result.values[1]),
                committed: UsageAmount::new(result.values[2], result.values[3]),
            }),
            1 => Err(ReservationCommitError::ZeroActual),
            2 => Err(ReservationCommitError::ActualExceedsReserved {
                reserved: commit.reserved,
                actual: commit.actual,
            }),
            3 => Err(ReservationCommitError::ReservedBalanceUnderflow {
                reserved: commit.reserved,
                available: snapshot.reserved,
            }),
            4 => Err(ReservationCommitError::CommittedUsageOverflow {
                committed: snapshot.committed,
                actual: commit.actual,
            }),
            _ => Err(ReservationCommitError::CommittedUsageOverflow {
                committed: snapshot.committed,
                actual: commit.actual,
            }),
        }
    }

    #[cfg(not(feature = "mojo"))]
    {
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

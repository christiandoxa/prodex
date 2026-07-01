# 0411. Domain reservation reconciliation overflow

## Status

Accepted

## Context

Reservation commit and reconciliation apply measured post-provider usage and
release any unused reservation. The enterprise accounting path must not silently
saturate usage totals when committed counters overflow.

## Decision

`commit_reservation` and `reconcile_reserved_usage` now check
`snapshot.committed + actual` with `UsageAmount::checked_add` before
constructing the updated snapshot. They also reject actual usage above the
reserved amount before releasing reservation deltas.

Commit overflow returns `ReservationCommitError::CommittedUsageOverflow` and
maps to `reservation_committed_usage_overflow`. Reconciliation overflow returns
`ReservationReconciliationError::CommittedUsageOverflow` and maps to
`reservation_committed_usage_overflow`.

## Consequences

Adapters get a deterministic failure instead of a saturated accounting value.
Rollback is to remove the fallible commit/reconciliation branches, but that
reopens silent accounting corruption risk.

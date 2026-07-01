# ADR 0111: Storage Expired Reservation Recovery Contract

## Status

Accepted

## Context

Reservation-based accounting must recover abandoned reservations. A gateway may
crash, a stream may disappear before reconciliation, or a client may never
complete a request. Without a shared recovery contract, replicas can leak
reserved budget or reimplement release logic inconsistently.

## Decision

`prodex-storage` exposes `ExpiredReservationRecoveryCommand`,
`ExpiredReservationRecoveryPlan`, and `plan_expired_reservation_recovery`. The
plan:

- validates tenant-scoped storage keys against the reservation record;
- rejects records that have not expired;
- releases reserved capacity without committing usage;
- emits the idempotent released ledger event from the domain reservation model.

The contract is adapter-neutral and contains no database driver, filesystem,
network, async runtime, HTTP, or provider dependency.

## Consequences

- Durable adapters can implement periodic reservation recovery using the same
  domain semantics as request-path reconciliation.
- Abandoned reservations no longer require local in-memory cleanup logic.
- Expired reservation recovery is tenant-scoped and ledger-auditable.

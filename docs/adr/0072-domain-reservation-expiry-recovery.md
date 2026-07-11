# ADR 0072: Domain reservation expiry and recovery

## Status

Accepted.

## Context

The enterprise accounting target requires reservation-based budget handling:
reserve before upstream calls, commit actual usage after response, release the
reservation delta, and expire or recover abandoned reservations. `prodex-domain`
already modeled reservation requests, checked commits, and append-only ledger
idempotency keys, but it did not model abandoned reservation recovery.

## Decision

Add `ReservationRecord`, `ReservationRecoveryError`, and
`release_expired_reservation`. A reservation record captures tenant, call,
reservation, reserved amount, creation time, and expiry time. Expired recovery is
tenant-scoped, does not increase committed usage, and emits a `Released` ledger
event using the same tenant/call/reservation/kind idempotency key shape as other
ledger events.

## Consequences

- Future storage implementations can recover abandoned reservations without
  double charging.
- Release events remain append-only and idempotent.
- The domain logic is time-input driven and does not perform background work,
  filesystem access, database access, or network I/O.

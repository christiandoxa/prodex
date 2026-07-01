# ADR 0923: Storage atomic reservation debug redaction

## Status

Accepted.

## Context

Atomic reservation commands and plans carry storage keys, idempotency keys,
budget snapshots, request identifiers, reservation records, ledger events, and
timing fields. Nested domain types already redact many sensitive values, but
derived debug output still exposed too much reservation shape at the storage
boundary.

## Decision

Use custom `Debug` implementations for `AtomicReservationCommand` and
`AtomicReservationPlan`. Redact storage keys, idempotency keys, budget/request
payloads, reservation records, ledger events, and timing fields while keeping
the typed fields public for storage adapters and tests.

Regression coverage rejects tenant ID, call ID, reservation ID, idempotency key,
and raw TTL values in rendered atomic reservation debug output.

## Consequences

Storage diagnostics can identify atomic reservation command/plan shape without
exposing request or accounting identifiers. Planning behavior is unchanged.

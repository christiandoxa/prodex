# ADR 0395: Observability Reservation Recovery Metric

## Status

Accepted.

## Context

Reservation-based accounting must recover abandoned reservations across replicas. Operators need to count expired scans, recovery lease attempts, budget releases, ledger writes, skipped work, lease contention, and failures without exposing tenant IDs, reservation IDs, lease keys, token amounts, request payloads, or raw storage errors as metric labels.

## Decision

Add `plan_reservation_recovery_metric` to `prodex-observability`.

The planner emits `prodex_reservation_recovery_events_total`, increments by one, and uses only the closed enum labels `reservation_recovery_operation` and `reservation_recovery_result`.

## Consequences

- Expired reservation recovery adapters can publish scan, lease, release, ledger-write, skip, and failure outcomes through one shared low-cardinality contract.
- Tenant IDs, reservation IDs, lease keys, token amounts, request payloads, and raw storage errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the reservation recovery telemetry contract before a concrete metrics backend is wired in.

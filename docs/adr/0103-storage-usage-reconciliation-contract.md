# ADR 0103: Storage Usage Reconciliation Contract

## Status

Accepted

## Context

Reservation-based accounting requires more than an atomic reserve operation.
After the upstream call finishes, is cancelled, or is interrupted, the gateway
must commit actual usage, release unused reservation capacity, and write
idempotent ledger events without relying on replica-local state.

The storage boundary needs an adapter-neutral reconciliation contract before
PostgreSQL, SQLite, or future durable adapters can implement transactional
commit/release SQL.

## Decision

`prodex-storage` defines `UsageReconciliationCommand`,
`UsageReconciliationPlan`, and `plan_usage_reconciliation`. The plan:

- validates that the storage key tenant matches the reservation record tenant;
- delegates actual-vs-reserved validation to the domain reconciliation model;
- returns the updated budget snapshot;
- emits idempotent committed and optional released ledger events.

The crate remains side-effect-free and does not choose a database driver,
runtime, HTTP framework, or filesystem implementation.

## Consequences

- Data-plane adapters can reconcile streaming completion, cancellation, and
  interruption without inventing duplicate accounting semantics.
- Ledger event idempotency remains keyed by tenant, call, reservation, and
  event kind.
- Durable SQL adapters can now add transactional reconciliation plans behind
  this shared contract.

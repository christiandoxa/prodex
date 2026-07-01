# ADR 0104: PostgreSQL Usage Reconciliation Plan

## Status

Accepted

## Context

Reservation-based accounting must atomically commit actual usage, release unused
reserved capacity, and append idempotent ledger events after upstream
completion, cancellation, or stream interruption. The storage boundary now
defines a driver-neutral reconciliation contract; PostgreSQL needs a matching
request-path SQL plan that keeps DDL outside serving paths and preserves tenant
isolation.

## Decision

`prodex-storage-postgres` defines `plan_postgres_usage_reconciliation` and the
`reconcile_usage_atomically` DML statement. The plan validates the shared
`UsageReconciliationCommand`, then returns:

- `set_tenant_context`;
- a reconciliation statement that locks the reservation row, updates tenant
  budget counters, marks the reservation as committed or released, and inserts
  committed plus optional released ledger events.

Ledger inserts use `(tenant_id, reservation_id, event_kind)` idempotency, and
all request-path statements remain DML-only.

## Consequences

- PostgreSQL can act as the durable source of truth for post-upstream usage
  reconciliation across gateway replicas.
- Retries can reuse the same reservation/call identity without duplicate ledger
  charges.
- Adapter implementations can execute the plan transactionally while this crate
  remains driver-free.

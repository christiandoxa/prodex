# ADR 0105: SQLite Usage Reconciliation Plan

## Status

Accepted

## Context

SQLite is a local compatibility backend, but it still needs the same
reservation-based accounting semantics as the enterprise data plane: commit
actual usage, release unused capacity, and append idempotent ledger events
after completion, cancellation, or stream interruption. The request-serving
path must remain free of migration DDL.

## Decision

`prodex-storage-sqlite` defines `plan_sqlite_usage_reconciliation` and a
`reconcile_usage_locally` DML statement. The plan validates the shared
`UsageReconciliationCommand`, then returns:

- `BEGIN IMMEDIATE`;
- DML to update tenant budget counters, mark the reservation, and insert
  committed plus optional released ledger rows;
- `COMMIT`.

Cross-tenant storage keys and actual usage above the reserved amount are
rejected before SQL statements are exposed.

## Consequences

- Local compatibility deployments preserve post-upstream accounting semantics.
- SQLite request paths can reconcile usage without running DDL.
- The durable enterprise source of truth remains PostgreSQL; SQLite remains a
  local/single-node compatibility target.

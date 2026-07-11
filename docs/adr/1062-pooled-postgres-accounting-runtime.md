# ADR 1062: Pooled PostgreSQL Accounting Runtime

## Status

Accepted

## Context

The PostgreSQL storage boundary already owns tenant-scoped, atomic reservation
and reconciliation SQL plans. The active compatibility gateway still opens a
synchronous PostgreSQL connection per accounting operation, so it cannot provide
an async connection pool or bounded database wait budget for the enterprise
gateway adapter. Moving a driver into `prodex-storage-postgres` would weaken the
existing driver-free plan boundary.

## Decision

Add `prodex-storage-postgres-runtime` as the async execution adapter beneath the
application composition root. It:

- consumes mutation plans from `prodex-storage-postgres` and executes no DDL;
- uses a bounded `deadpool-postgres` pool and caller-selected TLS connector;
- names the development-only no-TLS constructor explicitly;
- applies tenant context inside every transaction;
- uses serializable transactions with bounded retries for reservation races;
- bounds pool acquisition and complete repository operations with explicit
  timeouts;
- distinguishes reserved, replayed, rejected, applied, and replayed outcomes;
- loads reservations by typed tenant and call identifiers for cross-replica
  reconciliation; and
- returns stable redacted errors and debug output.

The shared reconciliation SQL casts release parameters to `BIGINT`. This keeps
the PostgreSQL inferred parameter type aligned with the domain `u64` to checked
`i64` conversion used by both the existing and async executors.

## Consequences

Two repository instances can reserve and reconcile against the same PostgreSQL
database without duplicate ledger events or counter drift. Heavy CI runs this
real database proof before the backup and restore drill.

The runtime adapter is not enabled merely by existing in the workspace. The
gateway composition root must inject it, preserve request cancellation, and
retain the production startup gate until all required multi-replica accounting
checks and Redis coordination are wired.

Rollback removes the runtime adapter wiring and returns to the previous executor;
no schema rollback is required because this decision adds no migrations.

## Verification

```bash
cargo test -p prodex-storage-postgres-runtime
cargo clippy -p prodex-storage-postgres-runtime --all-targets -- -D warnings
npm run ci:storage-postgres-proof
```

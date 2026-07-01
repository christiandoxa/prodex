# ADR 0118: Multi-replica accounting concurrency spec

## Status

Accepted

## Context

Enterprise accounting requires database-backed concurrency tests with at least
two gateway replicas sharing PostgreSQL and Redis. The durable storage and Redis
boundary crates now expose atomic reservation, reconciliation, recovery, and
coordination plans, but adapter/integration suites still need a canonical list
of invariants that must be proven before production readiness is claimed.

## Decision

Add `plan_multi_replica_accounting_concurrency_spec` to `prodex-storage`. The
contract accepts a storage topology and gateway replica count, then requires:

- at least two gateway replicas;
- PostgreSQL as the durable source of truth;
- Redis as the shared coordination/cache backend; and
- checks for no lost update, no duplicate charge, no dropped ledger event, no
  request id collision, and no undocumented limit overshoot.

The contract is side-effect-free and driver-independent. It does not execute the
database tests itself; adapter and deployment test suites must use it to select
the required checks and then run against real PostgreSQL/Redis instances.

## Consequences

The remaining full database concurrency validation is now explicit and
machine-checkable at the storage boundary. SQLite/local compatibility paths
cannot satisfy the production multi-replica accounting gate.

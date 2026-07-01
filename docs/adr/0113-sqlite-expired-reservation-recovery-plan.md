# ADR 0113: SQLite expired reservation recovery plan

## Status

Accepted

## Context

Local SQLite compatibility must support abandoned reservation cleanup without
running migrations or other DDL from request-serving paths. Expired reservations
also need the same tenant-scoped, idempotent release semantics as the durable
PostgreSQL accounting plan so cancellation and interrupted streams do not leave
reserved capacity permanently unavailable.

## Decision

Add `plan_sqlite_expired_reservation_recovery` in `prodex-storage-sqlite`. The
plan delegates validation to the storage-domain `plan_expired_reservation_recovery`
contract, maps tenant mismatch and not-expired failures into SQLite-specific
errors, and emits a `BEGIN IMMEDIATE` transaction with DML-only statements.

The recovery statement:

- requires `tenant_id`, `reservation_id`, and `call_id` predicates;
- releases counters only when the reservation is uncommitted, unreleased, and
  expired;
- marks the reservation released with the same timestamp; and
- writes an idempotent `released` ledger row using the tenant-scoped unique key.

## Consequences

SQLite remains a local compatibility store, not the multi-replica source of
truth, but its accounting behavior now mirrors the enterprise reservation
lifecycle and keeps DDL out of hot paths.

# ADR 0112: PostgreSQL Expired Reservation Recovery Plan

## Status

Accepted

## Context

The storage boundary now models recovery of abandoned reservations. PostgreSQL
is the durable source of truth for enterprise accounting, so it needs a DML-only
plan that releases expired reservations without reintroducing request-path DDL
or local in-memory cleanup semantics.

## Decision

`prodex-storage-postgres` exposes
`plan_postgres_expired_reservation_recovery` and the
`recover_expired_reservation` DML statement. The plan:

- validates tenant-scoped recovery through `prodex-storage`;
- sets the PostgreSQL tenant context;
- locks the expired reservation row;
- subtracts reserved counters;
- marks the reservation released;
- inserts an idempotent `released` ledger event.

## Consequences

- Abandoned reservation recovery can be executed transactionally against
  PostgreSQL across gateway replicas.
- Recovery remains tenant-scoped and uses the same ledger idempotency model as
  request-path reconciliation.
- DDL remains external-only.

# 0227: Redis Durable State Prohibition Plan

## Status

Accepted.

## Context

The enterprise target uses PostgreSQL as the durable source of truth for
tenant-owned state, budget reservations, billing ledger events, audit records,
and published configuration. Redis may support horizontal replicas only as a
distributed limiter, short-lived cache, or rebuildable coordination primitive.

Legacy gateway compatibility paths still include Redis usage and ledger adapters.
The boundary crate needs an explicit contract that rejects Redis as durable
state before adapters are migrated.

## Decision

Add `plan_redis_storage_use` to `prodex-storage-redis`.

The planner allows only:

- distributed rate limiting;
- short-lived cache entries; and
- coordination leases.

It rejects durable usage accounting, billing ledger, audit log, and
configuration state with `RedisPlanError::DurableStateForbidden`.

## Consequences

Future Redis adapters can execute limiter/cache/coordination plans without
changing the production contract. Durable accounting and control-plane state
must flow through PostgreSQL-backed storage plans, with Redis data treated as
rebuildable and disposable.

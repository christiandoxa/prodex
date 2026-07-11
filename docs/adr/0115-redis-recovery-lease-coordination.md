# ADR 0115: Redis recovery lease coordination

## Status

Accepted

## Context

Expired reservation recovery may run from multiple gateway replicas or scheduled
workers. Durable PostgreSQL recovery is idempotent, but multi-replica cleanup
still needs a short-lived coordination primitive so replicas avoid repeatedly
scanning and attempting the same recovery shard.

Redis is allowed only for rate limiting, short-lived cache, and rebuildable
coordination. It must not become a durable usage source of truth or store whole
usage maps.

## Decision

Add Redis recovery lease plans in `prodex-storage-redis`:

- `plan_recovery_lease_acquire` creates a tenant-scoped lease key for a recovery
  shard and an owner token with `SET NX PX` semantics. The same owner may renew
  the lease with `PEXPIRE`.
- `plan_recovery_lease_release` releases only when the stored owner token
  matches, shortening the TTL with `PEXPIRE` instead of deleting unrelated state.

The plans validate tenant mismatch, missing TTL, and empty owner tokens before
adapter execution. Lua scripts avoid JSON, lists, and whole-map rewrites.

## Consequences

Recovery workers can coordinate across replicas without making Redis durable
accounting storage. If Redis is unavailable, PostgreSQL idempotency still
protects ledger correctness, but operators can use the lease plan to reduce
duplicate recovery work under normal conditions.

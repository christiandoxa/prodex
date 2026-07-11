# ADR 0093: Redis Storage Rate-Limit Boundary

## Status

Accepted

## Context

Prodex uses Redis in legacy gateway paths for shared usage and ledger state. The
enterprise target requires PostgreSQL as the durable source of truth and Redis
only for distributed rate limiting, short-lived cache, and rebuildable
coordination. Redis must not store whole usage maps or ledger lists as JSON that
are loaded, mutated, and rewritten under a global lock.

The current adapter still has legacy compatibility code, so the next incremental
step is to define the target Redis contract in a driver-free boundary crate and
guard it from regressing into whole-map state.

## Decision

Add `prodex-storage-redis` as a client-free planning crate. It owns:

- tenant-scoped Redis key construction;
- atomic Lua for distributed rate-limit admission using `GET`, `INCRBY`, and
  `PEXPIREAT`;
- short-lived policy revision cache plans with mandatory TTL;
- short-lived coordination lock plans using `SET NX PX`;
- validation that tenant storage keys match the tenant context;
- helpers/tests asserting scripts avoid JSON, `LRANGE`, `LSET`, `RPUSH`,
  `DEL`, and whole-list rewrite patterns.

Add `scripts/ci/storage-redis-boundary-guard.mjs` and include its self-test plus
workspace scan in preflight. The guard forbids Redis clients,
HTTP/runtime/storage drivers, `serde_json`, and whole-list rewrite commands
inside the boundary crate.

## Consequences

Future Redis adapter work can execute these plans without changing the target
contract. Durable usage accounting and billing ledger writes must remain in
PostgreSQL. Redis outage or eviction may degrade rate-limit/cache behavior but
must not lose billing source-of-truth data.

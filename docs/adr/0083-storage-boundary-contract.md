# ADR 0083: Storage boundary contract

## Status
Accepted

## Context
Enterprise budget and usage accounting must be atomic across gateway replicas.
The target architecture also requires tenant IDs to be present in durable keys,
ledger uniqueness, idempotency keys, and migration behavior that is not run from
request-serving paths. These rules need a storage boundary that gateway and
future Postgres/Redis/SQLite adapter crates can share without putting database
drivers or filesystem/network behavior into domain or application code.

## Decision
Introduce `prodex-storage` as an adapter-neutral storage contract crate. The
crate defines tenant-scoped storage keys, gateway storage topology validation,
and atomic reservation planning primitives. Reservation plans carry the tenant
storage key, idempotency key, durable reservation record, and append-only ledger
event that storage adapters must persist atomically.

Gateway storage topology validation rejects any configuration where migrations
may run on a request-serving path. Tenant mismatch between the storage key and
reservation request is rejected before any adapter can execute a mutation.

## Consequences
Postgres/Redis/SQLite adapter crates can be added incrementally behind this
contract. Durable adapters should implement the reservation plan with a single
transaction and uniqueness on tenant, call, reservation, and ledger event kind.
Redis remains a cache/rate-limit/coordination adapter and should not become the
durable source of truth for whole usage state.

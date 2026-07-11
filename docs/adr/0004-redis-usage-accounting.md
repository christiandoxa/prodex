# ADR 0004: Redis usage accounting uses per-key atomic updates

## Status

Accepted.

## Context

Phase 0 audit identified Redis usage accounting that persisted the complete
virtual-key usage map as one JSON value. Updating that map requires a global
read/modify/write cycle and can lose increments when multiple gateway replicas
share the same Redis backend.

## Decision

Redis usage accounting now stores an index set plus one Redis hash per virtual
key. Applying usage deltas runs a Lua `EVAL` operation per virtual key to reset
minute counters when needed and increment request, token, and spend counters
atomically.

The loader keeps a legacy whole-map JSON fallback when the per-key index does
not exist, so existing development state can still be read during the transition.
Ledger append remains separate and should be migrated to append-only atomic Redis
operations in a later slice.

## Consequences

- Concurrent Redis usage updates no longer rewrite a shared whole-map JSON value.
- Redis usage counters are scoped to each virtual key and updated atomically.
- Redis remains a coordination/cache backend; PostgreSQL is still the desired
  durable source of truth for multi-replica production accounting.

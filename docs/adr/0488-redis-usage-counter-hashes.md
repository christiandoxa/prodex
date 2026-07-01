# ADR 0488: Store Redis Gateway Usage as Atomic Hash Counters

## Status

Accepted.

## Context

Enterprise deployments cannot rely on Redis whole-map JSON snapshots for gateway usage accounting. A whole-map shape makes concurrent updates fragile and turns Redis into durable application state instead of a coordination/cache layer.

## Decision

The Redis usage backend stores each virtual key's counters in its own hash and tracks key names in an index set:

- `<usage>:keys`
- `<usage>:key:<name>`

Usage deltas are applied with a small Lua script that uses `HINCRBY` for request, token, and spend counters. Reads still accept the legacy whole-map JSON payload when the index set is absent.

## Consequences

Redis usage updates no longer rewrite a full usage map. This keeps Redis usage closer to atomic counter coordination while preserving legacy read compatibility.

PostgreSQL remains the target durable source of truth for regulated multi-replica accounting.

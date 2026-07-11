# ADR 0117: Application recovery lease release boundary

## Status

Accepted

## Context

Application-level expired reservation recovery can acquire Redis recovery leases
for multi-replica cleanup coordination. Workers also need to release those
leases safely after a cleanup attempt completes or fails. Without an application
boundary, composition roots would call Redis lease primitives directly and risk
duplicating tenant and owner-token validation.

## Decision

Add `plan_application_recovery_lease_release` to `prodex-application`. The use
case accepts a tenant id, storage key, recovery shard, and owner token, then
delegates to the Redis storage boundary `plan_recovery_lease_release`.

The release plan is side-effect-free and contains only the Redis lease release
script plan. Cross-tenant storage keys and empty owner tokens are rejected before
adapter execution.

## Consequences

Recovery workers can acquire and release coordination leases through application
use cases while keeping Redis as a rebuildable coordination primitive. Durable
accounting correctness continues to rely on PostgreSQL/SQLite recovery plans and
tenant-scoped idempotent ledger events.

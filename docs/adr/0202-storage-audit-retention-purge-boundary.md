# 0202: Storage Audit Retention Purge Boundary

## Status

Accepted

## Context

Domain retention cleanup now produces tenant-scoped purge batches. Storage
adapters still need an adapter-neutral command that validates the storage key
matches the batch tenant before any concrete PostgreSQL, SQLite, or future
adapter builds delete statements.

Without this boundary, a composition root could pass a valid domain batch to the
wrong tenant storage key and rely on each adapter to rediscover the mismatch.

## Decision

Add `AuditRetentionPurgeCommand` and `AuditRetentionPurgePlan` to
`prodex-storage`.

The storage plan accepts a `TenantStorageKey` and `AuditRetentionPurgeBatch`.
Planning rejects tenant mismatches and exposes a stable redacted error response
through `plan_audit_retention_purge_error_response`. Concrete SQL adapters must
use the planned batch tenant and event IDs when building retention delete DML.

## Consequences

- Retention purge deletes have an adapter-neutral storage boundary.
- Storage-key tenant mismatch fails before database mutation.
- Concrete adapters can share one validated command shape.
- Error responses do not expose tenant IDs, event IDs, or backend details.

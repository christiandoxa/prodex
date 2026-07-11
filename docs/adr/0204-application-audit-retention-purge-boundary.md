# 0204: Application Audit Retention Purge Boundary

## Status

Accepted

## Context

Domain and storage crates now validate retention purge keys, batches, and SQL
adapter plans. The application boundary still needs a side-effect-free use case
that selects the configured durable store without letting composition roots
duplicate SQL adapter selection or expose storage-specific details.

Retention purge is a control-plane/background operation, but it mutates
tenant-owned audit storage. It must preserve tenant-scoped storage plans and
stable redacted failures before concrete adapters execute anything.

## Decision

Add `plan_application_audit_retention_purge`.

The use case accepts an `AuditRetentionPurgeCommand` and a durable store kind.
It selects PostgreSQL or SQLite retention purge SQL plans and maps storage
planning failures to a stable application error envelope. Tenant IDs, audit
event IDs, backend names, and SQL details stay out of client-visible failures.

## Consequences

- Composition roots call one application use case for retention purge.
- Durable-store selection stays outside `prodex-app` business logic.
- Storage tenant mismatch remains fail-closed before SQL execution.
- Application-level error responses stay stable and redacted.

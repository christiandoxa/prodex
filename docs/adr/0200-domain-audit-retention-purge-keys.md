# 0200: Domain Audit Retention Purge Keys

## Status

Accepted

## Context

Retention cleanup pages identify events that may be deleted, but storage
adapters should not execute deletes using a bare audit event ID. Enterprise
tenant isolation requires tenant-owned resources to carry tenant scope into
query predicates, indexes, cache keys, and delete operations.

If retention storage code reconstructs delete keys on its own, PostgreSQL,
SQLite, Redis-backed metadata, and future adapters can drift on tenant scoping
or accidentally delete a matching event ID outside the current tenant scope.

## Decision

Introduce `AuditRetentionPurgeKey`.

The key contains `tenant_id` and `audit_event_id`. It can be derived only from
an `AuditEvent` plus an `AuditQueryScope`, which validates tenant scope and the
event timestamp before exposing a storage-facing delete key.
`AuditRetentionPlan::purgeable_candidate_key_page` combines legal-hold-aware
pagination with key derivation, preserving the next cursor while returning only
tenant-scoped delete keys.

## Consequences

- Storage adapters can delete retention candidates with explicit tenant
  predicates.
- Bare audit event IDs are no longer the recommended retention delete contract.
- Cross-tenant delete-key construction fails closed in the domain boundary.
- Retention cursor and legal-hold behavior stays shared with purgeable pages.

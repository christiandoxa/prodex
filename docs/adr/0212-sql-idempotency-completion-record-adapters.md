# 0212: SQL Idempotency Completion Record Adapters

## Status

Accepted

## Context

Pending idempotency records stop duplicate mutating admin requests from running
concurrently, but the system also needs a durable completion marker so later
retries can replay the completed response instead of executing the mutation
again.

Without a tenant-scoped completion contract, adapters could update replay state
without checking the original request fingerprint, or store completion data in a
way that bypasses SQL tenant isolation.

## Decision

Add `IdempotencyCompletedRecordCommand` and
`plan_idempotency_completed_record` to `prodex-storage`. The command carries
the `TenantStorageKey`, `IdempotentOperation`, completion timestamp, and
serialized response body. Planning rejects cross-tenant storage keys and
returns a completed `IdempotencyEntry<Vec<u8>>`.

Add `plan_postgres_idempotency_completed_record` and
`plan_sqlite_idempotency_completed_record`. Both adapter plans reuse the
storage boundary and emit request-path DML only. The SQL update requires the
same `(tenant_id, idempotency_key, request_fingerprint)` and only transitions a
`pending` row to `completed`. PostgreSQL also sets the RLS tenant context;
SQLite wraps the update in `BEGIN IMMEDIATE`/`COMMIT`.

## Consequences

- Completed admin mutation responses have a durable replay marker per tenant
  and idempotency key.
- Reused idempotency keys with a different request fingerprint cannot complete
  an unrelated pending operation.
- SQL adapters keep completion persistence out of migration/open paths and use
  request-path DML only.
- Response serialization format remains a composition-root concern; the
  storage boundary carries opaque bytes.

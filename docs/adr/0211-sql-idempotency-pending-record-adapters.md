# 0211: SQL Idempotency Pending Record Adapters

## Status

Accepted

## Context

The storage boundary can now produce a tenant-scoped pending idempotency entry
for mutating control-plane operations. Durable SQL adapters still need a
concrete schema and request-path DML plan so pending replay markers are written
before destructive operations such as audit retention purge.

If each adapter defines this independently, tenant predicates, uniqueness, RLS,
or local transaction behavior can drift and duplicate admin mutations may race
or replay incorrectly.

## Decision

Add `prodex_idempotency_records` to the PostgreSQL and SQLite migration plans.
The table is keyed by `(tenant_id, idempotency_key)` and stores the request
fingerprint, entry status, start time, optional completion time, and optional
response body. PostgreSQL enables RLS on the table with the same
`prodex.tenant_id` setting used by the other tenant-owned tables.

Add `plan_postgres_idempotency_pending_record` and
`plan_sqlite_idempotency_pending_record`. Both plans reuse
`plan_idempotency_pending_record`, reject cross-tenant storage keys, and emit
request-path DML only. PostgreSQL sets tenant context and inserts with
`ON CONFLICT (tenant_id, idempotency_key) DO NOTHING`; SQLite wraps
`INSERT OR IGNORE` in `BEGIN IMMEDIATE`/`COMMIT`.

## Consequences

- Pending admin mutation replay markers have a durable tenant-scoped SQL home.
- Duplicate pending inserts are idempotent per tenant and key.
- PostgreSQL gets RLS defense in depth for idempotency replay records.
- SQLite keeps local compatibility while still serializing pending inserts with
  an immediate transaction.
- Completion persistence and response serialization can be added as a separate
  reviewable adapter slice.

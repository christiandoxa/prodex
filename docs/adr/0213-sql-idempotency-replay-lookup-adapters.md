# 0213: SQL Idempotency Replay Lookup Adapters

## Status

Accepted

## Context

Pending and completed idempotency records give mutating control-plane
operations a durable replay state, but a retry must first load the existing
record before deciding whether to execute, wait, replay, or reject a reused key.

Filtering the lookup by request fingerprint would hide reused-key conflicts as
missing records. Filtering without tenant scope would allow cross-tenant replay
or information leakage.

## Decision

Add `IdempotencyRecordLookupCommand` and
`plan_idempotency_record_lookup` to `prodex-storage`. The boundary rejects
storage-key and operation tenant mismatches and returns the tenant-scoped
operation that adapters should query.

Add `plan_postgres_idempotency_record_lookup` and
`plan_sqlite_idempotency_record_lookup`. Both plans read
`prodex_idempotency_records` by `(tenant_id, idempotency_key)` only and return
the stored fingerprint, status, timestamps, and response body. PostgreSQL sets
the RLS tenant context before the lookup. SQLite emits a single DML-only SELECT.

The domain replay guard remains responsible for comparing the stored
fingerprint with the current request fingerprint so reused idempotency keys fail
as conflicts instead of silently executing again.

## Consequences

- Control-plane retries can load durable replay state before executing a
  mutation.
- Cross-tenant lookup attempts fail before reaching adapters.
- Reused keys with different fingerprints stay observable to the domain replay
  guard.
- SQL lookup plans remain request-path DML and do not run migration DDL.

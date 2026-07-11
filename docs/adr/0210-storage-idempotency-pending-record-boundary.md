# 0210: Storage Idempotency Pending Record Boundary

## Status

Accepted

## Context

Application control-plane idempotency can decide whether a mutating admin
request should execute, wait, replay, or fail with a conflict. Before execution,
composition roots need a storage-neutral way to record the pending operation so
concurrent duplicate requests observe an in-progress replay entry.

Without a tenant-scoped pending-record storage contract, adapters can diverge on
tenant predicates or skip the pending marker for destructive operations such as
audit retention purge.

## Decision

Add `IdempotencyPendingRecordCommand` and
`plan_idempotency_pending_record` to `prodex-storage`.

The command carries a `TenantStorageKey`, `IdempotentOperation`, and start
timestamp. Planning rejects storage-key and operation tenant mismatches, then
produces a pending `IdempotencyEntry<()>` for adapter persistence. Errors map to
a stable redacted storage response.

## Consequences

- Storage adapters share one tenant-scoped pending idempotency record contract.
- Concurrent duplicate admin mutations can observe pending state before
  mutation execution.
- Tenant IDs, idempotency keys, and request fingerprints stay out of
  storage-facing error responses.
- Concrete SQL/Redis adapters can add persistence plans without redefining the
  domain replay semantics.

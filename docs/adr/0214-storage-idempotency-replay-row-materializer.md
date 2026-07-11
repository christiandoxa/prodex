# 0214: Storage Idempotency Replay Row Materializer

## Status

Accepted

## Context

SQL replay lookup plans load durable idempotency rows by `(tenant_id,
idempotency_key)`. After the row is fetched, adapters still need a common way to
turn the raw row shape into an `IdempotencyEntry<Vec<u8>>` for the domain replay
guard.

If each adapter materializes rows independently, one adapter may ignore tenant
or key mismatches, treat a completed row without completion metadata or a
response body as replayable, or drop a mismatched request fingerprint before
the domain can reject reused keys.

## Decision

Add `IdempotencyRecordLookupRow`,
`IdempotencyRecordLookupRowStatus`, and
`materialize_idempotency_record_lookup_row` to `prodex-storage`.

The materializer checks that the loaded row tenant and idempotency key match
the requested operation. It preserves the row request fingerprint in the
materialized `IdempotencyEntry` so `decide_idempotency_replay` can detect
reused keys with different fingerprints. Pending rows become pending entries.
Completed rows must include a completion timestamp and response body before
they become completed entries with opaque response bytes.

Materializer errors use a stable redacted storage response.

## Consequences

- PostgreSQL and SQLite adapters share one fail-closed replay row
  materialization contract.
- Cross-tenant or wrong-key rows cannot be replayed accidentally.
- Reused-key fingerprint mismatches remain visible to the domain replay guard.
- Completed rows missing a completion timestamp or response body fail closed
  instead of producing an invalid replay.

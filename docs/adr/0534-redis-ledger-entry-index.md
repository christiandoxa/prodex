# ADR 0534: Store Redis Gateway Ledger Entries by Id

## Status

Accepted.

## Context

The Redis gateway billing ledger adapter appended new request entries by loading
the entire ledger list, checking duplicates in memory, and replacing the full
list under a Redis lock. That preserved compatibility for small local
installations, but it made Redis behave like durable whole-map state and created
a global write bottleneck in multi-replica deployments.

PostgreSQL remains the target durable accounting source for enterprise
deployments. The Redis adapter still needs to avoid lost updates and whole-list
rewrites while legacy data is migrated away.

## Decision

New Redis ledger writes store each ledger record as an entry payload keyed by a
stable id derived from `(call_id, lower(key_name), phase)`. An ordered index list
tracks entry ids, and a per-call index tracks entries that can be reconciled
after a provider response.

Append uses a Redis Lua script with `SETNX` to dedupe atomically before adding
the entry id to the indexes. Response reconciliation reads only the indexed
entries for the call id and updates those payloads directly.

Reads continue to fall back to the legacy list payload when the new index is
absent, but new writes no longer load and replace the whole ledger list. Redis
ledger reads cap unbounded compatibility requests at 100,000 tail records, and a
zero limit returns no records instead of issuing `LRANGE 0 -1`.

## Consequences

Redis ledger writes no longer require a global ledger lock or whole-list
rewrite. Duplicate request accounting is keyed the same way as the SQL ledger:
`call_id`, case-insensitive `key_name`, and `phase`.

Legacy Redis list records remain readable for admin export, but response
reconciliation is entry-index based for new writes. Regulated multi-replica
deployments should use PostgreSQL for durable accounting. Very large historical
Redis exports should be paginated or migrated through PostgreSQL instead of
depending on full-list reads.

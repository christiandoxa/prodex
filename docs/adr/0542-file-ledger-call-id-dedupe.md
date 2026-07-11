# ADR 0542: File billing ledger dedupes call entries

## Status

Accepted

## Context

The file-backed gateway billing ledger appended every usage delta. If a gateway
save retried the same admitted call, the file ledger could contain duplicate
`request` phase rows for the same virtual key and call.

## Decision

While holding the existing file ledger lock, stream existing JSONL entries into
a set of `(call_id, lower(key_name), phase)` ids and skip new entries whose id
already exists. This matches the SQL and Redis gateway ledger idempotency key
without changing the JSONL format.

File ledger reads also stream JSONL line by line and retain only the requested
tail window instead of reading the whole file into memory before applying the
limit.

Response reconciliation also streams existing JSONL entries into a temporary
file, applies the matching `call_id`, and renames the temporary file only when a
request-phase row changed.

## Consequences

File-backed local deployments no longer double-record retried usage deltas. The
check remains an O(n) compatibility path under the existing file lock, but it no
longer materializes full ledger records just to build the dedupe set or satisfy
bounded reads or response reconciliation. Regulated multi-replica deployments
should still use PostgreSQL for durable accounting.

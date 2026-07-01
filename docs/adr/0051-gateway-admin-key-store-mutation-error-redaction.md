# ADR 0051: Redact gateway admin key-store mutation errors

## Status

Accepted

## Context

Gateway admin key, SCIM, and related control-plane-compatible endpoints mutate the
virtual-key store through shared storage helpers. File, SQLite, PostgreSQL, and
Redis backends can fail while creating lock files, opening transactions, loading
state, or saving updates. Returning backend errors directly from those mutation
paths can expose local filesystem paths, database diagnostics, lock names, or
other implementation details in admin API responses.

## Decision

Key-store mutation failures now use stable error envelopes and redacted messages
for storage infrastructure failures:

- `gateway_key_store_lock_failed`: `gateway key store lock could not be acquired`
- `gateway_key_store_load_failed`: `gateway key store could not be loaded`
- `gateway_key_store_save_failed`: `gateway key store could not be saved`

Domain validation errors such as duplicate names, missing keys, invalid JSON, or
invalid persisted key-store contents keep their existing machine-readable codes
and client-safe messages.

## Consequences

- Admin API responses no longer expose raw lock, transaction, IO, Redis, or SQL
  diagnostics from key-store mutation failures.
- Regression coverage verifies a deterministic file-lock failure does not leak
  the lock filename, root path, raw IO message, or admin token.
- Operators should use runtime logs and backend diagnostics for low-level storage
  investigation rather than API response bodies.

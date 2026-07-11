# ADR 0587: SQL Ledger Request Scope Snapshot

## Status

Accepted

## Context

File and Redis gateway billing ledgers persist the typed UUIDv7 request ID and
admission-time governance scope snapshot. The compatibility SQL gateway ledger
still stored only the legacy numeric request sequence, call ID, key name, and
model fields, so SQL-backed billing reads could not authorize historical rows
from immutable row scope without falling back to mutable key metadata.

## Decision

Fresh SQLite and PostgreSQL gateway billing ledger schemas include nullable
`typed_request_id`, `tenant_id`, `team_id`, `project_id`, `user_id`, and
`budget_id` columns. SQL ledger insert and load paths now round-trip those
fields while preserving the legacy numeric `request_id` column and call-ID
dedupe key. Existing schemas without those columns continue using the legacy
insert/select shape and do not run request-path DDL.

## Consequences

- New SQL-backed gateway ledger rows keep the same typed request and scope
  metadata as file and Redis ledger rows.
- Existing SQL deployments still need an explicit versioned migration before
  old schemas can expose these columns.
- The SQL ledger format remains backward-compatible for callers that still read
  the legacy numeric request sequence.

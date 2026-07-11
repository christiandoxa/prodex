# ADR 0143: SQL storage stable error responses

## Status

Accepted

## Context

`prodex-storage-postgres` and `prodex-storage-sqlite` own request-path SQL
planning, migration planning, tenant-scope checks, reservation validation,
reconciliation validation, recovery validation, and append-only audit planning.
Raw storage planning errors can include tenant IDs, usage amounts, idempotency
details, backend names, and migration/DDL internals. These details are useful
for trusted diagnostics but must not become client-visible API or worker
response text.

## Decision

Add stable redacted response planners to the SQL storage crates:

- `plan_postgres_storage_error_response`
- `plan_sqlite_storage_error_response`

Both planners map raw storage planning errors to a service-unavailable response
for their storage boundary. Application composition roots may adapt the storage
status into use-case-specific codes and messages while preserving the storage
boundary's redaction decision.

## Consequences

PostgreSQL and SQLite storage planning failures now have reusable redaction
boundaries. Application-level usage reconciliation, expired-reservation
recovery, audit persistence, configuration publication, and runtime planning
responses no longer need to inspect raw SQL storage error variants to decide
whether the failure is safe to expose.
